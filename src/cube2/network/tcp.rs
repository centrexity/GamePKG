//! Host-owned, nonblocking ENet stream sockets. No descriptor crosses the RPC.
use super::{encoded_address, number, MAX_DATAGRAM};
use anyhow::{ensure, Context, Result};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::fd::{AsRawFd, FromRawFd},
    time::Duration,
};

pub(super) struct Socket {
    stream: TcpStream,
    listening: bool,
}
fn checked(result: libc::c_int) -> Result<()> {
    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
fn raw_address(bytes: &[u8]) -> Result<libc::sockaddr_in> {
    let ip: [u8; 4] = bytes
        .get(4..8)
        .context("missing IPv4 address")?
        .try_into()?;
    let port = number(bytes, 8)?;
    ensure!(port <= 65535, "invalid port");
    // SAFETY: zero covers ABI padding before filling all IPv4 fields.
    let mut raw: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    raw.sin_family = libc::AF_INET as _;
    raw.sin_port = (port as u16).to_be();
    raw.sin_addr.s_addr = u32::from_ne_bytes(ip);
    Ok(raw)
}
impl Socket {
    pub fn new() -> Result<Self> {
        // SAFETY: no pointer arguments; immediately adopt successful descriptors.
        let fd = unsafe {
            libc::socket(
                libc::AF_INET,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        checked(fd)?;
        Ok(Self {
            stream: unsafe { TcpStream::from_raw_fd(fd) },
            listening: false,
        })
    }
    pub fn accept(&self) -> Result<(Self, SocketAddr)> {
        ensure!(self.listening, "socket is not listening");
        // SAFETY: null address pointers request no address copy. peer_addr obtains
        // the address from the adopted descriptor; every error closes it.
        let fd = unsafe {
            libc::accept4(
                self.stream.as_raw_fd(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            )
        };
        checked(fd)?;
        let stream = unsafe { TcpStream::from_raw_fd(fd) };
        let peer = stream.peer_addr()?;
        Ok((
            Self {
                stream,
                listening: false,
            },
            peer,
        ))
    }
    fn option(&self, option: u32, value: u32) -> Result<()> {
        match option {
            1 => {
                ensure!(value == 1, "blocking sockets are unavailable");
                self.stream.set_nonblocking(true)?;
            }
            2..=5 => {
                ensure!(
                    if option == 2 || option == 5 {
                        value <= 1
                    } else {
                        value <= 1048576
                    },
                    "invalid socket option"
                );
                let option = match option {
                    2 => libc::SO_BROADCAST,
                    3 => libc::SO_RCVBUF,
                    4 => libc::SO_SNDBUF,
                    _ => libc::SO_REUSEADDR,
                };
                let value = value as libc::c_int;
                // SAFETY: live socket, initialized integer and exact ABI size.
                checked(unsafe {
                    libc::setsockopt(
                        self.stream.as_raw_fd(),
                        libc::SOL_SOCKET,
                        option,
                        (&value as *const libc::c_int).cast(),
                        std::mem::size_of_val(&value) as libc::socklen_t,
                    )
                })?;
            }
            6 | 7 => {
                ensure!(value <= 1000, "socket timeout exceeds limit");
                let timeout = (value != 0).then(|| Duration::from_millis(u64::from(value)));
                if option == 6 {
                    self.stream.set_read_timeout(timeout)?;
                } else {
                    self.stream.set_write_timeout(timeout)?;
                }
            }
            9 => {
                ensure!(value <= 1, "invalid TCP no-delay value");
                self.stream.set_nodelay(value == 1)?;
            }
            10 => {
                ensure!((1..=255).contains(&value), "invalid TTL");
                self.stream.set_ttl(value)?;
            }
            _ => anyhow::bail!("unsupported socket option"),
        }
        Ok(())
    }
    pub fn call(&mut self, op: u32, bytes: &[u8], used: &mut usize) -> Result<Vec<u8>> {
        let fd = self.stream.as_raw_fd();
        match op {
            2 | 10 => {
                ensure!(bytes.len() == 12, "invalid stream address request");
                let raw = raw_address(bytes)?;
                if op == 10 {
                    ensure!(raw.sin_port != 0, "invalid connect port");
                }
                // SAFETY: the live descriptor and initialized IPv4 address have
                // the exact length expected by bind/connect. Both are nonblocking.
                let result = unsafe {
                    if op == 2 {
                        libc::bind(
                            fd,
                            (&raw as *const libc::sockaddr_in).cast(),
                            std::mem::size_of_val(&raw) as libc::socklen_t,
                        )
                    } else {
                        libc::connect(
                            fd,
                            (&raw as *const libc::sockaddr_in).cast(),
                            std::mem::size_of_val(&raw) as libc::socklen_t,
                        )
                    }
                };
                if result < 0
                    && !(op == 10
                        && std::io::Error::last_os_error().raw_os_error()
                            == Some(libc::EINPROGRESS))
                {
                    checked(result)?;
                }
                Ok(Vec::new())
            }
            3 => {
                ensure!(
                    (12..=12 + MAX_DATAGRAM).contains(&bytes.len()) && bytes[4..12] == [0; 8],
                    "invalid stream send"
                );
                ensure!(*used + bytes.len() - 12 <= 1048576, "network rate limited");
                let sent = (&self.stream).write(&bytes[12..])?;
                *used += sent;
                Ok((sent as u32).to_le_bytes().to_vec())
            }
            4 => {
                ensure!(bytes.len() == 8, "invalid stream receive");
                let capacity = number(bytes, 4)? as usize;
                ensure!(
                    (1..=MAX_DATAGRAM).contains(&capacity),
                    "invalid receive capacity"
                );
                // Check before reading so a quota rejection cannot consume bytes.
                ensure!(*used + capacity <= 1048576, "network rate limited");
                let mut reply = vec![0; 8 + capacity];
                let count = (&self.stream).read(&mut reply[8..])?;
                *used += count;
                reply.truncate(8 + count);
                Ok(reply)
            }
            5 => {
                ensure!(bytes.len() == 4, "invalid address request");
                encoded_address(self.stream.local_addr()?)
            }
            8 => {
                ensure!(bytes.len() == 8, "invalid stream poll");
                let wanted = number(bytes, 4)?;
                ensure!(wanted & !3 == 0, "invalid poll flags");
                let mut poll = libc::pollfd {
                    fd,
                    events: (if wanted & 1 != 0 { libc::POLLOUT } else { 0 })
                        | (if wanted & 2 != 0 { libc::POLLIN } else { 0 }),
                    revents: 0,
                };
                // SAFETY: one live initialized pollfd; zero timeout cannot block.
                checked(unsafe { libc::poll(&mut poll, 1, 0) })?;
                let errors = libc::POLLERR | libc::POLLHUP;
                let ready = (if poll.revents & (libc::POLLOUT | errors) != 0 {
                    wanted & 1
                } else {
                    0
                }) | (if poll.revents & (libc::POLLIN | errors) != 0 {
                    wanted & 2
                } else {
                    0
                });
                Ok(ready.to_le_bytes().to_vec())
            }
            9 => {
                ensure!(bytes.len() == 8, "invalid listen request");
                let backlog = number(bytes, 4)?;
                ensure!(
                    backlog == u32::MAX || backlog <= 128,
                    "listen backlog exceeds limit"
                );
                // SAFETY: owned descriptor; -1 is ENet's request for SOMAXCONN,
                // bounded here to 128 pending connections.
                checked(unsafe {
                    libc::listen(
                        fd,
                        if backlog == u32::MAX {
                            128
                        } else {
                            backlog as i32
                        },
                    )
                })?;
                self.listening = true;
                Ok(Vec::new())
            }
            11 => {
                ensure!(bytes.len() == 12, "invalid option request");
                self.option(number(bytes, 4)?, number(bytes, 8)?)?;
                Ok(Vec::new())
            }
            12 => {
                ensure!(bytes.len() == 8, "invalid query request");
                let value = match number(bytes, 4)? {
                    8 => self
                        .stream
                        .take_error()?
                        .map_or(0, |e| e.raw_os_error().unwrap_or(libc::EIO))
                        as u32,
                    9 => u32::from(self.stream.nodelay()?),
                    10 => self.stream.ttl()?,
                    _ => anyhow::bail!("unsupported socket query"),
                };
                Ok(value.to_le_bytes().to_vec())
            }
            13 => {
                ensure!(bytes.len() == 8, "invalid shutdown request");
                let how = match number(bytes, 4)? {
                    0 => std::net::Shutdown::Read,
                    1 => std::net::Shutdown::Write,
                    2 => std::net::Shutdown::Both,
                    _ => anyhow::bail!("invalid shutdown direction"),
                };
                self.stream.shutdown(how)?;
                Ok(Vec::new())
            }
            _ => anyhow::bail!("unsupported TCP operation"),
        }
    }
}
