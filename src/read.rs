use core::str::from_utf8;
use embassy_sync::{blocking_mutex::raw::RawMutex, pipe::Pipe};
use embassy_time::Instant;
use embedded_io_async::Read;
use heapless::Vec;

use crate::{Error, log};

pub struct ModemReader<'context, M: RawMutex> {
    read: &'context Pipe<M, 2048>,
    buffer: Vec<u8, 256>,
    line_timestamps: Vec<(usize, Instant), 3>,
    line_end: Option<usize>,
}

impl<'context, M> ModemReader<'context, M>
where
    M: RawMutex,
{
    pub fn new(read: &'context Pipe<M, 2048>) -> ModemReader<'context, M> {
        ModemReader {
            read,
            buffer: Vec::new(),
            line_timestamps: Vec::new(),
            line_end: None,
        }
    }

    fn drain_line(&mut self, line_end: usize) {
        self.buffer.rotate_left(line_end);
        self.buffer.truncate(self.buffer.len() - line_end);

        if let Some(next_timestamp_index) = self
            .line_timestamps
            .iter()
            .position(|(offset, _)| *offset > line_end)
        {
            if next_timestamp_index > 0 {
                self.line_timestamps.drain(..next_timestamp_index);
            }
            if let Some((offset, _)) = self.line_timestamps.first_mut() {
                *offset = offset.saturating_sub(line_end);
            }
        } else {
            self.line_timestamps.clear();
        }
    }

    pub async fn read_line(&mut self) -> Result<(&str, Option<&Instant>), Error> {
        if let Some(line_end) = self.line_end.take() {
            // Remove the previously read line from the buffer
            self.drain_line(line_end);
        }
        const MODEM_INPUT_PROMPT: &str = "> ";
        const LINE_END: &str = "\n";
        loop {
            #[cfg(debug_assertions)]
            if !self.buffer.is_empty() {
                match from_utf8(&self.buffer) {
                    Ok(line) => log::trace!("CURRENT BUFFER (utf-8) {:?}", line),
                    Err(_) => log::trace!("CURRENT BUFFER (binary) {:?}", self.buffer.as_slice()),
                }
            }

            if self.buffer.starts_with(MODEM_INPUT_PROMPT.as_bytes()) {
                // When the modem outputs a "> " without a CRLF, it's expecting input,
                // since there is no CRLF we handle this as a special case.
                // Notably this happens after a CIPSEND command

                self.drain_line(MODEM_INPUT_PROMPT.len());

                return Ok((
                    MODEM_INPUT_PROMPT,
                    self.line_timestamps.first().map(|(_, instant)| instant),
                ));
            } else if let Some(position) = self
                .buffer
                .windows(LINE_END.len())
                .position(|slice| slice == LINE_END.as_bytes())
            {
                // If we see a line break, the modem has probably sent us a message

                let line_end = position + LINE_END.len();
                let Ok(line) = from_utf8(&self.buffer[..position]) else {
                    self.buffer.rotate_left(line_end);
                    self.buffer.truncate(self.buffer.len() - line_end);
                    return Err(Error::InvalidUtf8);
                };
                log::trace!("RECV LINE: {:?}", line);

                // Ignore empty lines, as well as echoed lines (which end with \r\r\n)
                if line.trim().is_empty() || line.ends_with("\r\r") {
                    self.drain_line(line_end);
                    continue;
                }

                self.line_end = Some(line_end);

                let line = line.trim(); // The modem likes to be inconsistent with white space

                return Ok((
                    line,
                    self.line_timestamps.first().map(|(_, instant)| instant),
                ));
            }

            if self.buffer.capacity() == self.buffer.len() {
                panic!(
                    "read buffer is full, this should never happen. contents: {:?}",
                    self.buffer.as_slice()
                );
            }

            let mut buf = [0u8; 256];
            let amount = Read::read(
                &mut self.read,
                &mut buf[..self.buffer.capacity() - self.buffer.len()],
            )
            .await
            .map_err(|_| Error::Serial)?;

            // Store the read time for this chunk if there is space
            //  otherwise extend the last chunk
            if self.line_timestamps.len() < self.line_timestamps.capacity() {
                let _ = self.line_timestamps
                    .push((self.buffer.len() + amount, Instant::now()));
            } else if let Some((offset, _)) = self.line_timestamps.last_mut() {
                *offset = self.buffer.len() + amount;
            }

            self.buffer
                .extend_from_slice(&buf[..amount])
                .map_err(|_| Error::BufferOverflow)?;
        }
    }

    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if self.buffer.len() >= buf.len() {
            buf.copy_from_slice(&self.buffer.as_slice()[..buf.len()]);
            self.buffer.rotate_left(buf.len());
            self.buffer.truncate(self.buffer.len() - buf.len())
        } else {
            buf[..self.buffer.len()].copy_from_slice(self.buffer.as_slice());
            self.read
                .read_exact(&mut buf[self.buffer.len()..])
                .await
                .map_err(|_| Error::Serial)?; // TODO: figure out error types
            self.buffer.clear();
        }

        Ok(())
    }
}
