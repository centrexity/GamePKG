//! Per-game, signed on/off networking permission. Only the host owns sockets and HTTPS clients.
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const MAX_HANDLES: usize = 16;
const MAX_BYTES: usize = 65536;
const MAX_TRANSFER_BYTES: usize = 256 * 1024;
const MAX_JOBS: usize = 16;
static JOBS: AtomicUsize = AtomicUsize::new(0);
fn connect_tcp(addresses: &[SocketAddr]) -> Result<TcpStream> {
    ensure!(
        !addresses.is_empty() && addresses.len() <= 64,
        "invalid DNS result"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_error = None;
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        // A refusal or unavailable address family leaves the remaining original
        // deadline for later candidates; no address receives a fresh deadline.
        match TcpStream::connect_timeout(address, remaining) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .unwrap_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))
        .into())
}
fn connect_udp(addresses: &[SocketAddr]) -> Result<UdpSocket> {
    ensure!(
        !addresses.is_empty() && addresses.len() <= 64,
        "invalid DNS result"
    );
    let mut last_error = None;
    for address in addresses {
        let attempt = || -> std::io::Result<UdpSocket> {
            let socket = UdpSocket::bind(if address.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            })?;
            socket.set_broadcast(true)?;
            socket.connect(address)?;
            socket.set_nonblocking(true)?;
            Ok(socket)
        };
        match attempt() {
            Ok(socket) => return Ok(socket),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.context("invalid DNS result")?.into())
}
#[derive(Debug, Clone)]
pub(crate) struct Endpoint {
    host: String,
    port: u16,
}
impl Endpoint {
    fn parse(text: &str) -> Result<Self> {
        ensure!(
            text.len() <= 256 && !text.chars().any(char::is_control),
            "invalid network endpoint"
        );
        let u = url::Url::parse(&format!("tcp://{text}"))?;
        ensure!(
            u.username().is_empty()
                && u.password().is_none()
                && u.path().is_empty()
                && u.query().is_none()
                && u.fragment().is_none(),
            "network endpoints must be exact host:port pairs"
        );
        let host = u
            .host_str()
            .context("network host missing")?
            .to_ascii_lowercase();
        ensure!(
            host.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-.:[]".contains(&c)),
            "network hosts cannot contain wildcards or encoded characters"
        );
        let port = u.port().context("explicit network port required")?;
        ensure!(port > 0, "network port must be nonzero");
        Ok(Self { host, port })
    }
    fn https_command(&self, url: &str, addresses: &[SocketAddr]) -> Result<std::process::Command> {
        ensure!(
            !addresses.is_empty() && addresses.len() <= 64,
            "invalid DNS result"
        );
        let mut command = std::process::Command::new("curl");
        command.env_clear().args([
            "-q",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--proxy",
            "",
            "--max-redirs",
            "0",
            "--connect-timeout",
            "2",
            "--max-time",
            "5",
            "--max-filesize",
            "65536",
            "--limit-rate",
            "65536",
        ]);
        // Numeric URL hosts already select their exact address. Older curl
        // versions reject IPv6 literals in the host field of --resolve.
        if self
            .host
            .trim_matches(['[', ']'])
            .parse::<IpAddr>()
            .is_err()
        {
            let ips = addresses
                .iter()
                .map(|address| {
                    if address.is_ipv6() {
                        format!("[{}]", address.ip())
                    } else {
                        address.ip().to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            command
                .arg("--resolve")
                .arg(format!("{}:{}:{ips}", self.host, self.port));
        }
        command.arg("--url").arg(url);
        Ok(command)
    }
    fn key(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    fn resolve(&self, local: bool) -> Result<SocketAddr> {
        Ok(self.resolve_all(local)?[0])
    }
    fn resolve_all(&self, _local: bool) -> Result<Vec<SocketAddr>> {
        let hostname = self.host.trim_start_matches('[').trim_end_matches(']');
        let addresses = (hostname, self.port).to_socket_addrs()?.collect::<Vec<_>>();
        ensure!(
            !addresses.is_empty() && addresses.len() <= 64,
            "invalid DNS result"
        );
        Ok(addresses)
    }
}
/// Resolve saved launch addresses for a reviewed engine; not an allowlist.
#[cfg(target_os = "linux")]
pub(crate) fn multiplayer_endpoints(
    p: &crate::manifest::Permissions,
    listen: bool,
) -> Result<Vec<(String, SocketAddr)>> {
    validate(p)?;
    ensure!(p.network, "multiplayer permission denied");
    let entries = if listen {
        &p.network_listen
    } else {
        &p.network_hosts
    };
    let mut pinned = BTreeMap::new();
    entries
        .iter()
        .map(|text| {
            let endpoint = Endpoint::parse(text)?;
            let ip = if let Some(ip) = pinned.get(&endpoint.host) {
                *ip
            } else {
                let ip = endpoint
                    .resolve_all(true)?
                    .into_iter()
                    .find(SocketAddr::is_ipv4)
                    .context("Cube 2 ENet requires an IPv4 endpoint")?
                    .ip();
                pinned.insert(endpoint.host.clone(), ip);
                ip
            };
            let address = SocketAddr::new(ip, endpoint.port);
            Ok((endpoint.host.clone(), address))
        })
        .collect()
}
pub(crate) fn validate(_p: &crate::manifest::Permissions) -> Result<()> {
    // Legacy endpoint fields are retained for reading signed packages and host
    // launch preferences, but are no longer capability restrictions.
    Ok(())
}
pub(crate) fn validate_user_selected(p: &crate::manifest::Permissions) -> Result<()> {
    validate(p)
}
pub(crate) fn selected_multiplayer(
    server: &str,
    listen: &str,
    local: bool,
) -> Result<crate::manifest::Permissions> {
    ensure!(
        server.is_empty() || listen.is_empty(),
        "choose joining a server or hosting, not both"
    );
    let endpoints = |text: &str, hosting: bool| -> Result<Vec<String>> {
        if text.is_empty() {
            return Ok(Vec::new());
        }
        let endpoint = Endpoint::parse(text)?;
        ensure!(
            !endpoint.host.contains(':') && endpoint.port < 65535,
            "use an IPv4 address or hostname and port 1–65534"
        );
        if hosting {
            ensure!(
                endpoint.host.parse::<std::net::Ipv4Addr>().is_ok(),
                "hosting requires a local numeric IPv4 address"
            );
        }
        Ok(vec![
            endpoint.key(),
            format!("{}:{}", endpoint.host, endpoint.port + 1),
        ])
    };
    let hosts = endpoints(server, false)?;
    let listeners = endpoints(listen, true)?;
    let enabled = !hosts.is_empty() || !listeners.is_empty();
    let permissions = crate::manifest::Permissions {
        files: Vec::new(),
        input: false,
        audio: false,
        save: false,
        network: enabled,
        network_hosts: hosts,
        network_listen: listeners,
        network_allow_local: enabled && local,
    };
    validate(&permissions)?;
    Ok(permissions)
}
enum Handle {
    Pending(mpsc::Receiver<Result<Resource>>),
    Ready(Resource),
    Failed,
}
enum Resource {
    Https(Vec<u8>),
    Tcp(TcpStream),
    Udp(UdpSocket),
    Listener(TcpListener),
}
pub(crate) struct Service {
    permissions: crate::manifest::Permissions,
    handles: BTreeMap<u32, Handle>,
    next: u32,
    urls: BTreeMap<String, u32>,
    window: Instant,
    used: usize,
    opens: usize,
}
struct Job<'a>(&'a AtomicUsize);
impl<'a> Job<'a> {
    // Keep compatibility with Rust 1.89, before fetch_update was renamed.
    #[allow(deprecated)]
    fn acquire(counter: &'a AtomicUsize) -> Result<Self> {
        counter
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                if used < MAX_JOBS {
                    Some(used + 1)
                } else {
                    None
                }
            })
            .map_err(|_| anyhow::anyhow!("network workers busy"))?;
        Ok(Self(counter))
    }
}
impl Drop for Job<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
#[cfg(target_os = "linux")]
pub(crate) fn resolve_engine_host(name: &str) -> Result<SocketAddr> {
    let endpoint = Endpoint::parse(&format!("{name}:1"))?;
    let guard = Job::acquire(&JOBS)?;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("gamepkg-dns".into())
        .spawn(move || {
            let _guard = guard;
            let result = endpoint.resolve_all(true).and_then(|addresses| {
                addresses
                    .into_iter()
                    .find(SocketAddr::is_ipv4)
                    .context("IPv4 address missing")
            });
            let _ = tx.send(result);
        })?;
    // Never leave a DNS response queued on the engine RPC channel after timeout.
    rx.recv_timeout(Duration::from_millis(500))
        .context("DNS lookup timed out")?
}
impl Service {
    pub fn new(permissions: crate::manifest::Permissions) -> Self {
        Self {
            permissions,
            handles: BTreeMap::new(),
            next: 1,
            urls: BTreeMap::new(),
            window: Instant::now(),
            used: 0,
            opens: 0,
        }
    }
    fn capable(&self, _https: bool) -> Result<()> {
        ensure!(self.permissions.network, "network permission denied");
        Ok(())
    }
    fn endpoint(&self, text: &str, listen: bool) -> Result<Endpoint> {
        self.capable(false)?;
        let endpoint = Endpoint::parse(text)?;
        if listen {
            endpoint
                .host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .context("listening requires a numeric bind address")?;
        }
        Ok(endpoint)
    }
    fn reset_budget(&mut self) {
        if self.window.elapsed() >= Duration::from_secs(1) {
            self.window = Instant::now();
            self.used = 0;
            self.opens = 0;
        }
    }
    fn check_reservation(&mut self) -> Result<()> {
        self.reset_budget();
        ensure!(self.opens < 8, "network connections busy");
        ensure!(
            self.handles.len() < MAX_HANDLES && self.next < u32::MAX,
            "network handle limit reached"
        );
        Ok(())
    }
    fn reserve(&mut self) -> Result<u32> {
        self.check_reservation()?;
        self.opens += 1;
        let id = self.next;
        self.next += 1;
        Ok(id)
    }
    fn spawn(&mut self, job: impl FnOnce() -> Result<Resource> + Send + 'static) -> Result<u32> {
        self.spawn_with_counter(&JOBS, job)
    }
    fn spawn_with_counter(
        &mut self,
        counter: &'static AtomicUsize,
        job: impl FnOnce() -> Result<Resource> + Send + 'static,
    ) -> Result<u32> {
        self.check_reservation()?;
        let guard = Job::acquire(counter)?;
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("gamepkg-network".into())
            .spawn(move || {
                let _guard = guard;
                let _ = tx.send(job());
            })?;
        let id = self.reserve()?;
        self.handles.insert(id, Handle::Pending(rx));
        Ok(id)
    }
    fn refresh(&mut self, id: u32) -> Result<&mut Resource> {
        let h = self
            .handles
            .get_mut(&id)
            .context("unknown network handle")?;
        if let Handle::Pending(rx) = h {
            match rx.try_recv() {
                Ok(Ok(value)) => *h = Handle::Ready(value),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => *h = Handle::Failed,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match h {
            Handle::Ready(value) => Ok(value),
            Handle::Pending(_) => bail!("pending"),
            Handle::Failed => bail!("network operation failed"),
        }
    }
    fn budget(&mut self, n: usize) -> Result<()> {
        self.reset_budget();
        ensure!(
            n <= MAX_BYTES && self.used + n <= MAX_TRANSFER_BYTES,
            "network transfer budget exceeded"
        );
        self.used += n;
        Ok(())
    }
    fn https(&mut self, url: &str) -> Result<u32> {
        self.capable(true)?;
        ensure!(
            url.len() <= 2048 && !url.chars().any(char::is_control),
            "URL too long or contains control characters"
        );
        let u = url::Url::parse(url)?;
        ensure!(
            u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
                && u.fragment().is_none(),
            "only HTTPS URLs without credentials or fragments are allowed"
        );
        let host = u.host_str().context("URL host missing")?.to_owned();
        let port = u.port_or_known_default().context("HTTPS port missing")?;
        let endpoint = self.endpoint(&format!("{host}:{port}"), false)?;
        let local = true;
        let url = u.to_string();
        self.spawn(move || {
            let addresses = endpoint.resolve_all(local)?;
            let mut command = endpoint.https_command(&url, &addresses)?;
            let output = crate::compiler::run_timed(&mut command, Duration::from_secs(6))?;
            ensure!(output.status.success(), "HTTPS request failed");
            ensure!(
                output.stdout_bytes.len() <= MAX_BYTES,
                "HTTPS response too large"
            );
            Ok(Resource::Https(output.stdout_bytes))
        })
    }
    fn connect(&mut self, text: &str, udp: bool) -> Result<u32> {
        self.capable(false)?;
        let endpoint = self.endpoint(text, false)?;
        let local = true;
        self.spawn(move || {
            let addresses = endpoint.resolve_all(local)?;
            if udp {
                Ok(Resource::Udp(connect_udp(&addresses)?))
            } else {
                let stream = connect_tcp(&addresses)?;
                stream.set_nonblocking(true)?;
                stream.set_nodelay(true)?;
                Ok(Resource::Tcp(stream))
            }
        })
    }
    fn operation(&mut self, op: u8, b: &[u8], capacity: usize) -> Result<Vec<u8>> {
        fn id(b: &[u8]) -> Result<u32> {
            Ok(u32::from_le_bytes(
                b.get(..4).context("missing handle")?.try_into()?,
            ))
        }
        fn number(v: u32) -> Vec<u8> {
            let mut b = vec![0];
            b.extend(v.to_le_bytes());
            b
        }
        ensure!(
            capacity <= MAX_BYTES && b.len() <= MAX_BYTES + 4,
            "network message too large"
        );
        match op {
            0 => Ok(number(self.https(std::str::from_utf8(b)?)?)),
            1 => match self.refresh(id(b)?)? {
                Resource::Https(bytes) => {
                    ensure!(bytes.len() <= capacity, "output buffer too small");
                    let mut reply = vec![0];
                    reply.extend(bytes.as_slice());
                    Ok(reply)
                }
                _ => Ok(vec![0]),
            },
            2 | 3 => Ok(number(self.connect(std::str::from_utf8(b)?, op == 3)?)),
            4 => {
                let id = id(b)?;
                let bytes = &b[4..];
                self.reset_budget();
                let available = MAX_TRANSFER_BYTES.saturating_sub(self.used);
                let n = match self.refresh(id)? {
                    Resource::Tcp(s) => {
                        ensure!(bytes.is_empty() || available > 0, "pending");
                        s.write(&bytes[..bytes.len().min(available)])?
                    }
                    Resource::Udp(s) => {
                        // Datagram sends must remain atomic across rate windows.
                        ensure!(bytes.len() <= available, "pending");
                        s.send(bytes)?
                    }
                    _ => bail!("handle is not a socket"),
                };
                self.budget(n)?;
                Ok(number(n as u32))
            }
            5 => {
                ensure!(capacity <= MAX_BYTES, "network transfer too large");
                let mut bytes = vec![0; capacity];
                self.reset_budget();
                let available = MAX_TRANSFER_BYTES.saturating_sub(self.used);
                let n = match self.refresh(id(b)?)? {
                    Resource::Tcp(s) => {
                        if capacity > 0 && available == 0 {
                            // EOF uses no transfer budget. Peek preserves any
                            // unread bytes until the next rate window.
                            ensure!(s.peek(&mut [0])? == 0, "pending");
                            0
                        } else {
                            s.read(&mut bytes[..capacity.min(available)])?
                        }
                    }
                    Resource::Udp(s) => {
                        if capacity > available {
                            // A small datagram may still fit. Never consume a
                            // larger one before its budget becomes available.
                            ensure!(s.peek(&mut bytes)? <= available, "pending");
                        }
                        s.recv(&mut bytes)?
                    }
                    _ => bail!("handle is not a socket"),
                };
                self.budget(n)?;
                bytes.truncate(n);
                let mut reply = vec![0];
                reply.extend(bytes);
                Ok(reply)
            }
            6 => {
                let id = id(b)?;
                self.handles.remove(&id).context("unknown network handle")?;
                self.urls.retain(|_, handle| *handle != id);
                Ok(vec![0])
            }
            7 => {
                self.capable(false)?;
                let endpoint = self.endpoint(std::str::from_utf8(b)?, true)?;
                self.check_reservation()?;
                let address = endpoint.resolve(true)?;
                let socket = TcpListener::bind(address)?;
                socket.set_nonblocking(true)?;
                let id = self.reserve()?;
                self.handles
                    .insert(id, Handle::Ready(Resource::Listener(socket)));
                Ok(number(id))
            }
            8 => {
                let listener = id(b)?;
                ensure!(
                    matches!(self.refresh(listener)?, Resource::Listener(_)),
                    "handle is not a listener"
                );
                // Check capacity before consuming a queued connection. A
                // rejected accept must leave it available for a later retry.
                self.check_reservation()?;
                let stream = match self.refresh(listener)? {
                    Resource::Listener(s) => s.accept()?.0,
                    _ => bail!("handle is not a listener"),
                };
                stream.set_nonblocking(true)?;
                stream.set_nodelay(true)?;
                let id = self.reserve()?;
                self.handles
                    .insert(id, Handle::Ready(Resource::Tcp(stream)));
                Ok(number(id))
            }
            9 => {
                let url = std::str::from_utf8(b)?.to_owned();
                let id = if let Some(id) = self.urls.get(&url) {
                    *id
                } else {
                    let id = self.https(&url)?;
                    self.urls.insert(url, id);
                    id
                };
                let result = self.operation(1, &id.to_le_bytes(), capacity);
                // The convenience API exposes no handle to close after failure.
                // Report this failure once, then let a later call start afresh.
                // Pending work and successful responses (including a response
                // that did not fit the caller's buffer) remain cached.
                if matches!(self.handles.get(&id), Some(Handle::Failed)) {
                    self.handles.remove(&id);
                    self.urls.retain(|_, cached| *cached != id);
                }
                result
            }
            _ => bail!("unknown network operation"),
        }
    }
    fn error_reply(error: anyhow::Error) -> Vec<u8> {
        if matches!(
            error.to_string().as_str(),
            "pending" | "network connections busy" | "network workers busy"
        ) || error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.kind() == std::io::ErrorKind::WouldBlock)
        {
            vec![2]
        } else if error.to_string() == "network operation failed"
            || error.downcast_ref::<std::io::Error>().is_some()
        {
            vec![3]
        } else {
            vec![1]
        }
    }
    pub fn call(&mut self, op: u8, b: &[u8], capacity: usize) -> Vec<u8> {
        match self.operation(op, b, capacity) {
            Ok(value) => value,
            Err(error) => Self::error_reply(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn permissions() -> crate::manifest::Permissions {
        let m: crate::manifest::Manifest =
            toml::from_str(include_str!("../examples/minimal/manifest.toml")).unwrap();
        m.permissions
    }
    fn handle(reply: Vec<u8>) -> u32 {
        assert_eq!(reply.first(), Some(&0), "{reply:?}");
        assert_eq!(reply.len(), 5);
        u32::from_le_bytes(reply[1..].try_into().unwrap())
    }
    fn until(mut f: impl FnMut() -> Vec<u8>) -> Vec<u8> {
        let end = Instant::now() + Duration::from_secs(3);
        loop {
            let reply = f();
            if reply != [2] {
                return reply;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn enabled_network_needs_no_endpoints_and_disabled_network_denies_every_open() {
        let mut p = permissions();
        assert!(validate(&p).is_ok());
        p.network = true;
        // Legacy restrictions are ignored; no DNS or public traffic in this test.
        p.network_hosts = vec!["legacy.invalid:443".into()];
        p.network_listen = vec!["legacy.invalid:1".into()];
        p.network_allow_local = false;
        assert!(validate(&p).is_ok());
        let mut s = Service::new(p);
        for endpoint in [
            "127.0.0.1:1234",
            "10.1.2.3:42",
            "example.org:8443",
            "[::1]:1234",
        ] {
            assert!(s.endpoint(endpoint, false).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "*.example.com:443",
            "user@example.com:443",
            "x:1/path",
            "x:0",
            "x:1?x",
            "x:1#x",
        ] {
            assert!(s.endpoint(endpoint, false).is_err(), "{endpoint}");
        }
        for url in [
            "http://example.com/",
            "https://user:password@example.com/",
            "https://example.com/#secret",
        ] {
            assert_eq!(s.call(0, url.as_bytes(), 4), [1], "{url}");
        }
        let mut denied = Service::new(permissions());
        for (op, request) in [
            (0, "https://example.com/"),
            (2, "127.0.0.1:1234"),
            (3, "127.0.0.1:1234"),
            (7, "127.0.0.1:1234"),
        ] {
            assert_eq!(denied.call(op, request.as_bytes(), 4), [1]);
        }
        assert!(denied.handles.is_empty());
    }
    #[test]
    fn enabled_tcp_and_udp_reach_unlisted_local_peers_and_handles_are_scoped() {
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 4];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"ping");
            stream.write_all(b"pong").unwrap();
        });
        let mut p = permissions();
        p.network = true;
        assert!(p.network_hosts.is_empty());
        assert!(!p.network_allow_local);
        validate(&p).unwrap();
        let mut service = Service::new(p.clone());
        let id = handle(service.call(2, address.to_string().as_bytes(), 4));
        assert_eq!(until(|| service.call(1, &id.to_le_bytes(), 0)), [0]);
        let mut request = id.to_le_bytes().to_vec();
        request.extend(b"ping");
        assert_eq!(handle(until(|| service.call(4, &request, 4))), 4);
        assert_eq!(until(|| service.call(5, &id.to_le_bytes(), 16)), b"\0pong");
        worker.join().unwrap();
        let mut other = Service::new(p);
        assert_eq!(other.call(5, &id.to_le_bytes(), 16), [1]);
        assert!(service.endpoint("127.0.0.1:1", false).is_ok());
        assert_eq!(service.call(6, &id.to_le_bytes(), 0), [0]);
        assert_eq!(service.call(5, &id.to_le_bytes(), 16), [1]);
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut b = [0; 4];
            let (n, peer) = socket.recv_from(&mut b).unwrap();
            assert_eq!(&b[..n], b"ping");
            socket.send_to(b"pong", peer).unwrap();
        });
        let mut p = permissions();
        p.network = true;
        assert!(p.network_hosts.is_empty());
        assert!(!p.network_allow_local);
        let mut service = Service::new(p);
        let id = handle(service.call(3, address.to_string().as_bytes(), 4));
        assert_eq!(until(|| service.call(1, &id.to_le_bytes(), 0)), [0]);
        let mut request = id.to_le_bytes().to_vec();
        request.extend(b"ping");
        assert_eq!(handle(until(|| service.call(4, &request, 4))), 4);
        assert_eq!(until(|| service.call(5, &id.to_le_bytes(), 16)), b"\0pong");
        worker.join().unwrap();
    }
    fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client.set_nonblocking(true).unwrap();
        (client, server)
    }
    fn ready_service(resource: Resource) -> Service {
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        service.handles.insert(1, Handle::Ready(resource));
        service.next = 2;
        service
    }
    #[test]
    fn cached_request_releases_completed_failures_but_retains_pending_work() {
        for disconnected in [false, true] {
            let mut permissions = permissions();
            permissions.network = true;
            let mut service = Service::new(permissions);
            let url = "https://unused.invalid/";
            let (sender, receiver) = mpsc::sync_channel(1);
            service.handles.insert(1, Handle::Pending(receiver));
            service.urls.insert(url.to_string(), 1);
            service.next = 2;
            for _ in 0..3 {
                assert_eq!(service.call(9, url.as_bytes(), 16), [2]);
                assert_eq!(service.handles.len(), 1);
                assert_eq!(service.urls.get(url), Some(&1));
                assert_eq!(service.next, 2);
                assert_eq!(service.opens, 0);
            }
            if disconnected {
                drop(sender);
            } else {
                sender
                    .send(Err(anyhow::anyhow!("fixture HTTPS failure")))
                    .unwrap();
            }
            assert_eq!(service.call(9, url.as_bytes(), 16), [3]);
            assert!(service.urls.is_empty());
            assert!(service.handles.is_empty());
            assert_eq!(service.next, 2);
            assert_eq!(service.opens, 0);
        }
    }
    #[test]
    fn cached_success_and_small_output_buffers_do_not_restart_requests() {
        let mut service = ready_service(Resource::Https(b"complete response".to_vec()));
        let url = "https://unused.invalid/";
        service.urls.insert(url.to_string(), 1);
        assert_eq!(service.call(9, url.as_bytes(), 2), [1]);
        for _ in 0..3 {
            assert_eq!(service.call(9, url.as_bytes(), 32), b"\0complete response");
            assert_eq!(service.urls.get(url), Some(&1));
            assert_eq!(service.handles.len(), 1);
            assert_eq!(service.next, 2);
            assert_eq!(service.opens, 0);
        }
        assert_eq!(service.call(6, &1u32.to_le_bytes(), 0), [0]);
        assert!(service.urls.is_empty());
        assert!(service.handles.is_empty());
    }
    #[test]
    #[ignore = "performs an actual public HTTPS GET"]
    fn cached_https_retries_after_a_completed_failure() {
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        let url = "https://example.com/";
        service.handles.insert(1, Handle::Failed);
        service.urls.insert(url.to_string(), 1);
        service.next = 2;
        assert_eq!(service.call(9, url.as_bytes(), MAX_BYTES), [3]);
        let deadline = Instant::now() + Duration::from_secs(8);
        let response = loop {
            let reply = service.call(9, url.as_bytes(), MAX_BYTES);
            if reply == [2] {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(20));
            } else {
                break reply;
            }
        };
        assert_eq!(response.first(), Some(&0));
        assert!(std::str::from_utf8(&response[1..])
            .unwrap()
            .contains("Example Domain"));
        assert_eq!(service.urls.get(url), Some(&2));
        assert_eq!(service.handles.len(), 1);
        assert_eq!(service.next, 3);
        assert_eq!(service.opens, 1);
        assert_eq!(service.call(9, url.as_bytes(), MAX_BYTES), response);
    }
    #[test]
    fn job_permits_never_exceed_capacity_and_release_on_drop() {
        let counter = AtomicUsize::new(0);
        let mut guards = Vec::new();
        for _ in 0..MAX_JOBS {
            guards.push(Job::acquire(&counter).unwrap());
        }
        assert_eq!(counter.load(Ordering::SeqCst), MAX_JOBS);
        assert!(Job::acquire(&counter).is_err());
        assert_eq!(counter.load(Ordering::SeqCst), MAX_JOBS);
        guards.pop();
        assert_eq!(counter.load(Ordering::SeqCst), MAX_JOBS - 1);
        guards.push(Job::acquire(&counter).unwrap());
        assert_eq!(counter.load(Ordering::SeqCst), MAX_JOBS);
        drop(guards);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn rejected_game_admission_never_takes_worker_capacity() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut service = Service::new(permissions());
        for rate_limited in [true, false] {
            service.opens = if rate_limited { 8 } else { 0 };
            if !rate_limited {
                for id in 1..=MAX_HANDLES as u32 {
                    service.handles.insert(id, Handle::Failed);
                }
            }
            assert!(service
                .spawn_with_counter(&COUNTER, || panic!("rejected job started"))
                .is_err());
            assert_eq!(COUNTER.load(Ordering::SeqCst), 0);
            assert_eq!(service.next, 1);
            assert_eq!(service.opens, if rate_limited { 8 } else { 0 });
        }
    }
    #[test]
    fn cancelled_jobs_close_late_sockets_and_cannot_replace_new_handles() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        for stopped in [false, true] {
            let (client, mut peer) = tcp_pair();
            let mut permissions = permissions();
            permissions.network = true;
            let mut service = Some(Service::new(permissions));
            let (release, wait) = mpsc::sync_channel(1);
            let id = service
                .as_mut()
                .unwrap()
                .spawn_with_counter(&COUNTER, move || {
                    wait.recv_timeout(Duration::from_secs(2))?;
                    Ok(Resource::Tcp(client))
                })
                .unwrap();
            assert_eq!(COUNTER.load(Ordering::SeqCst), 1);
            let mut replacement = 0;
            if stopped {
                drop(service.take());
            } else {
                let current = service.as_mut().unwrap();
                assert_eq!(current.call(6, &id.to_le_bytes(), 0), [0]);
                assert_eq!(current.call(1, &id.to_le_bytes(), 8), [1]);
                replacement = current
                    .spawn_with_counter(&COUNTER, || Ok(Resource::Https(b"new".to_vec())))
                    .unwrap();
                assert!(replacement > id);
                assert_eq!(
                    until(|| current.call(1, &replacement.to_le_bytes(), 8)),
                    b"\0new"
                );
            }
            assert!(
                COUNTER.load(Ordering::SeqCst) >= 1,
                "closing a handle released capacity while its job was still running"
            );
            release.send(()).unwrap();
            assert_eq!(
                peer.read(&mut [0u8; 1]).unwrap(),
                0,
                "abandoned result retained its socket"
            );
            let deadline = Instant::now() + Duration::from_secs(2);
            while COUNTER.load(Ordering::SeqCst) != 0 {
                assert!(Instant::now() < deadline, "worker permit leaked");
                std::thread::yield_now();
            }
            if let Some(current) = service.as_mut() {
                assert_eq!(current.handles.len(), 1);
                assert_eq!(current.call(1, &replacement.to_le_bytes(), 8), b"\0new");
                assert_eq!(current.call(1, &id.to_le_bytes(), 8), [1]);
                assert_eq!(current.call(6, &replacement.to_le_bytes(), 0), [0]);
            }
        }
    }
    #[test]
    fn busy_worker_admission_is_retryable_without_spending_game_capacity() {
        static COUNTER: AtomicUsize = AtomicUsize::new(16);
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        let error = service
            .spawn_with_counter(&COUNTER, || panic!("busy job started"))
            .unwrap_err();
        assert_eq!(COUNTER.load(Ordering::SeqCst), 16);
        assert_eq!(Service::error_reply(error), [2]);
        assert_eq!(service.opens, 0);
        assert_eq!(service.next, 1);
        assert!(service.handles.is_empty());
    }
    #[test]
    fn completed_job_failure_releases_worker_capacity_for_a_fresh_request() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        let failed = service
            .spawn_with_counter(&COUNTER, || bail!("fixture connection failure"))
            .unwrap();
        assert_eq!(until(|| service.call(1, &failed.to_le_bytes(), 8)), [3]);
        assert_eq!(service.call(6, &failed.to_le_bytes(), 0), [0]);
        let fresh = service
            .spawn_with_counter(&COUNTER, || Ok(Resource::Https(b"fresh".to_vec())))
            .unwrap();
        assert!(fresh > failed);
        assert_eq!(
            until(|| service.call(1, &fresh.to_le_bytes(), 8)),
            b"\0fresh"
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        while COUNTER.load(Ordering::SeqCst) != 0 {
            assert!(Instant::now() < deadline, "completed job retained a permit");
            std::thread::yield_now();
        }
        assert_eq!(service.call(1, &failed.to_le_bytes(), 8), [1]);
        assert_eq!(service.call(6, &fresh.to_le_bytes(), 0), [0]);
        assert!(service.handles.is_empty());
    }
    #[test]
    fn pending_failed_and_unknown_sends_do_not_consume_transfer_budget() {
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        let (_sender, receiver) = mpsc::sync_channel(1);
        service.handles.insert(1, Handle::Pending(receiver));
        service.handles.insert(2, Handle::Failed);
        for (id, status) in [(1u32, 2), (2, 3), (3, 1)] {
            let mut request = id.to_le_bytes().to_vec();
            request.extend(b"unsent");
            assert_eq!(service.call(4, &request, 4), [status]);
            assert_eq!(service.used, 0);
        }
    }
    #[test]
    fn receive_quota_preserves_tcp_bytes_and_eof() {
        let (client, mut peer) = tcp_pair();
        peer.write_all(b"abcdef").unwrap();
        peer.shutdown(std::net::Shutdown::Write).unwrap();
        let mut service = ready_service(Resource::Tcp(client));
        service.used = 256 * 1024 - 2;
        assert_eq!(until(|| service.call(5, &1u32.to_le_bytes(), 8)), b"\0ab");
        assert_eq!(service.used, 256 * 1024);
        assert_eq!(service.call(5, &1u32.to_le_bytes(), 8), [2]);
        service.window = Instant::now() - Duration::from_secs(1);
        assert_eq!(until(|| service.call(5, &1u32.to_le_bytes(), 8)), b"\0cdef");
        assert_eq!(service.used, 4);
        service.used = 256 * 1024;
        assert_eq!(until(|| service.call(5, &1u32.to_le_bytes(), 8)), [0]);
        assert_eq!(service.used, 256 * 1024);
    }
    #[test]
    fn receive_quota_preserves_udp_datagrams_and_counts_actual_sizes() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_nonblocking(true).unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .send_to(b"abcdef", receiver.local_addr().unwrap())
            .unwrap();
        sender
            .send_to(b"xy", receiver.local_addr().unwrap())
            .unwrap();
        let address = receiver.local_addr().unwrap();
        let mut service = ready_service(Resource::Udp(receiver));
        service.used = 256 * 1024 - 2;
        assert_eq!(service.call(5, &1u32.to_le_bytes(), 8), [2]);
        assert_eq!(service.used, 256 * 1024 - 2);
        service.window = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            until(|| service.call(5, &1u32.to_le_bytes(), 8)),
            b"\0abcdef"
        );
        service.used = 256 * 1024 - 2;
        assert_eq!(until(|| service.call(5, &1u32.to_le_bytes(), 64)), b"\0xy");
        assert_eq!(service.used, 256 * 1024);
        sender.send_to(b"", address).unwrap();
        assert_eq!(until(|| service.call(5, &1u32.to_le_bytes(), 8)), [0]);
        assert_eq!(service.used, 256 * 1024);
    }
    #[test]
    fn send_quota_charges_only_transferred_bytes_and_defers_whole_datagrams() {
        let (client, mut peer) = tcp_pair();
        let mut service = ready_service(Resource::Tcp(client));
        let mut request = 1u32.to_le_bytes().to_vec();
        request.extend(b"abcdef");
        service.used = 256 * 1024 - 2;
        assert_eq!(handle(until(|| service.call(4, &request, 4))), 2);
        let mut bytes = [0; 2];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ab");
        assert_eq!(service.used, 256 * 1024);
        let mut request = 1u32.to_le_bytes().to_vec();
        request.extend(b"cdef");
        assert_eq!(service.call(4, &request, 4), [2]);
        service.window = Instant::now() - Duration::from_secs(1);
        assert_eq!(handle(until(|| service.call(4, &request, 4))), 4);
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"cdef");
        assert_eq!(service.used, 4);

        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_nonblocking(true).unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.connect(peer.local_addr().unwrap()).unwrap();
        let mut service = ready_service(Resource::Udp(client));
        let mut oversized = 1u32.to_le_bytes().to_vec();
        oversized.extend(vec![0; MAX_BYTES]);
        assert_eq!(service.call(4, &oversized, 4), [3]);
        assert_eq!(service.used, 0, "failed send consumed transfer budget");
        service.used = 256 * 1024 - 2;
        assert_eq!(service.call(4, &request, 4), [2]);
        let mut bytes = [0; 8];
        assert_eq!(
            peer.recv(&mut bytes).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        request.truncate(6);
        assert_eq!(handle(service.call(4, &request, 4)), 2);
        assert_eq!(peer.recv(&mut bytes).unwrap(), 2);
        assert_eq!(&bytes[..2], b"cd");
        assert_eq!(service.used, 256 * 1024);
    }
    #[test]
    fn rejected_reservations_preserve_the_open_rate_and_handle_sequence() {
        let mut permissions = permissions();
        permissions.network = true;
        let mut service = Service::new(permissions);
        for id in 1..=MAX_HANDLES as u32 {
            service.handles.insert(id, Handle::Failed);
        }
        service.next = MAX_HANDLES as u32 + 1;
        for _ in 0..8 {
            assert!(service.reserve().is_err());
        }
        assert_eq!(service.opens, 0);
        assert_eq!(service.next, MAX_HANDLES as u32 + 1);
        service.handles.remove(&1);
        assert_eq!(service.reserve().unwrap(), MAX_HANDLES as u32 + 1);
        assert_eq!(service.opens, 1);
        service.handles.clear();
        service.next = u32::MAX - 1;
        assert_eq!(service.reserve().unwrap(), u32::MAX - 1);
        assert_eq!(service.next, u32::MAX);
        let opens = service.opens;
        assert!(service.reserve().is_err());
        assert_eq!(service.opens, opens);
        assert_eq!(service.next, u32::MAX);
    }
    #[test]
    fn invalid_accepts_remain_errors_even_when_open_capacity_is_exhausted() {
        let mut service = ready_service(Resource::Https(Vec::new()));
        service.handles.insert(2, Handle::Failed);
        service.opens = 8;
        for (request, expected) in [
            (vec![0], 1),
            (1u32.to_le_bytes().to_vec(), 1),
            (2u32.to_le_bytes().to_vec(), 3),
            (3u32.to_le_bytes().to_vec(), 1),
        ] {
            assert_eq!(service.call(8, &request, 4), [expected]);
            assert_eq!(service.opens, 8);
            assert_eq!(service.next, 2);
        }
    }
    #[test]
    fn accept_keeps_connections_queued_until_handle_or_rate_capacity_returns() {
        for handles_full in [true, false] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let mut service = ready_service(Resource::Listener(listener));
            let mut peer = TcpStream::connect(address).unwrap();
            peer.write_all(b"ping").unwrap();
            if handles_full {
                for id in 2..=MAX_HANDLES as u32 {
                    service.handles.insert(id, Handle::Failed);
                }
                service.next = MAX_HANDLES as u32 + 1;
            } else {
                service.opens = 8;
            }
            assert_eq!(
                service.call(8, &1u32.to_le_bytes(), 4),
                if handles_full { vec![1] } else { vec![2] }
            );
            assert_eq!(
                service.handles.len(),
                if handles_full { MAX_HANDLES } else { 1 }
            );
            if handles_full {
                assert_eq!(service.call(6, &2u32.to_le_bytes(), 0), [0]);
            } else {
                service.window = Instant::now() - Duration::from_secs(1);
            }
            let accepted = handle(until(|| service.call(8, &1u32.to_le_bytes(), 4)));
            assert_ne!(accepted, 1);
            assert_eq!(
                until(|| service.call(5, &accepted.to_le_bytes(), 8)),
                b"\0ping"
            );
            assert_eq!(service.opens, 1);
            assert_eq!(service.call(6, &accepted.to_le_bytes(), 0), [0]);
            assert_eq!(service.call(5, &accepted.to_le_bytes(), 8), [1]);
        }
    }
    #[test]
    fn enabled_tcp_listener_accepts_without_declared_bind_addresses() {
        let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reservation.local_addr().unwrap();
        drop(reservation);
        let mut p = permissions();
        p.network = true;
        let mut service = Service::new(p);
        let listener = handle(service.call(7, address.to_string().as_bytes(), 4));
        let mut peer = TcpStream::connect(address).unwrap();
        peer.write_all(b"ping").unwrap();
        let connection = handle(until(|| service.call(8, &listener.to_le_bytes(), 4)));
        assert_eq!(
            until(|| service.call(5, &connection.to_le_bytes(), 16)),
            b"\0ping"
        );
        assert_eq!(service.call(6, &connection.to_le_bytes(), 0), [0]);
        assert_eq!(service.call(6, &listener.to_le_bytes(), 0), [0]);
    }
    #[test]
    fn tcp_falls_back_after_a_refused_resolved_address() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let reachable = listener.local_addr().unwrap();
        let refused = SocketAddr::new("127.0.0.2".parse().unwrap(), reachable.port());
        let mut stream = connect_tcp(&[refused, reachable]).unwrap();
        stream.write_all(b"ping").unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut bytes = [0; 4];
        peer.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ping");
        assert!(connect_tcp(&[]).is_err());
        assert!(connect_tcp(&[reachable; 65]).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn udp_setup_falls_back_to_reachable_ipv4_and_ipv6_addresses() {
        assert!(connect_udp(&[]).is_err());
        for bind in ["127.0.0.1:0", "[::1]:0"] {
            let peer = UdpSocket::bind(bind).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let reachable = peer.local_addr().unwrap();
            // A link-local IPv6 destination requires an interface scope ID.
            let invalid = SocketAddr::new("fe80::1".parse().unwrap(), reachable.port());
            let socket = connect_udp(&[invalid, reachable]).unwrap();
            assert_eq!(socket.peer_addr().unwrap(), reachable);
            assert_eq!(socket.send(b"ping").unwrap(), 4);
            let mut bytes = [0; 4];
            let (count, _) = peer.recv_from(&mut bytes).unwrap();
            assert_eq!(count, 4);
            assert_eq!(&bytes, b"ping");
            assert!(connect_udp(&[reachable; 65]).is_err());
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn https_resolved_fallback_and_numeric_ipv6_reach_a_verified_local_server() {
        use std::process::{Child, Command, Stdio};
        struct Server(Child);
        impl Drop for Server {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let temporary = tempfile::tempdir().unwrap();
        let certificate = temporary.path().join("certificate.pem");
        let key = temporary.path().join("key.pem");
        let output = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "1",
                "-subj",
                "/CN=gamepkg.test",
                "-addext",
                "subjectAltName=DNS:gamepkg.test,IP:127.0.0.1,IP:::1",
                "-keyout",
            ])
            .arg(&key)
            .arg("-out")
            .arg(&certificate)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for bind in ["127.0.0.1:0", "[::1]:0"] {
            let reservation = TcpListener::bind(bind).unwrap();
            let address = reservation.local_addr().unwrap();
            drop(reservation);
            let mut server = Server(
                Command::new("openssl")
                    .args(["s_server", "-quiet", "-www", "-accept"])
                    .arg(address.to_string())
                    .arg("-cert")
                    .arg(&certificate)
                    .arg("-key")
                    .arg(&key)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap(),
            );
            let started = Instant::now();
            while TcpStream::connect_timeout(&address, Duration::from_millis(20)).is_err() {
                assert!(server.0.try_wait().unwrap().is_none(), "TLS server exited");
                assert!(
                    started.elapsed() < Duration::from_secs(2),
                    "TLS server startup"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let (endpoint, url, addresses) = if address.is_ipv4() {
                (
                    Endpoint::parse(&format!("gamepkg.test:{}", address.port())).unwrap(),
                    format!("https://gamepkg.test:{}/", address.port()),
                    vec![
                        SocketAddr::new("127.0.0.2".parse().unwrap(), address.port()),
                        address,
                    ],
                )
            } else {
                (
                    Endpoint::parse(&address.to_string()).unwrap(),
                    format!("https://{address}/"),
                    vec![address],
                )
            };
            let mut command = endpoint.https_command(&url, &addresses).unwrap();
            let args = command
                .get_args()
                .map(|arg| arg.to_str().unwrap())
                .collect::<Vec<_>>();
            if address.is_ipv4() {
                assert!(args.windows(2).any(|pair| pair
                    == [
                        "--resolve",
                        &format!("gamepkg.test:{}:127.0.0.2,127.0.0.1", address.port())
                    ]));
            } else {
                assert!(!args.contains(&"--resolve"));
            }
            assert!(!args.contains(&"--insecure"));
            let output = crate::compiler::run_timed(&mut command, Duration::from_secs(6)).unwrap();
            assert_eq!(output.status.code(), Some(60), "{}", output.stderr);
            // Trust this test's synthetic CA only. Production command construction
            // never supplies a CA override or disables certificate verification.
            command.arg("--cacert").arg(&certificate);
            let output = crate::compiler::run_timed(&mut command, Duration::from_secs(6)).unwrap();
            assert!(output.status.success(), "{}", output.stderr);
            assert!(output.stdout.contains("s_server"), "{}", output.stdout);
            assert!(output.stdout_bytes.len() <= MAX_BYTES);
        }
    }
    #[test]
    fn local_endpoints_need_only_network_enabled_and_transfer_budgets_are_bounded() {
        let mut p = permissions();
        p.network = true;
        assert!(p.network_hosts.is_empty());
        let mut service = Service::new(p);
        let id = handle(service.call(3, b"127.0.0.1:1234", 4));
        assert_eq!(until(|| service.call(1, &id.to_le_bytes(), 0)), [0]);
        assert_eq!(service.call(5, &id.to_le_bytes(), 65537), [1]);
        for _ in 0..4 {
            service.budget(65536).unwrap();
        }
        assert!(service.budget(1).is_err());
    }
    #[test]
    #[ignore = "performs an actual public HTTPS GET"]
    fn real_https_get_preserves_bounded_response() {
        let mut p = permissions();
        p.network = true;
        assert!(p.network_hosts.is_empty() && p.network_listen.is_empty());
        assert!(!p.network_allow_local);
        let mut service = Service::new(p);
        let id = handle(service.call(0, b"https://example.com/", 4));
        let end = Instant::now() + Duration::from_secs(8);
        loop {
            let reply = service.call(1, &id.to_le_bytes(), 65536);
            if reply == [2] {
                assert!(Instant::now() < end);
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            assert_eq!(reply.first(), Some(&0));
            assert!(std::str::from_utf8(&reply[1..])
                .unwrap()
                .contains("Example Domain"));
            break;
        }
    }
}
