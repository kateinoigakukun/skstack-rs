use core::fmt;
use fmt::Debug;
use log::info;
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
    StrDecode(std::str::Utf8Error),
    Io(std::io::Error),
    TTY(tty::Error),
    Decode(String),
    UnexpectedEvent(SKEvent),
    ExpectOK(String),
}
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        match self {
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

impl SKSTACK {
    pub fn open(path: String) -> Result<Self> {
        let port = TTYPort::open(path, 115_200, std::time::Duration::from_millis(1000))?;
        let reader = std::io::BufReader::new(port);
        Ok(SKSTACK { reader })
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

    pub fn set_password<S: Into<String>>(&mut self, password: S) -> Result<()> {
        let password: String = password.into();
        self.write_str(format!("SKSETPWD {} {}\r\n", password.len(), password))?;
        self.read_line_str()?;
        self.consume_ok()?;
        Ok(())
    }

    pub fn set_rbid<S: Into<String>>(&mut self, id: S) -> Result<()> {
        let id: String = id.into();
        self.write_str(format!("SKSETRBID {}\r\n", id))?;
        self.read_line_str()?;
        self.consume_ok()?;
        Ok(())
    }

    fn write_str(&mut self, str: String) -> Result<usize> {
        print!("{}", str);
        self.write(str.as_bytes())
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        info!("write: {}", {
            if let Ok(str) = std::str::from_utf8(buf) {
                str.to_string()
            } else {
                format!("{:?}", buf)
            }
        });
        let len = self.reader.get_mut().write(buf)?;
        Ok(len)
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
        if let Some(version) = str.strip_prefix("EVER ") {
            return Ok(SKEvent::EVER(version.to_string()));
        }
        return Err(Error::Decode(format!("failed decoding SKEvent: {}", str)));
    }

    fn read_line_str(&mut self) -> Result<String> {
        let bytes = self.read_line()?;
        Ok(std::str::from_utf8(&bytes)?.to_string())
    }

    fn read_line(&mut self) -> Result<Vec<u8>> {
        let mut buf = vec![];
        read_until_crlf(&mut self.reader, &mut buf)?;
        let result: Vec<u8> = buf[..buf.len() - 2].into();
        info!("read: {}", {
            if let Ok(str) = std::str::from_utf8(&result) {
                str.to_string()
            } else {
                format!("{:?}", buf)
            }
        });
        Ok(result)
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
