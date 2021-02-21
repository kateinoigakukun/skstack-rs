use std::io;
use std::mem::MaybeUninit;
use std::os::unix::prelude::*;
use std::path::Path;
use std::slice;
use std::time::Duration;

use libc::{cfsetspeed, speed_t};
use nix::fcntl::OFlag;
use nix::poll::{PollFd, PollFlags};
use nix::{self, libc, unistd};

fn close(fd: RawFd) {
    let _ = unistd::close(fd);
}

#[derive(Debug)]
pub struct TTYPort {
    fd: RawFd,
    port_name: Option<String>,
    baud_rate: u32,
    timeout: Option<Duration>,
}

pub struct Error(io::Error);

impl From<nix::Error> for Error {
    fn from(err: nix::Error) -> Self {
        match err {
            nix::Error::InvalidPath => Error(io::Error::new(io::ErrorKind::InvalidInput, err)),
            nix::Error::InvalidUtf8 => Error(io::Error::new(io::ErrorKind::Other, err)),
            nix::Error::UnsupportedOperation => Error(io::Error::new(io::ErrorKind::Other, err)),
            nix::Error::Sys(errno) => Error(io::Error::from_raw_os_error(errno as i32)),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self(err)
    }
}
impl Into<io::Error> for Error {
    fn into(self) -> io::Error {
        self.0
    }
}

impl TTYPort {
    pub fn open(
        path_str: String,
        baud_rate: u32,
        timeout: Option<Duration>,
    ) -> Result<TTYPort, Error> {
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

        Ok(TTYPort {
            fd,
            port_name: Some(path_str.clone()),
            baud_rate: baud_rate,
            timeout: timeout,
        })
    }
    pub fn set_timeout(&mut self, timeout: Option<Duration>) {
        self.timeout = timeout;
    }
}

impl Drop for TTYPort {
    fn drop(&mut self) {
        close(self.fd);
    }
}

impl io::Read for TTYPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(timeout) = self.timeout {
            if let Err(e) = wait_read_fd(self.fd, timeout) {
                return Err(Error::from(e).into());
            }
        }
        nix::unistd::read(self.fd, buf).map_err(|e| Error::from(e).into())
    }
}

impl io::Write for TTYPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(timeout) = self.timeout {
            if let Err(e) = wait_write_fd(self.fd, timeout) {
                return Err(e.into());
            }
        }
        nix::unistd::write(self.fd, buf).map_err(|e| Error::from(e).into())
    }

    fn flush(&mut self) -> io::Result<()> {
        nix::sys::termios::tcdrain(self.fd)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "flush failed"))
    }
}

fn wait_read_fd(fd: RawFd, timeout: Duration) -> Result<(), Error> {
    wait_fd(fd, PollFlags::POLLIN, timeout)
}

fn wait_write_fd(fd: RawFd, timeout: Duration) -> Result<(), Error> {
    wait_fd(fd, PollFlags::POLLOUT, timeout)
}

fn wait_fd(fd: RawFd, events: PollFlags, timeout: Duration) -> Result<(), Error> {
    use nix::errno::Errno::{EIO, EPIPE};

    let mut fd = PollFd::new(fd, events);

    let milliseconds =
        timeout.as_secs() as i64 * 1000 + i64::from(timeout.subsec_nanos()) / 1_000_000;
    let wait_res = nix::poll::poll(slice::from_mut(&mut fd), milliseconds as nix::libc::c_int);

    let wait = match wait_res {
        Ok(r) => r,
        Err(e) => return Err(Error::from(e)),
    };
    if wait != 1 {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Operation timed out").into());
    }

    match fd.revents() {
        Some(e) if e == events => return Ok(()),
        Some(e) if e.contains(PollFlags::POLLHUP) || e.contains(PollFlags::POLLNVAL) => {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, EPIPE.desc()).into());
        }
        Some(_) | None => (),
    }

    Err(io::Error::new(io::ErrorKind::Other, EIO.desc()).into())
}
