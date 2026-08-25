use core::str::from_utf8;
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::Receiver,
    pipe::{Pipe, Reader},
    pubsub::Subscriber,
};
use embassy_time::Instant;
use heapless::Vec;

use crate::{Error, PacketStorage, log};

#[derive(Clone, Copy)]
pub(crate) struct ReadTimestamp {
    pub total_read: usize,
    pub timestamp: Instant,
}

pub struct ModemReader<'context, M: RawMutex, const IO_BUF: usize, const IO_CHUNKS: usize> {
    read: Reader<'context, M, IO_BUF>,
    read_timestamps: Receiver<'context, M, ReadTimestamp, IO_CHUNKS>,
    total_consumed: usize,
    // Length of the line currently returned by read_line
    line_len: Option<usize>,
    line_timestamp: Option<Instant>,
    next_timestamp: Option<ReadTimestamp>,
}

impl<'context, M, const IO_BUF: usize, const IO_CHUNKS: usize>
    ModemReader<'context, M, IO_BUF, IO_CHUNKS>
where
    M: RawMutex,
{
    pub fn new(
        read: Reader<'context, M, IO_BUF>,
        read_timestamps: Receiver<'context, M, ReadTimestamp, IO_CHUNKS>,
    ) -> ModemReader<'context, M, IO_BUF, IO_CHUNKS> {
        ModemReader {
            read,
            read_timestamps,
            total_consumed: 0,
            line_len: None,
            line_timestamp: None,
            next_timestamp: None,
        }
    }

    fn update_read_timestamp(&mut self) {
        loop {
            match &self.next_timestamp {
                Some(ReadTimestamp {
                    total_read,
                    timestamp,
                }) => {
                    if *total_read <= self.total_consumed {
                        self.line_timestamp = Some(*timestamp)
                    } else {
                        break;
                    }
                }
                None => {}
            }
            match self.read_timestamps.try_receive() {
                Ok(ReadTimestamp {
                    total_read,
                    timestamp,
                }) => {
                    self.next_timestamp = Some(ReadTimestamp {
                        total_read,
                        timestamp,
                    });
                }
                Err(embassy_sync::channel::TryReceiveError::Empty) => {
                    break;
                }
            }
        }
    }

    fn consume_line(&mut self, line_len: usize) {
        self.read.consume(line_len);
        self.total_consumed += line_len;
    }

    pub async fn read_line(&mut self) -> Result<(&str, Option<&Instant>), Error> {
        if let Some(line_len) = self.line_len.take() {
            // Remove the previously read line from the buffer
            self.consume_line(line_len);
        }
        const MODEM_INPUT_PROMPT: &str = "> ";
        const LINE_END: &str = "\n";
        loop {
            let buffer = self.read.fill_buf().await;
            self.update_read_timestamp();
            let line_timestamp = self.line_timestamp.as_ref();
            #[cfg(feature = "__trace_io")]
            if !buffer.is_empty() {
                match from_utf8(&buffer) {
                    Ok(line) => log::trace!("CURRENT BUFFER (utf-8) {:?}", line),
                    Err(_) => log::trace!("CURRENT BUFFER (binary) {:?}", buffer.as_slice()),
                }
            }

            if buffer.starts_with(MODEM_INPUT_PROMPT.as_bytes()) {
                // When the modem outputs a "> " without a CRLF, it's expecting input,
                // since there is no CRLF we handle this as a special case.
                // Notably this happens after a CIPSEND command

                self.line_len = Some(MODEM_INPUT_PROMPT.len());

                return Ok((MODEM_INPUT_PROMPT, line_timestamp));
            } else if let Some(position) = buffer
                .windows(LINE_END.len())
                .position(|slice| slice == LINE_END.as_bytes())
            {
                // If we see a line break, the modem has probably sent us a message

                let line_len = position + LINE_END.len();
                let Ok(line) = from_utf8(&buffer[..position]) else {
                    self.consume_line(line_len);
                    return Err(Error::InvalidUtf8);
                };
                log::trace!("RECV LINE: {:?}", line);

                // Ignore empty lines, as well as echoed lines (which end with \r\r\n)
                if line.trim().is_empty() || line.ends_with("\r\r") {
                    self.consume_line(line_len);
                    continue;
                }

                self.line_len = Some(line_len);

                let line = line.trim(); // The modem likes to be inconsistent with white space

                return Ok((line, line_timestamp));
            }
        }
    }

    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if let Some(line_len) = self.line_len.take() {
            // Remove the previously read line from the buffer
            self.consume_line(line_len);
        }
        while !buf.is_empty() {
            match self.read.read(buf).await {
                0 => return Err(Error::Serial),
                n => {
                    self.total_consumed += n;
                    buf = &mut buf[n..];
                }
            }
        }
        self.update_read_timestamp();
        Ok(())
    }
}
