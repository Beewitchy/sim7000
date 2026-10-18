use core::str::from_utf8;
use embassy_sync::channel::DynamicReceiver;
use embassy_time::Instant;

use crate::pipe::DynamicReader;
use crate::{Error, log};

#[derive(Clone, Copy)]
pub(crate) struct ReadTimestamp {
    pub total_read: usize,
    pub timestamp: Instant,
}

pub struct ModemReader<'r> {
    rx: DynamicReader<'r>,
    rx_timestamps: DynamicReceiver<'r, ReadTimestamp>,
    total_consumed: usize,
    // Length of the line currently returned by rx_line
    line_len: Option<usize>,
    line_timestamp: Option<Instant>,
    next_timestamp: Option<ReadTimestamp>,
}

impl<'r> ModemReader<'r> {
    pub(crate) fn new(
        rx: DynamicReader<'r>,
        rx_timestamps: DynamicReceiver<'r, ReadTimestamp>,
    ) -> ModemReader<'r> {
        ModemReader {
            rx,
            rx_timestamps,
            total_consumed: 0,
            line_len: None,
            line_timestamp: None,
            next_timestamp: None,
        }
    }

    fn update_read_timestamp(&mut self) {
        self.next_timestamp = Self::find_next_timestamp(
            self.next_timestamp,
            self.total_consumed,
            &self.rx_timestamps,
        );
        Self::line_timestamp(
            self.line_timestamp,
            &self.next_timestamp,
            self.total_consumed,
        );
    }

    fn line_timestamp(
        line_timestamp: Option<Instant>,
        next_timestamp: &Option<ReadTimestamp>,
        total_consumed: usize,
    ) -> Option<Instant> {
        match next_timestamp {
            Some(ReadTimestamp {
                total_read,
                timestamp,
            }) => {
                if *total_read <= total_consumed {
                    Some(*timestamp)
                } else {
                    line_timestamp
                }
            }
            None => line_timestamp,
        }
    }

    /// Consume timestamps until caught up to the next unconsumed segment
    fn find_next_timestamp(
        next_timestamp: Option<ReadTimestamp>,
        total_consumed: usize,
        rx_timestamps: &DynamicReceiver<'_, ReadTimestamp>,
    ) -> Option<ReadTimestamp> {
        let mut next_timestamp = next_timestamp;
        loop {
            match &next_timestamp {
                Some(ReadTimestamp {
                    total_read,
                    timestamp: _,
                }) => {
                    if *total_read <= total_consumed {
                        break next_timestamp;
                    } else {
                        break next_timestamp;
                    }
                }
                None => {}
            }
            match rx_timestamps.try_receive() {
                Ok(ReadTimestamp {
                    total_read,
                    timestamp,
                }) => {
                    next_timestamp = Some(ReadTimestamp {
                        total_read,
                        timestamp,
                    });
                }
                Err(embassy_sync::channel::TryReceiveError::Empty) => {
                    break next_timestamp;
                }
            }
        }
    }

    fn consume_line(&mut self, line_len: usize) {
        #[cfg(feature = "__trace_io")]
        log::trace!("CONSUMED LEN {:?}", line_len);
        self.rx.consume(line_len);
        self.total_consumed += line_len;
    }

    pub async fn read_line(&mut self) -> Result<(&str, Option<&Instant>), Error> {
        if let Some(line_len) = self.line_len.take() {
            // Remove the previously read line from the buffer
            self.consume_line(line_len);
        }
        self.update_read_timestamp();
        let mut next_timestamp = self.next_timestamp;
        let rx_timestamps = self.rx_timestamps;
        const MODEM_INPUT_PROMPT: &str = "> ";
        const LINE_END: &str = "\n";
        loop {
            let total_consumed = self.total_consumed;
            let buffer = self.rx.fill_buf().await;
            next_timestamp =
                Self::find_next_timestamp(next_timestamp, total_consumed, &rx_timestamps);
            self.line_timestamp =
                Self::line_timestamp(self.line_timestamp, &next_timestamp, total_consumed);
            let line_timestamp = self.line_timestamp.as_ref();
            #[cfg(feature = "__trace_io")]
            if !buffer.is_empty() {
                match from_utf8(buffer) {
                    Ok(line) => log::debug!("CURRENT BUFFER (utf-8) {:?}", line),
                    Err(_) => log::debug!("CURRENT BUFFER (binary) {:?}", buffer),
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
                    #[cfg(feature = "__trace_io")]
                    log::trace!("INVALID UTF8");
                    self.consume_line(line_len);
                    return Err(Error::InvalidUtf8);
                };
                log::trace!("RECV LINE: {:?}", line);

                // Ignore empty lines, as well as echoed lines (which end with \r\r\n)
                if line.trim().is_empty() || line.ends_with("\r\r") {
                    #[cfg(feature = "__trace_io")]
                    log::trace!("IS EMPTY/ECHO");
                    self.consume_line(line_len);
                    continue;
                }

                self.line_len = Some(line_len);

                let line = line.trim(); // The modem likes to be inconsistent with white space

                return Ok((line, line_timestamp));
            } else {
                // todo: ellie (18.10.2026) - There is currently a sort of pitfall in the embassy
                // Pipe implementation where when the pipe wraps around Reader::fill_buf() will only
                // return the first slice (up to the wrapping location) until it is fully consumed,
                // meaning it cannot be safely used to wait for a the line-end tokens. Other rust
                // ring buffer implementations handle this by returning a pair of slices, but that
                // would make parsing more complicated. So, currently, we just try to shift the
                // buffer to avoid wrapping whenever the currently returned slice doesn't contain a
                // line-end:
                self.rx.shift();
                next_timestamp = Some(rx_timestamps.receive().await);
            }
        }
    }

    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        if let Some(line_len) = self.line_len.take() {
            // Remove the previously read line from the buffer
            self.consume_line(line_len);
        }
        let mut buf = buf;
        while !buf.is_empty() {
            match self.rx.read(buf).await {
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
