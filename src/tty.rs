use std::mem::MaybeUninit;
use std::os::unix::prelude::*;
use std::path::Path;

use std::{fmt, io};

use libc::{cfsetspeed, speed_t};
use nix::fcntl::OFlag;
use nix::{self, libc, unistd};

fn close(fd: RawFd) {
    let _ = unistd::close(fd);
}

#[derive(Debug)]
pub struct TTYPort {
    fd: RawFd,
    exclusive: bool,
    port_name: Option<String>,
    baud_rate: u32,
}

#[derive(Debug)]
pub struct Error {
    description: String,
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> std::result::Result<(), fmt::Error> {
        fmt.write_str(&self.description)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error {
            description: format!("io::Error: {:?}", e),
        }
    }
}

impl From<nix::Error> for Error {
    fn from(e: nix::Error) -> Self {
        Error {
            description: format!("nix::Error: {:?}", e),
        }
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        io::Error::new(io::ErrorKind::Other, error.description)
    }
}

impl TTYPort {
    pub fn open(path_str: String, baud_rate: u32) -> Result<TTYPort, Error> {
        use nix::libc::{tcgetattr, tcsetattr};

        let path = Path::new(&path_str);
        let fd = nix::fcntl::open(
            path,
            OFlag::O_RDWR | OFlag::O_NOCTTY | OFlag::O_NONBLOCK,
            nix::sys::stat::Mode::empty(),
        )?;

        let mut termios = MaybeUninit::uninit();
        let res = unsafe { tcgetattr(fd, termios.as_mut_ptr()) };
        if let Err(e) = nix::errno::Errno::result(res) {
            close(fd);
            return Err(e.into());
        }
        let mut termios = unsafe { termios.assume_init() };

        {
            termios.c_cflag = libc::CS8 | libc::CREAD | libc::CLOCAL | libc::HUPCL;
            termios.c_lflag &= !(libc::ICANON
                | libc::ECHO
                | libc::ECHOE
                | libc::ECHOK
                | libc::ECHONL
                | libc::ISIG
                | libc::IEXTEN);
            termios.c_oflag &= !(libc::OPOST | libc::ONLCR | libc::OCRNL);
            termios.c_iflag &= !(libc::INLCR | libc::IGNCR | libc::ICRNL | libc::IGNBRK);
            termios.c_cc[libc::VTIME] = 0;
            unsafe { cfsetspeed(&mut termios, baud_rate as speed_t) };
            unsafe { tcsetattr(fd, libc::TCSANOW, &termios) };
            unsafe { libc::tcflush(fd, libc::TCIOFLUSH) };
            nix::fcntl::fcntl(fd, nix::fcntl::F_SETFL(nix::fcntl::OFlag::empty()))?;

            Ok(())
        }
        .map_err(|e: Error| {
            close(fd);
            e
        })?;

        // Return the final port object
        Ok(TTYPort {
            fd,
            exclusive: false,
            port_name: Some(path_str.clone()),
            baud_rate: baud_rate,
        })
    }
}

impl Drop for TTYPort {
    fn drop(&mut self) {
        close(self.fd);
    }
}

impl io::Read for TTYPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        nix::unistd::read(self.fd, buf).map_err(|e| io::Error::from(Error::from(e)))
    }
}

impl io::Write for TTYPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        nix::unistd::write(self.fd, buf).map_err(|e| io::Error::from(Error::from(e)))
    }

    fn flush(&mut self) -> io::Result<()> {
        nix::sys::termios::tcdrain(self.fd)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "flush failed"))
    }
}
