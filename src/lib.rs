use core::fmt;
use fmt::Debug;
use memchr;

use std::{
    io::{BufRead, Write},
    usize,
};
use tty::TTYPort;
mod tty;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Serial(serialport::Error),
    StrDecode(std::str::Utf8Error),
    Io(std::io::Error),
    TTY(tty::Error),
    Decode(String),
    UnexpectedEvent(SKEvent),
    ExpectOK(String)
}
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        match self {
            Error::Serial(error) => <serialport::Error as fmt::Display>::fmt(error, fmt),
            Error::StrDecode(error) => <std::str::Utf8Error as fmt::Display>::fmt(error, fmt),
            Error::Io(error) => <std::io::Error as fmt::Display>::fmt(error, fmt),
            Error::TTY(error) => <tty::Error as fmt::Display>::fmt(error, fmt),
            Error::Decode(string) => write!(fmt, "{}", string),
            Error::UnexpectedEvent(error) => SKEvent::fmt(error, fmt),
            Error::ExpectOK(string) => write!(fmt, "{}", string),
        }
    }
}

impl std::error::Error for Error {}

impl From<serialport::Error> for Error {
    fn from(error: serialport::Error) -> Self {
        Error::Serial(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(error: std::str::Utf8Error) -> Self {
        Error::StrDecode(error)
    }
}

impl From<tty::Error> for Error {
    fn from(error: tty::Error) -> Self {
        Error::TTY(error)
    }
}

pub struct SKSTACK {
    reader: std::io::BufReader<tty::TTYPort>,
}

#[derive(Debug)]
pub enum SKEvent {
    EVER(String),
}

impl SKEvent {
    fn from(str: String) -> Result<SKEvent> {
        if let Some(version) = str.strip_prefix("EVER ") {
            return Ok(SKEvent::EVER(version.to_string()));
        }
        return Err(Error::Decode(format!("failed decoding SKEvent: {}", str)));
    }
}

impl SKSTACK {
    pub fn open(path: String) -> Result<Self> {
        let port = TTYPort::open(path, 115_200, std::time::Duration::from_millis(1000))?;
        let reader = std::io::BufReader::new(port);
        Ok(SKSTACK { reader })
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let len = self.reader.get_mut().write(buf)?;
        Ok(len)
    }

    pub fn version(&mut self) -> Result<String> {
        self.write(b"SKVER\r\n")?;
        self.read_line_str()?;
        let version = match self.read_event()? {
            SKEvent::EVER(version) => version,
            other => return Err(Error::UnexpectedEvent(other)),
        };
        self.consume_ok()?;
        Ok(version)
    }

    fn consume_ok(&mut self) -> Result<()> {
        let ok = self.read_line_str()?;
        if ok == "OK" {
            Ok(())
        } else {
            Err(Error::ExpectOK(ok))
        }
    }

    fn read_event(&mut self) -> Result<SKEvent> {
        let str = self.read_line_str()?;
        SKEvent::from(str)
    }

    fn read_line_str(&mut self) -> Result<String> {
        let bytes = self.read_line()?;
        Ok(std::str::from_utf8(&bytes)?.to_string())
    }

    fn read_line(&mut self) -> Result<Vec<u8>> {
        let mut buf = vec![];
        read_until_crlf(&mut self.reader, &mut buf)?;
        Ok(buf[..buf.len() - 2].into())
    }
}

/// Read until CRLF
fn read_until_crlf<R: BufRead + ?Sized>(
    r: &mut R,
    buf: &mut Vec<u8>,
) -> std::result::Result<usize, std::io::Error> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr::memchr(b'\r', available) {
                Some(i) if available[i + 1] == b'\n' => {
                    buf.extend_from_slice(&available[..=i + 1]);
                    (true, i + 2)
                }
                Some(_) | None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{read_until_crlf, Result};

    #[test]
    fn test_read_line_zero() -> Result<()> {
        let contents = "\r\n".as_bytes();
        let mut cursor = std::io::Cursor::new(contents);
        let mut buf = vec![];
        read_until_crlf(&mut cursor, &mut buf)?;
        assert_eq!(buf, "\r\n".as_bytes());
        Ok(())
    }

    #[test]
    fn test_read_line() -> Result<()> {
        let contents = "line_content\r\n".as_bytes();
        let mut cursor = std::io::Cursor::new(contents);
        let mut buf = vec![];
        read_until_crlf(&mut cursor, &mut buf)?;
        assert_eq!(buf, "line_content\r\n".as_bytes());
        Ok(())
    }
}
