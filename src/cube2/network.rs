//! ENet's sockets remain host-owned; the reviewed engine receives virtual handles.
mod tcp;
use anyhow::{ensure, Context, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const MAX_PACKET: usize = 8192;
const MAX_DATAGRAM: usize = 4096;
const MAX_HANDLES: usize = 16;
#[derive(Clone)]
pub(super) struct Config {
    pub permissions: crate::manifest::Permissions,
    pub master: Option<super::discovery::Master>,
}
impl From<crate::manifest::Permissions> for Config {
    fn from(permissions: crate::manifest::Permissions) -> Self {
        Self {
            permissions,
            master: None,
        }
    }
}
struct MasterSocket {
    connected: bool,
    request: usize,
    response: Option<Vec<u8>>,
    cursor: usize,
    read_closed: bool,
    write_closed: bool,
}
static WORKERS: AtomicUsize = AtomicUsize::new(0);
struct Slot;
impl Drop for Slot {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::SeqCst);
    }
}
pub(super) struct Worker {
    control: UnixStream,
    done: mpsc::Receiver<()>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.control.shutdown(std::net::Shutdown::Both);
        let _ = self.done.recv_timeout(Duration::from_millis(100));
    }
}
struct Socket {
    udp: UdpSocket,
    connected: Option<SocketAddr>,
}
// ENet option numbers are protocol values, never raw platform constants supplied
// by the engine. Descriptors stay owned by this process.
fn set_option(socket: &UdpSocket, option: u32, value: u32) -> Result<()> {
    match option {
        1 => {
            ensure!(value == 1, "blocking sockets are unavailable");
            socket.set_nonblocking(true)?;
        }
        2 => {
            ensure!(value <= 1, "invalid broadcast option");
            socket.set_broadcast(value == 1)?;
        }
        3..=5 => {
            ensure!(
                if option == 5 {
                    value <= 1
                } else {
                    value <= 1048576
                },
                "invalid socket option value"
            );
            let name = match option {
                3 => libc::SO_RCVBUF,
                4 => libc::SO_SNDBUF,
                _ => libc::SO_REUSEADDR,
            };
            let value = value as libc::c_int;
            use std::os::fd::AsRawFd;
            // SAFETY: a live owned UDP socket and correctly sized initialized int.
            let result = unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    name,
                    (&value as *const libc::c_int).cast(),
                    std::mem::size_of_val(&value) as libc::socklen_t,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        6 | 7 => {
            ensure!(value <= 1000, "socket timeout exceeds limit");
            let timeout = (value != 0).then(|| Duration::from_millis(u64::from(value)));
            if option == 6 {
                socket.set_read_timeout(timeout)?;
            } else {
                socket.set_write_timeout(timeout)?;
            }
        }
        10 => {
            ensure!((1..=255).contains(&value), "invalid TTL");
            socket.set_ttl(value)?;
        }
        _ => anyhow::bail!("unsupported socket option"),
    }
    Ok(())
}
fn unbound_udp() -> Result<UdpSocket> {
    use std::os::fd::FromRawFd;
    // SAFETY: adopt every successful descriptor immediately, closing on error.
    let fd = unsafe {
        libc::socket(
            libc::AF_INET,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            0,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { UdpSocket::from_raw_fd(fd) })
}
fn bind_socket(socket: &UdpSocket, address: SocketAddr) -> Result<()> {
    use std::os::fd::AsRawFd;
    let SocketAddr::V4(address) = address else {
        anyhow::bail!("IPv4 required");
    };
    // Zero initialization covers platform padding; all meaningful IPv4 fields
    // are then filled in native/network order as required by the socket ABI.
    let mut raw: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    raw.sin_family = libc::AF_INET as _;
    raw.sin_port = address.port().to_be();
    raw.sin_addr.s_addr = u32::from_ne_bytes(address.ip().octets());
    let result = unsafe {
        libc::bind(
            socket.as_raw_fd(),
            (&raw as *const libc::sockaddr_in).cast(),
            std::mem::size_of_val(&raw) as libc::socklen_t,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
struct State {
    master: Option<super::discovery::Master>,
    streams: BTreeMap<u32, MasterSocket>,
    fetching: Option<mpsc::Receiver<Result<super::discovery::List>>>,
    list: Option<Vec<u8>>,
    refreshed: Option<Instant>,
    attempted: Option<Instant>,
    discovered: BTreeSet<SocketAddr>,
    enabled: bool,
    sockets: BTreeMap<u32, Socket>,
    tcp: BTreeMap<u32, tcp::Socket>,
    next: u32,
    window: Instant,
    calls: usize,
    bytes: usize,
    opens: usize,
}
fn number(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("truncated network integer")?
            .try_into()?,
    ))
}
fn address(bytes: &[u8], offset: usize) -> Result<SocketAddr> {
    let octets: [u8; 4] = bytes
        .get(offset..offset + 4)
        .context("missing IPv4 address")?
        .try_into()?;
    let port = number(bytes, offset + 4)?;
    ensure!((1..=65535).contains(&port), "invalid port");
    Ok(SocketAddrV4::new(Ipv4Addr::from(octets), port as u16).into())
}
fn encoded_address(address: SocketAddr) -> Result<Vec<u8>> {
    let SocketAddr::V4(address) = address else {
        anyhow::bail!("ENet requires IPv4");
    };
    let mut bytes = address.ip().octets().to_vec();
    bytes.extend(u32::from(address.port()).to_le_bytes());
    Ok(bytes)
}
impl State {
    fn configured(config: Config) -> Result<Self> {
        ensure!(
            config.master.is_none() || config.permissions.network,
            "master discovery requires networking enabled"
        );
        let mut state = Self::new(config.permissions)?;
        state.master = config.master;
        state.begin_refresh()?;
        Ok(state)
    }
    fn new(p: crate::manifest::Permissions) -> Result<Self> {
        crate::network::validate(&p)?;
        let enabled = p.network;
        Ok(Self {
            master: None,
            streams: BTreeMap::new(),
            fetching: None,
            list: None,
            refreshed: None,
            attempted: None,
            discovered: BTreeSet::new(),
            enabled,
            sockets: BTreeMap::new(),
            tcp: BTreeMap::new(),
            next: 32,
            window: Instant::now(),
            calls: 0,
            bytes: 0,
            opens: 0,
        })
    }
    fn dispatch(&mut self, op: u32, bytes: &[u8]) -> Result<Vec<u8>> {
        self.collect_master();
        if self.window.elapsed() >= Duration::from_secs(1) {
            self.window = Instant::now();
            self.calls = 0;
            self.bytes = 0;
            self.opens = 0;
        }
        self.calls += 1;
        ensure!(
            self.calls <= 2048 && self.bytes <= 1048576,
            "network rate limited"
        );
        ensure!(self.enabled, "network permission denied");
        if op == 16 {
            ensure!(bytes.is_empty(), "invalid snapshot socket request");
            ensure!(
                self.sockets.len() + self.streams.len() + self.tcp.len() < MAX_HANDLES
                    && self.next < 1024
                    && self.opens < 8,
                "network handle limit"
            );
            let id = self.next;
            self.next += 1;
            self.opens += 1;
            self.streams.insert(
                id,
                MasterSocket {
                    connected: false,
                    request: 0,
                    response: None,
                    cursor: 0,
                    read_closed: false,
                    write_closed: false,
                },
            );
            return Ok(id.to_le_bytes().to_vec());
        }
        if op != 1 && bytes.len() >= 4 && self.streams.contains_key(&number(bytes, 0)?) {
            return self.stream_call(op, bytes);
        }
        if bytes.len() >= 4 && self.tcp.contains_key(&number(bytes, 0)?) {
            let id = number(bytes, 0)?;
            if op == 7 {
                ensure!(bytes.len() == 4, "invalid stream close");
                self.tcp.remove(&id);
                return Ok(Vec::new());
            }
            if op == 14 {
                ensure!(bytes.len() == 4, "invalid accept request");
                ensure!(
                    self.sockets.len() + self.streams.len() + self.tcp.len() < MAX_HANDLES
                        && self.next < 1024
                        && self.opens < 8,
                    "network handle limit"
                );
                let (socket, peer) = self.tcp[&id].accept()?;
                let accepted = self.next;
                self.next += 1;
                self.opens += 1;
                self.tcp.insert(accepted, socket);
                let mut reply = accepted.to_le_bytes().to_vec();
                reply.extend(encoded_address(peer)?);
                return Ok(reply);
            }
            return self
                .tcp
                .get_mut(&id)
                .unwrap()
                .call(op, bytes, &mut self.bytes);
        }
        match op {
            1 => {
                ensure!(
                    bytes.len() == 4 && matches!(number(bytes, 0)?, 1 | 2),
                    "invalid socket type"
                );
                ensure!(
                    self.sockets.len() + self.streams.len() + self.tcp.len() < MAX_HANDLES
                        && self.next < 1024
                        && self.opens < 8,
                    "network handle limit"
                );
                if number(bytes, 0)? == 1 {
                    let socket = tcp::Socket::new()?;
                    let id = self.next;
                    self.next += 1;
                    self.opens += 1;
                    self.tcp.insert(id, socket);
                    return Ok(id.to_le_bytes().to_vec());
                }
                let udp = unbound_udp()?;
                udp.set_nonblocking(true)?;
                let id = self.next;
                self.next += 1;
                self.opens += 1;
                self.sockets.insert(
                    id,
                    Socket {
                        udp,
                        connected: None,
                    },
                );
                Ok(id.to_le_bytes().to_vec())
            }
            2 => {
                ensure!(bytes.len() == 12, "invalid bind request");
                let id = number(bytes, 0)?;
                ensure!(self.sockets.contains_key(&id), "unknown socket");
                let port = number(bytes, 8)?;
                ensure!(port <= 65535, "invalid bind port");
                let ip: [u8; 4] = bytes[4..8].try_into()?;
                let requested = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(ip), port as u16));
                bind_socket(&self.sockets[&id].udp, requested)?;
                Ok(Vec::new())
            }
            3 => {
                ensure!(
                    (12..=12 + MAX_DATAGRAM).contains(&bytes.len()),
                    "invalid datagram size"
                );
                let id = number(bytes, 0)?;
                let socket = self.sockets.get_mut(&id).context("unknown socket")?;
                let to = if bytes[4..12] == [0; 8] {
                    socket.connected.context("socket is not connected")?
                } else {
                    address(bytes, 4)?
                };
                let size = bytes.len() - 12;
                ensure!(self.bytes + size <= 1048576, "network rate limited");
                let sent = socket.udp.send_to(&bytes[12..], to)?;
                self.bytes += sent;
                Ok((sent as u32).to_le_bytes().to_vec())
            }
            4 => {
                ensure!(bytes.len() == 8, "invalid receive request");
                let capacity = number(bytes, 4)? as usize;
                ensure!(
                    (1..=MAX_DATAGRAM).contains(&capacity),
                    "invalid receive capacity"
                );
                let socket = self
                    .sockets
                    .get_mut(&number(bytes, 0)?)
                    .context("unknown socket")?;
                ensure!(self.bytes + capacity <= 1048576, "network rate limited");
                let mut packet = [0; MAX_DATAGRAM + 1];
                for _ in 0..8 {
                    let (size, from) = socket.udp.recv_from(&mut packet)?;
                    if size > capacity || size > MAX_DATAGRAM {
                        continue;
                    }
                    ensure!(self.bytes + size <= 1048576, "network rate limited");
                    self.bytes += size;
                    let mut out = encoded_address(from)?;
                    out.extend(&packet[..size]);
                    return Ok(out);
                }
                Err(std::io::Error::from(std::io::ErrorKind::WouldBlock).into())
            }
            5 => {
                ensure!(bytes.len() == 4, "invalid address request");
                encoded_address(
                    self.sockets
                        .get(&number(bytes, 0)?)
                        .context("unknown socket")?
                        .udp
                        .local_addr()?,
                )
            }
            6 => {
                ensure!(!bytes.is_empty() && bytes.len() <= 253, "invalid hostname");
                let name = std::str::from_utf8(bytes)?;
                ensure!(
                    name.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-.".contains(&c)),
                    "invalid hostname"
                );
                let address = crate::network::resolve_engine_host(name)?;
                Ok(encoded_address(address)?[..4].to_vec())
            }
            7 => {
                ensure!(bytes.len() == 4, "invalid close request");
                ensure!(
                    self.sockets.remove(&number(bytes, 0)?).is_some(),
                    "unknown socket"
                );
                Ok(Vec::new())
            }
            8 => {
                ensure!(bytes.len() == 8, "invalid poll request");
                let wanted = number(bytes, 4)?;
                ensure!(wanted & !3 == 0, "invalid poll flags");
                let socket = self
                    .sockets
                    .get(&number(bytes, 0)?)
                    .context("unknown socket")?;
                let mut ready = wanted & 1;
                if wanted & 2 != 0 {
                    let mut b = [0; 1];
                    if socket.udp.peek_from(&mut b).is_ok() {
                        ready |= 2;
                    }
                }
                Ok(ready.to_le_bytes().to_vec())
            }
            11 => {
                ensure!(bytes.len() == 12, "invalid set-option request");
                let option = number(bytes, 4)?;
                let value = number(bytes, 8)?;
                let socket = self
                    .sockets
                    .get_mut(&number(bytes, 0)?)
                    .context("unknown socket")?;
                set_option(&socket.udp, option, value)?;
                Ok(Vec::new())
            }
            12 => {
                ensure!(bytes.len() == 8, "invalid get-option request");
                let socket = self
                    .sockets
                    .get(&number(bytes, 0)?)
                    .context("unknown socket")?;
                let value = match number(bytes, 4)? {
                    8 => socket
                        .udp
                        .take_error()?
                        .map_or(0, |e| e.raw_os_error().unwrap_or(libc::EIO))
                        as u32,
                    10 => socket.udp.ttl()?,
                    _ => anyhow::bail!("unsupported socket query"),
                };
                Ok(value.to_le_bytes().to_vec())
            }
            10 => {
                ensure!(bytes.len() == 12, "invalid connect request");
                let peer = address(bytes, 4)?;
                let socket = self
                    .sockets
                    .get_mut(&number(bytes, 0)?)
                    .context("unknown socket")?;
                socket.udp.connect(peer)?;
                socket.connected = Some(peer);
                Ok(Vec::new())
            }
            13 => {
                ensure!(bytes.len() == 8, "invalid shutdown request");
                let how = number(bytes, 4)?;
                ensure!(how <= 2, "invalid shutdown direction");
                use std::os::fd::AsRawFd;
                let socket = &self
                    .sockets
                    .get(&number(bytes, 0)?)
                    .context("unknown socket")?
                    .udp;
                // SAFETY: live owned socket and validated ENet shutdown direction.
                if unsafe { libc::shutdown(socket.as_raw_fd(), how as i32) } != 0 {
                    return Err(std::io::Error::last_os_error().into());
                }
                Ok(Vec::new())
            }
            _ => anyhow::bail!("unknown network operation"),
        }
    }
    fn begin_refresh(&mut self) -> Result<()> {
        if self.fetching.is_some()
            || self
                .attempted
                .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
        {
            return Ok(());
        }
        let Some(master) = self.master.clone() else {
            return Ok(());
        };
        self.attempted = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("cube2-master".into())
            .spawn(move || {
                let _ = tx.send(master.fetch());
            })?;
        self.fetching = Some(rx);
        Ok(())
    }
    fn collect_master(&mut self) {
        if self
            .refreshed
            .is_some_and(|t| t.elapsed() > Duration::from_secs(600))
        {
            self.discovered.clear();
            self.list = None;
            self.refreshed = None;
        }
        match self.fetching.as_ref().map(|r| r.try_recv()) {
            Some(Ok(result)) => {
                self.fetching = None;
                if let Ok(list) = result {
                    self.discovered = list.endpoints;
                    self.list = Some(list.bytes);
                    self.refreshed = Some(Instant::now());
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.fetching = None;
            }
            _ => {}
        }
        // Discovery is asynchronous. The engine reads a bounded last-good
        // snapshot immediately, so a slow master cannot consume a game tick.
        let _ = self.begin_refresh();
    }
    fn stream_call(&mut self, op: u32, bytes: &[u8]) -> Result<Vec<u8>> {
        let id = number(bytes, 0)?;
        match op {
            11 => {
                ensure!(
                    bytes.len() == 12 && number(bytes, 4)? == 1 && number(bytes, 8)? == 1,
                    "discovery streams only support nonblocking mode"
                );
                Ok(Vec::new())
            }
            12 => {
                ensure!(
                    bytes.len() == 8 && number(bytes, 4)? == 8,
                    "invalid discovery query"
                );
                // This is a virtual snapshot connection, not an OS TCP socket.
                ensure!(
                    self.streams[&id].connected,
                    "discovery stream is not connected"
                );
                Ok(0u32.to_le_bytes().to_vec())
            }
            10 => {
                ensure!(bytes.len() == 12, "invalid master connect");
                let peer = address(bytes, 4)?;
                ensure!(!self.streams[&id].connected, "already connected");
                if self.master.as_ref().map(|m| m.address) != Some(peer) {
                    // Drop the old receiver so a late result cannot populate the
                    // new master's snapshot. Outstanding work keeps its deadline.
                    self.fetching = None;
                    self.attempted = None;
                    self.master = Some(super::discovery::Master { address: peer });
                    self.list = None;
                    self.discovered.clear();
                    self.refreshed = None;
                }
                self.begin_refresh()?;
                let s = self.streams.get_mut(&id).unwrap();
                ensure!(!s.connected, "already connected");
                s.connected = true;
                Ok(Vec::new())
            }
            3 => {
                ensure!(
                    (12..=17).contains(&bytes.len()) && bytes[4..12] == [0; 8],
                    "master only accepts list request"
                );
                let s = self.streams.get_mut(&id).unwrap();
                ensure!(
                    s.connected && !s.write_closed && s.request < 5,
                    "invalid master state"
                );
                let part = &bytes[12..];
                ensure!(
                    !part.is_empty()
                        && s.request + part.len() <= 5
                        && part == &b"list\n"[s.request..s.request + part.len()],
                    "master only accepts list request"
                );
                s.request += part.len();
                if s.request == 5 {
                    s.response = Some(self.list.clone().unwrap_or_default());
                }
                Ok((part.len() as u32).to_le_bytes().to_vec())
            }
            4 => {
                ensure!(bytes.len() == 8, "invalid master receive");
                let capacity = number(bytes, 4)? as usize;
                ensure!(
                    (1..=MAX_DATAGRAM).contains(&capacity),
                    "invalid receive capacity"
                );
                let s = self.streams.get_mut(&id).unwrap();
                ensure!(s.connected, "master retrieval failed");
                if s.read_closed {
                    return Ok(vec![0; 8]);
                }
                let data = s
                    .response
                    .as_ref()
                    .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
                let end = (s.cursor + capacity).min(data.len());
                ensure!(
                    self.bytes + end - s.cursor <= 1048576,
                    "network rate limited"
                );
                let mut out = vec![0; 8];
                out.extend_from_slice(&data[s.cursor..end]);
                self.bytes += end - s.cursor;
                s.cursor = end;
                Ok(out)
            }
            8 => {
                ensure!(bytes.len() == 8, "invalid master poll");
                let wanted = number(bytes, 4)?;
                ensure!(wanted & !3 == 0, "invalid poll flags");
                let s = &self.streams[&id];
                let ready = if s.connected {
                    wanted & 1
                        | if s.response.is_some() || s.read_closed {
                            wanted & 2
                        } else {
                            0
                        }
                } else {
                    0
                };
                Ok(ready.to_le_bytes().to_vec())
            }
            7 => {
                ensure!(bytes.len() == 4, "invalid master close");
                self.streams.remove(&id);
                Ok(Vec::new())
            }
            13 => {
                ensure!(bytes.len() == 8, "invalid snapshot shutdown");
                let how = number(bytes, 4)?;
                ensure!(how <= 2, "invalid shutdown direction");
                let s = self.streams.get_mut(&id).unwrap();
                ensure!(s.connected, "snapshot is not connected");
                if how == 0 || how == 2 {
                    s.read_closed = true;
                }
                if how == 1 || how == 2 {
                    s.write_closed = true;
                }
                Ok(Vec::new())
            }
            _ => anyhow::bail!("operation not supported for master discovery"),
        }
    }
    fn call(&mut self, op: u32, bytes: &[u8]) -> (u32, Vec<u8>) {
        match self.dispatch(op, bytes) {
            Ok(bytes) => (0, bytes),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::WouldBlock)
                    || error.to_string() == "network rate limited" =>
            {
                (2, Vec::new())
            }
            Err(_) => (1, Vec::new()),
        }
    }
}

pub(super) fn start(config: Config) -> Result<(Worker, UnixStream)> {
    let previous = WORKERS.fetch_add(1, Ordering::SeqCst);
    if previous >= 16 {
        WORKERS.fetch_sub(1, Ordering::SeqCst);
        anyhow::bail!("too many Cube 2 network workers");
    }
    let slot = Slot;
    // The broker owns all sockets; the engine receives only bounded RPC replies.
    let mut state = State::configured(config)?;
    let (mut host, child) = UnixStream::pair()?;
    host.set_read_timeout(Some(Duration::from_millis(250)))?;
    host.set_write_timeout(Some(Duration::from_secs(1)))?;
    child.set_read_timeout(Some(Duration::from_secs(2)))?;
    child.set_write_timeout(Some(Duration::from_secs(2)))?;
    let control = host.try_clone()?;
    let (send, done) = mpsc::channel();
    std::thread::Builder::new()
        .name("cube2-network".into())
        .spawn(move || {
            let _slot = slot;
            let _ = (|| -> Result<()> {
                loop {
                    let mut header = [0; 16];
                    // A timed-out partial header terminates the connection; never
                    // restart framing in the middle of attacker-controlled bytes.
                    match host.read(&mut header[..1]) {
                        Ok(0) => return Ok(()),
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            continue
                        }
                        Err(e) => return Err(e.into()),
                        Ok(_) => {}
                    }
                    host.read_exact(&mut header[1..])?;
                    ensure!(
                        &header[..4] == b"GPN1" && number(&header, 12)? == 0,
                        "invalid network request header"
                    );
                    let length = number(&header, 8)? as usize;
                    ensure!(length <= MAX_PACKET, "oversized network request");
                    let mut bytes = vec![0; length];
                    host.read_exact(&mut bytes)?;
                    let (status, reply) = state.call(number(&header, 4)?, &bytes);
                    ensure!(state.calls <= 4096, "network request flood");
                    let mut out = b"GPR2".to_vec();
                    out.extend(status.to_le_bytes());
                    out.extend((reply.len() as u32).to_le_bytes());
                    out.extend([0; 4]);
                    host.write_all(&out)?;
                    host.write_all(&reply)?;
                }
            })();
            // The owner holds a control clone for cancellation. Shut down the
            // shared endpoint explicitly so EOF also reaches the engine when
            // this worker rejects a malformed request and returns first.
            let _ = host.shutdown(std::net::Shutdown::Both);
            let _ = send.send(());
        })?;
    Ok((Worker { control, done }, child))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn permissions(
        hosts: Vec<String>,
        listen: Vec<String>,
        local: bool,
    ) -> crate::manifest::Permissions {
        crate::manifest::Permissions {
            files: Vec::new(),
            input: true,
            audio: true,
            save: true,
            network: true,
            network_hosts: hosts,
            network_listen: listen,
            network_allow_local: local,
        }
    }
    fn create(state: &mut State) -> u32 {
        number(&state.dispatch(1, &2u32.to_le_bytes()).unwrap(), 0).unwrap()
    }
    fn destination(id: u32, address: SocketAddr) -> Vec<u8> {
        let mut b = id.to_le_bytes().to_vec();
        b.extend(encoded_address(address).unwrap());
        b
    }
    fn create_tcp(state: &mut State) -> u32 {
        number(&state.dispatch(1, &1u32.to_le_bytes()).unwrap(), 0).unwrap()
    }
    fn pair(id: u32, value: u32) -> Vec<u8> {
        [id.to_le_bytes(), value.to_le_bytes()].concat()
    }
    fn retry(state: &mut State, op: u32, bytes: &[u8]) -> Vec<u8> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match state.call(op, bytes) {
                (0, reply) => return reply,
                (2, _) => {
                    assert!(Instant::now() < deadline, "operation {op} timed out");
                    std::thread::sleep(Duration::from_millis(2));
                }
                other => panic!("operation {op} failed: {other:?}"),
            }
        }
    }
    fn connected(state: &mut State, id: u32) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(3);
        while number(&state.dispatch(8, &pair(id, 3)).unwrap(), 0).unwrap() == 0 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        number(&state.dispatch(12, &pair(id, 8)).unwrap(), 0).unwrap()
    }
    #[test]
    fn tcp_connect_partial_reads_options_half_close_and_eof_reach_real_sockets() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let target = listener.local_addr().unwrap();
        let expected = (0..12001).map(|n| (n % 251) as u8).collect::<Vec<_>>();
        let sent = expected.clone();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"auth\n");
            assert_eq!(
                stream.read(&mut [0; 1]).unwrap(),
                0,
                "write shutdown did not reach peer"
            );
            for chunk in sent.chunks(137) {
                stream.write_all(chunk).unwrap();
            }
        });
        let mut state = State::new(permissions(Vec::new(), Vec::new(), false)).unwrap();
        let id = create_tcp(&mut state);
        for (option, value) in [
            (1u32, 1u32),
            (3, 32768),
            (4, 32768),
            (5, 1),
            (6, 500),
            (7, 500),
            (9, 1),
            (10, 42),
        ] {
            let request = [id.to_le_bytes(), option.to_le_bytes(), value.to_le_bytes()].concat();
            state.dispatch(11, &request).unwrap();
        }
        state.dispatch(10, &destination(id, target)).unwrap();
        assert_eq!(connected(&mut state, id), 0);
        assert_eq!(
            state.dispatch(12, &pair(id, 9)).unwrap(),
            1u32.to_le_bytes()
        );
        assert_eq!(
            state.dispatch(12, &pair(id, 10)).unwrap(),
            42u32.to_le_bytes()
        );
        let mut request = pair(id, 0);
        request.extend([0; 4]);
        request.extend(b"auth\n");
        assert_eq!(retry(&mut state, 3, &request), 5u32.to_le_bytes());
        state.dispatch(13, &pair(id, 1)).unwrap();
        assert!(state.dispatch(3, &request).is_err());
        let mut received = Vec::new();
        while received.len() < expected.len() {
            let reply = retry(&mut state, 4, &pair(id, 257));
            assert!(reply.len() > 8);
            received.extend_from_slice(&reply[8..]);
        }
        assert_eq!(received, expected);
        assert_eq!(retry(&mut state, 4, &pair(id, 257)), [0; 8]);
        worker.join().unwrap();
        state.dispatch(7, &id.to_le_bytes()).unwrap();
        assert!(state.dispatch(13, &pair(id, 2)).is_err());
    }
    #[test]
    fn tcp_listener_accept_peer_identity_quota_preservation_and_shutdown_are_real() {
        let mut state = State::new(permissions(Vec::new(), Vec::new(), false)).unwrap();
        let listener = create_tcp(&mut state);
        state
            .dispatch(2, &destination(listener, "127.0.0.1:0".parse().unwrap()))
            .unwrap();
        state.dispatch(9, &pair(listener, u32::MAX)).unwrap();
        assert_eq!(state.call(14, &listener.to_le_bytes()).0, 2);
        let target = address(&state.dispatch(5, &listener.to_le_bytes()).unwrap(), 0).unwrap();
        let mut peer = std::net::TcpStream::connect(target).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        peer.write_all(b"data").unwrap();
        state.opens = 8;
        assert_eq!(state.call(14, &listener.to_le_bytes()).0, 1);
        state.window = Instant::now() - Duration::from_secs(2);
        let accepted = retry(&mut state, 14, &listener.to_le_bytes());
        assert_eq!(address(&accepted, 4).unwrap(), peer.local_addr().unwrap());
        let id = number(&accepted, 0).unwrap();
        state.bytes = 1048576;
        assert_eq!(state.call(4, &pair(id, 4)).0, 2);
        state.bytes = 0;
        assert_eq!(&retry(&mut state, 4, &pair(id, 4))[8..], b"data");
        let mut reply = pair(id, 0);
        reply.extend([0; 4]);
        reply.extend(b"pong");
        state.dispatch(3, &reply).unwrap();
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"pong");
        state.dispatch(13, &pair(id, 0)).unwrap();
        assert_eq!(state.dispatch(4, &pair(id, 4)).unwrap(), [0; 8]);
        reply[12..].copy_from_slice(b"next");
        state.dispatch(3, &reply).unwrap();
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"next", "read shutdown also closed writes");
        state.dispatch(13, &pair(id, 2)).unwrap();
        assert_eq!(peer.read(&mut bytes).unwrap(), 0);
        for (op, packet) in [
            (9, pair(id, 129)),
            (13, pair(id, 3)),
            (8, pair(id, 4)),
            (11, [pair(id, 9), 2u32.to_le_bytes().to_vec()].concat()),
        ] {
            assert!(state.dispatch(op, &packet).is_err());
            let mut oversized = packet;
            oversized.push(0);
            assert!(state.dispatch(op, &oversized).is_err());
        }
        state.dispatch(7, &id.to_le_bytes()).unwrap();
        state.dispatch(7, &listener.to_le_bytes()).unwrap();
        assert!(state.tcp.is_empty());
    }
    #[test]
    fn tcp_connect_refusal_queries_the_os_and_offline_rejects_before_opening() {
        let mut held = tcp::Socket::new().unwrap();
        let bind = destination(32, "127.0.0.1:0".parse().unwrap());
        held.call(2, &bind, &mut 0).unwrap();
        let target = address(&held.call(5, &32u32.to_le_bytes(), &mut 0).unwrap(), 0).unwrap();
        let mut state = State::new(permissions(Vec::new(), Vec::new(), false)).unwrap();
        let id = create_tcp(&mut state);
        state.dispatch(10, &destination(id, target)).unwrap();
        assert_eq!(connected(&mut state, id), libc::ECONNREFUSED as u32);
        assert_eq!(
            state.dispatch(12, &pair(id, 8)).unwrap(),
            0u32.to_le_bytes()
        );
        let mut p = permissions(Vec::new(), Vec::new(), false);
        p.network = false;
        let mut offline = State::new(p).unwrap();
        for (op, bytes) in [
            (1, 1u32.to_le_bytes().to_vec()),
            (16, Vec::new()),
            (9, pair(32, 1)),
            (13, pair(32, 2)),
            (14, 32u32.to_le_bytes().to_vec()),
        ] {
            assert_eq!(offline.call(op, &bytes).0, 1);
        }
        assert!(offline.tcp.is_empty() && offline.streams.is_empty());
        assert_eq!(offline.next, 32);
    }
    #[test]
    fn udp_bind_connect_and_shutdown_preserve_original_socket_semantics() {
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let target = peer.local_addr().unwrap();
        let mut state = State::new(permissions(Vec::new(), Vec::new(), false)).unwrap();
        let id = create(&mut state);
        assert_eq!(
            number(&state.dispatch(5, &id.to_le_bytes()).unwrap(), 4).unwrap(),
            0
        );
        let bind = destination(id, "127.0.0.1:0".parse().unwrap());
        state.dispatch(2, &bind).unwrap();
        assert!(
            state.dispatch(2, &bind).is_err(),
            "second bind silently replaced a live socket"
        );
        state.dispatch(10, &destination(id, target)).unwrap();
        let mut request = pair(id, 0);
        request.extend([0; 4]);
        request.extend(b"hello");
        state.dispatch(3, &request).unwrap();
        let mut bytes = [0; 16];
        let (length, _) = peer.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..length], b"hello");
        state.dispatch(13, &pair(id, 1)).unwrap();
        assert!(state.dispatch(3, &request).is_err());
        assert!(peer.recv_from(&mut bytes).is_err());
    }
    #[test]
    fn changing_snapshot_master_discards_late_results_and_shutdown_has_eof_semantics() {
        let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let second = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let a = first.local_addr().unwrap();
        let b = second.local_addr().unwrap();
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let old = std::thread::spawn(move || {
            let (mut stream, _) = first.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"list\n");
            started.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            stream.write_all(b"addserver 127.0.0.1 1111\n").unwrap();
        });
        let new = std::thread::spawn(move || {
            let (mut stream, _) = second.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"list\n");
            stream.write_all(b"addserver 127.0.0.1 2222\n").unwrap();
        });
        let mut state = State::configured(Config {
            permissions: permissions(Vec::new(), Vec::new(), false),
            master: Some(super::super::discovery::Master { address: a }),
        })
        .unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        let id = number(&state.dispatch(16, &[]).unwrap(), 0).unwrap();
        state.dispatch(10, &destination(id, b)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while state.list.is_none() {
            state.collect_master();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        new.join().unwrap();
        assert!(state.dispatch(10, &destination(id, a)).is_err());
        assert_eq!(state.master.as_ref().unwrap().address, b);
        assert!(state.fetching.is_none(), "failed reconnect started a fetch");
        release.send(()).unwrap();
        old.join().unwrap();
        state.collect_master();
        assert_eq!(
            state.list.as_deref().unwrap(),
            b"addserver 127.0.0.1 2222\n"
        );
        let mut send = pair(id, 0);
        send.extend([0; 4]);
        send.extend(b"list\n");
        state.dispatch(3, &send).unwrap();
        state.dispatch(13, &pair(id, 0)).unwrap();
        assert_eq!(state.dispatch(4, &pair(id, 4096)).unwrap(), [0; 8]);
        let next = number(&state.dispatch(16, &[]).unwrap(), 0).unwrap();
        state.dispatch(10, &destination(next, b)).unwrap();
        state.dispatch(13, &pair(next, 1)).unwrap();
        send[..4].copy_from_slice(&next.to_le_bytes());
        assert!(state.dispatch(3, &send).is_err());
    }
    #[test]
    fn socket_options_reach_os_survive_rebind_and_reject_invalid_values() {
        let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = reservation.local_addr().unwrap();
        drop(reservation);
        let mut state = State::new(permissions(Vec::new(), vec![addr.to_string()], true)).unwrap();
        let id = create(&mut state);
        let request = |option: u32, value: u32| {
            [id.to_le_bytes(), option.to_le_bytes(), value.to_le_bytes()].concat()
        };
        for (option, value) in [
            (1, 1),
            (2, 0),
            (3, 32768),
            (4, 32768),
            (5, 1),
            (6, 250),
            (7, 500),
            (10, 37),
        ] {
            state.dispatch(11, &request(option, value)).unwrap();
        }
        let check = |socket: &UdpSocket| {
            assert_eq!(socket.ttl().unwrap(), 37);
            assert!(!socket.broadcast().unwrap());
            assert_eq!(
                socket.read_timeout().unwrap(),
                Some(Duration::from_millis(250))
            );
            assert_eq!(
                socket.write_timeout().unwrap(),
                Some(Duration::from_millis(500))
            );
            use std::os::fd::AsRawFd;
            for (option, minimum) in [
                (libc::SO_RCVBUF, 32768),
                (libc::SO_SNDBUF, 32768),
                (libc::SO_REUSEADDR, 1),
            ] {
                let mut value: libc::c_int = 0;
                let mut length = std::mem::size_of_val(&value) as libc::socklen_t;
                let result = unsafe {
                    libc::getsockopt(
                        socket.as_raw_fd(),
                        libc::SOL_SOCKET,
                        option,
                        (&mut value as *mut libc::c_int).cast(),
                        &mut length,
                    )
                };
                assert_eq!(result, 0);
                assert!(value >= minimum, "option {option} was not applied: {value}");
            }
        };
        check(&state.sockets[&id].udp);
        state.dispatch(2, &destination(id, addr)).unwrap();
        check(&state.sockets[&id].udp);
        assert_eq!(
            state.dispatch(12, &request(10, 0)[..8]).unwrap(),
            37u32.to_le_bytes()
        );
        assert_eq!(
            state.dispatch(12, &request(8, 0)[..8]).unwrap(),
            0u32.to_le_bytes()
        );
        for (option, value) in [
            (1, 0),
            (2, 2),
            (3, 1048577),
            (4, u32::MAX),
            (5, 2),
            (6, 1001),
            (7, u32::MAX),
            (8, 0),
            (9, 1),
            (10, 0),
            (10, 256),
            (99, 0),
        ] {
            assert!(state.dispatch(11, &request(option, value)).is_err());
        }
        for size in [0, 4, 8, 11] {
            assert!(state.dispatch(11, &request(10, 42)[..size]).is_err());
        }
        assert!(state.dispatch(12, &request(3, 0)[..8]).is_err());
        assert_eq!(state.sockets[&id].udp.ttl().unwrap(), 37);
        state.dispatch(7, &id.to_le_bytes()).unwrap();
        assert!(state.dispatch(11, &request(10, 42)).is_err());
        assert!(state.dispatch(12, &request(10, 0)[..8]).is_err());
    }
    #[test]
    fn master_snapshot_is_async_bounded_without_restricting_udp_destinations() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        let peer_addr = peer.local_addr().unwrap();
        let unrelated = UdpSocket::bind("127.0.0.1:0").unwrap();
        let (release, wait) = mpsc::channel();
        let mock = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"list\n");
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
            stream
                .write_all(
                    format!(
                        "echo ignored\naddserver {} {}\naddserver evil.invalid 1\n",
                        peer_addr.ip(),
                        peer_addr.port()
                    )
                    .as_bytes(),
                )
                .unwrap();
        });
        let mut state = State::configured(Config {
            permissions: permissions(Vec::new(), Vec::new(), true),
            master: Some(
                super::super::discovery::Master::resolve(&addr.to_string(), true).unwrap(),
            ),
        })
        .unwrap();
        let udp = create(&mut state);
        let mut packet = destination(udp, peer_addr);
        packet.extend(b"test");
        state.dispatch(3, &packet).unwrap();
        let stream = number(&state.dispatch(16, &[]).unwrap(), 0).unwrap();
        state.dispatch(10, &destination(stream, addr)).unwrap();
        let mut request = stream.to_le_bytes().to_vec();
        request.extend([0; 8]);
        request.extend(b"list\n");
        let mut bad = request.clone();
        *bad.last_mut().unwrap() = b';';
        assert!(state.dispatch(3, &bad).is_err());
        // A slow master never holds an engine RPC: the first snapshot is empty.
        let before = Instant::now();
        state.dispatch(3, &request).unwrap();
        let mut receive = stream.to_le_bytes().to_vec();
        receive.extend(32u32.to_le_bytes());
        assert_eq!(state.dispatch(4, &receive).unwrap(), vec![0; 8]);
        assert!(before.elapsed() < Duration::from_millis(100));
        release.send(()).unwrap();
        mock.join().unwrap();
        let end = Instant::now() + Duration::from_secs(2);
        while state.list.is_none() {
            state.collect_master();
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(state.discovered.len(), 2);
        state.dispatch(3, &packet).unwrap();
        let mut denied = destination(udp, unrelated.local_addr().unwrap());
        denied.extend(b"no");
        state.dispatch(3, &denied).unwrap();
        assert!(state.dispatch(6, b"bad/host").is_err());
        state.dispatch(7, &stream.to_le_bytes()).unwrap();
        let stream = number(&state.dispatch(16, &[]).unwrap(), 0).unwrap();
        state.dispatch(10, &destination(stream, addr)).unwrap();
        request[..4].copy_from_slice(&stream.to_le_bytes());
        state.dispatch(3, &request).unwrap();
        receive[..4].copy_from_slice(&stream.to_le_bytes());
        state.bytes = 1048576;
        assert_eq!(state.call(4, &receive).0, 2);
        state.bytes = 0;
        let data = state.dispatch(4, &receive).unwrap();
        assert_eq!(
            &data[8..],
            format!("addserver {} {}\n", peer_addr.ip(), peer_addr.port()).as_bytes()
        );
        assert_eq!(state.dispatch(4, &receive).unwrap(), vec![0; 8]);
        assert!(
            state.fetching.is_none(),
            "cached requests must not refetch within 30 seconds"
        );
        state.refreshed = Some(Instant::now() - Duration::from_secs(601));
        state.dispatch(3, &packet).unwrap();
        assert!(state.list.is_none(), "expired discovery snapshot survived");
    }
    #[test]
    fn enabled_udp_reaches_unlisted_peers_and_accepts_unlisted_sources_but_not_stale_handles() {
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let other = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = peer.local_addr().unwrap();
        let mut state = State::new(permissions(vec![addr.to_string()], Vec::new(), true)).unwrap();
        let id = create(&mut state);
        let mut send = destination(id, addr);
        send.extend(b"hello");
        state.dispatch(3, &send).unwrap();
        let mut bytes = [0; 32];
        let (size, from) = peer.recv_from(&mut bytes).unwrap();
        assert_eq!(&bytes[..size], b"hello");
        let mut denied = destination(id, other.local_addr().unwrap());
        denied.extend(b"no");
        state.dispatch(3, &denied).unwrap();
        other.send_to(b"other", from).unwrap();
        peer.send_to(b"reply", from).unwrap();
        let mut receive = id.to_le_bytes().to_vec();
        receive.extend(32u32.to_le_bytes());
        let end = Instant::now() + Duration::from_secs(1);
        let result = loop {
            if let Ok(result) = state.dispatch(4, &receive) {
                break result;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(&result[8..], b"other");
        assert!(state.dispatch(6, b"bad/host").is_err());
        assert_eq!(state.dispatch(6, b"127.0.0.1").unwrap(), [127, 0, 0, 1]);
        assert_eq!(state.dispatch(6, b"localhost").unwrap(), [127, 0, 0, 1]);
        state.dispatch(7, &id.to_le_bytes()).unwrap();
        assert!(state.dispatch(3, &send).is_err());
        assert!(State::new(permissions(Vec::new(), Vec::new(), false)).is_ok());
    }
    #[test]
    fn unlisted_listeners_and_peers_work_and_requests_have_strict_bounds() {
        let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = reservation.local_addr().unwrap();
        drop(reservation);
        let mut state = State::new(permissions(Vec::new(), vec![addr.to_string()], true)).unwrap();
        let id = create(&mut state);
        let bind = destination(id, addr);
        state.dispatch(2, &bind).unwrap();
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let mut reply = destination(id, peer.local_addr().unwrap());
        reply.extend(b"reply");
        state.dispatch(3, &reply).unwrap();
        peer.send_to(b"hello", addr).unwrap();
        let mut recv = id.to_le_bytes().to_vec();
        recv.extend(32u32.to_le_bytes());
        let end = Instant::now() + Duration::from_secs(1);
        while state.dispatch(4, &recv).is_err() {
            assert!(Instant::now() < end);
        }
        state.dispatch(3, &reply).unwrap();
        let mut b = [0; 32];
        let (n, _) = peer.recv_from(&mut b).unwrap();
        assert_eq!(&b[..n], b"reply");
        for op in [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 99] {
            assert!(state.dispatch(op, &[]).is_err());
        }
        let stream = number(&state.dispatch(1, &1u32.to_le_bytes()).unwrap(), 0).unwrap();
        state.dispatch(7, &stream.to_le_bytes()).unwrap();
        let mut huge = destination(id, peer.local_addr().unwrap());
        huge.extend(vec![0; MAX_DATAGRAM + 1]);
        assert!(state.dispatch(3, &huge).is_err());
        state.bytes = 1048576;
        assert_eq!(state.call(3, &reply).0, 2);
    }
    #[test]
    fn handles_open_rates_addresses_and_packet_fields_are_bounded() {
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        let mut state = State::new(permissions(
            vec![peer.local_addr().unwrap().to_string()],
            Vec::new(),
            true,
        ))
        .unwrap();
        let id = create(&mut state);
        for _ in 1..8 {
            create(&mut state);
        }
        assert!(state.dispatch(1, &2u32.to_le_bytes()).is_err());
        state.window = Instant::now() - Duration::from_secs(2);
        for _ in 0..8 {
            create(&mut state);
        }
        state.window = Instant::now() - Duration::from_secs(2);
        assert!(state.dispatch(1, &2u32.to_le_bytes()).is_err());
        assert_eq!(state.sockets.len(), 16);
        let mut recv = id.to_le_bytes().to_vec();
        recv.extend((MAX_DATAGRAM as u32 + 1).to_le_bytes());
        assert!(state.dispatch(4, &recv).is_err());
        let mut poll = id.to_le_bytes().to_vec();
        poll.extend(u32::MAX.to_le_bytes());
        assert!(state.dispatch(8, &poll).is_err());
        let mut send = destination(id, peer.local_addr().unwrap());
        send[8..12].copy_from_slice(&65536u32.to_le_bytes());
        assert!(state.dispatch(3, &send).is_err());
        assert_eq!(
            encoded_address("127.2.3.4:4660".parse().unwrap()).unwrap(),
            [127, 2, 3, 4, 0x34, 0x12, 0, 0]
        );
        state.calls = 2048;
        assert_eq!(state.call(5, &id.to_le_bytes()).0, 2);
        let mut p = permissions(Vec::new(), Vec::new(), false);
        p.network = false;
        let mut offline = State::new(p).unwrap();
        assert_eq!(offline.call(1, &2u32.to_le_bytes()).0, 1);
        assert!(offline.sockets.is_empty());
    }
    #[test]
    fn malformed_ipc_frames_close_without_reading_unbounded_payloads() {
        let mut p = permissions(Vec::new(), Vec::new(), false);
        p.network = false;
        let (_worker, mut channel) = start(p.into()).unwrap();
        let mut header = b"GPN1".to_vec();
        header.extend(1u32.to_le_bytes());
        header.extend(u32::MAX.to_le_bytes());
        header.extend([0; 4]);
        channel.write_all(&header).unwrap();
        let mut reply = [0; 1];
        assert_eq!(channel.read(&mut reply).unwrap(), 0);
    }
}
