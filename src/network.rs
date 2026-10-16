//! Per-game, signed endpoint capabilities. Only the host owns sockets and HTTPS clients.
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
static JOBS: AtomicUsize = AtomicUsize::new(0);
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
    fn key(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
    fn resolve(&self, local: bool) -> Result<SocketAddr> {
        let hostname = self.host.trim_start_matches('[').trim_end_matches(']');
        let addresses = (hostname, self.port).to_socket_addrs()?.collect::<Vec<_>>();
        ensure!(
            !addresses.is_empty() && addresses.len() <= 64,
            "invalid DNS result"
        );
        ensure!(
            addresses.iter().all(|a| allowed_ip(a.ip(), local)),
            "local, private or reserved network address denied"
        );
        Ok(addresses[0])
    }
}
fn allowed_ip(ip: IpAddr, local: bool) -> bool {
    if ip.is_unspecified() || ip.is_multicast() {
        return false;
    }
    match ip {
        IpAddr::V4(a) => {
            let [x, y, _, _] = a.octets();
            if a.is_broadcast() || x == 0 || x >= 224 {
                return false;
            }
            local
                || !(a.is_loopback()
                    || a.is_private()
                    || a.is_link_local()
                    || x == 100 && (64..=127).contains(&y)
                    || x == 192 && y == 0
                    || x == 198 && (18..=19).contains(&y)
                    || a.is_documentation())
        }
        IpAddr::V6(a) => {
            if let Some(v) = a.to_ipv4_mapped() {
                return allowed_ip(v.into(), local);
            }
            local
                || !(a.segments()[0] & 0xe000 != 0x2000
                    || a.segments()[0] == 0x2002
                    || (a.segments()[0] == 0x2001 && a.segments()[1] == 0)
                    || a.is_loopback()
                    || a.is_unique_local()
                    || a.is_unicast_link_local()
                    || a.segments()[0] == 0x2001 && a.segments()[1] == 0xdb8)
        }
    }
}
pub(crate) fn validate(p: &crate::manifest::Permissions) -> Result<()> {
    ensure!(
        matches!(p.network.as_str(), "none" | "https" | "multiplayer" | "all"),
        "unknown network permission"
    );
    ensure!(
        p.network_hosts.len() <= 16 && p.network_listen.len() <= 4,
        "too many network endpoints"
    );
    if p.network == "none" {
        ensure!(
            p.network_hosts.is_empty() && p.network_listen.is_empty() && !p.network_allow_local,
            "network endpoints require permission"
        );
        return Ok(());
    }
    ensure!(
        !p.network_hosts.is_empty() || !p.network_listen.is_empty(),
        "network permission requires explicit endpoints"
    );
    for text in &p.network_hosts {
        Endpoint::parse(text)?;
    }
    for text in &p.network_listen {
        let e = Endpoint::parse(text)?;
        let ip: IpAddr = e
            .host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse()
            .context("listening requires an explicit numeric IP address")?;
        ensure!(
            allowed_ip(ip, p.network_allow_local),
            "listening address requires local-network permission"
        );
    }
    if p.network == "https" {
        ensure!(
            p.network_listen.is_empty(),
            "HTTPS permission cannot listen for peers"
        );
    }
    Ok(())
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
struct Job;
impl Drop for Job {
    fn drop(&mut self) {
        JOBS.fetch_sub(1, Ordering::SeqCst);
    }
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
    fn capable(&self, https: bool) -> Result<()> {
        ensure!(
            self.permissions.network == "all"
                || self.permissions.network == if https { "https" } else { "multiplayer" },
            "network permission denied"
        );
        Ok(())
    }
    fn endpoint(&self, text: &str, listen: bool) -> Result<Endpoint> {
        let e = Endpoint::parse(text)?;
        let list = if listen {
            &self.permissions.network_listen
        } else {
            &self.permissions.network_hosts
        };
        ensure!(
            list.iter()
                .any(|s| Endpoint::parse(s).is_ok_and(|p| p.key() == e.key())),
            "endpoint not declared by signed manifest"
        );
        Ok(e)
    }
    fn reset_budget(&mut self) {
        if self.window.elapsed() >= Duration::from_secs(1) {
            self.window = Instant::now();
            self.used = 0;
            self.opens = 0;
        }
    }
    fn reserve(&mut self) -> Result<u32> {
        self.reset_budget();
        ensure!(self.opens < 8, "network connections busy");
        self.opens += 1;
        ensure!(
            self.handles.len() < MAX_HANDLES && self.next < u32::MAX,
            "network handle limit reached"
        );
        let id = self.next;
        self.next += 1;
        Ok(id)
    }
    fn spawn(&mut self, job: impl FnOnce() -> Result<Resource> + Send + 'static) -> Result<u32> {
        let id = self.reserve()?;
        let previous = JOBS.fetch_add(1, Ordering::SeqCst);
        if previous >= 16 {
            JOBS.fetch_sub(1, Ordering::SeqCst);
            bail!("network workers busy");
        }
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _guard = Job;
            let _ = tx.send(job());
        });
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
        if self.window.elapsed() >= Duration::from_secs(1) {
            self.window = Instant::now();
            self.used = 0;
            self.opens = 0;
        }
        ensure!(
            n <= MAX_BYTES && self.used + n <= 256 * 1024,
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
                && u.port_or_known_default() == Some(443)
                && u.fragment().is_none(),
            "only HTTPS port 443 URLs without credentials or fragments are allowed"
        );
        let host = u.host_str().context("URL host missing")?.to_owned();
        let endpoint = self.endpoint(&format!("{host}:443"), false)?;
        let local = self.permissions.network_allow_local;
        let url = u.to_string();
        self.spawn(move || {
            let address = endpoint.resolve(local)?;
            let ip = if address.is_ipv6() {
                format!("[{}]", address.ip())
            } else {
                address.ip().to_string()
            };
            let mut command = std::process::Command::new("curl");
            command
                .env_clear()
                .args([
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
                    "--resolve",
                ])
                .arg(format!("{host}:443:{ip}"))
                .arg("--url")
                .arg(url);
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
        let local = self.permissions.network_allow_local;
        self.spawn(move || {
            let address = endpoint.resolve(local)?;
            if udp {
                let socket = UdpSocket::bind(if address.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                })?;
                socket.connect(address)?;
                socket.set_nonblocking(true)?;
                Ok(Resource::Udp(socket))
            } else {
                let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
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
                self.budget(bytes.len())?;
                let n = match self.refresh(id)? {
                    Resource::Tcp(s) => s.write(bytes)?,
                    Resource::Udp(s) => s.send(bytes)?,
                    _ => bail!("handle is not a socket"),
                };
                Ok(number(n as u32))
            }
            5 => {
                ensure!(capacity <= MAX_BYTES, "network transfer too large");
                let mut bytes = vec![0; capacity];
                let n = match self.refresh(id(b)?)? {
                    Resource::Tcp(s) => s.read(&mut bytes)?,
                    Resource::Udp(s) => s.recv(&mut bytes)?,
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
                let address = endpoint.resolve(self.permissions.network_allow_local)?;
                let socket = TcpListener::bind(address)?;
                socket.set_nonblocking(true)?;
                let id = self.reserve()?;
                self.handles
                    .insert(id, Handle::Ready(Resource::Listener(socket)));
                Ok(number(id))
            }
            8 => {
                let stream = match self.refresh(id(b)?)? {
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
                self.operation(1, &id.to_le_bytes(), capacity)
            }
            _ => bail!("unknown network operation"),
        }
    }
    pub fn call(&mut self, op: u8, b: &[u8], capacity: usize) -> Vec<u8> {
        match self.operation(op, b, capacity) {
            Ok(value) => value,
            Err(error) => {
                if matches!(
                    error.to_string().as_str(),
                    "pending" | "network connections busy"
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
    fn explicit_endpoints_local_permissions_and_no_redirect_or_credential_urls() {
        let mut p = permissions();
        assert!(validate(&p).is_ok());
        p.network = "all".into();
        assert!(validate(&p).is_err());
        p.network_hosts = vec!["example.com:443".into()];
        assert!(validate(&p).is_ok());
        for address in [
            "*.example.com:443",
            "user@example.com:443",
            "example.com:443/path",
            "example.com:0",
            "example.com:443?x",
            "example.com:443#x",
        ] {
            p.network_hosts = vec![address.into()];
            assert!(validate(&p).is_err(), "{address}");
        }
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fe80::1",
            "fc00::1",
            "2001:db8::1",
        ] {
            assert!(!allowed_ip(ip.parse().unwrap(), false), "{ip}");
        }
        p.network_hosts = vec!["example.com:443".into()];
        let mut s = Service::new(p);
        for url in [
            "http://example.com/",
            "https://user:password@example.com/",
            "https://example.com:444/",
            "https://evil.invalid/",
            "https://example.com/#secret",
        ] {
            assert_eq!(s.call(0, url.as_bytes(), 4), [1], "{url}");
        }
        let mut denied = Service::new(permissions());
        assert_eq!(denied.call(0, b"https://example.com/", 4), [1]);
        assert_eq!(denied.call(2, b"example.com:443", 4), [1]);
    }
    #[test]
    fn tcp_and_udp_transfer_only_to_declared_peers_and_handles_are_scoped() {
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
        p.network = "multiplayer".into();
        p.network_hosts = vec![address.to_string()];
        p.network_allow_local = true;
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
        assert_eq!(service.call(2, b"127.0.0.1:1", 4), [1]);
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
        p.network = "multiplayer".into();
        p.network_hosts = vec![address.to_string()];
        p.network_allow_local = true;
        let mut service = Service::new(p);
        let id = handle(service.call(3, address.to_string().as_bytes(), 4));
        assert_eq!(until(|| service.call(1, &id.to_le_bytes(), 0)), [0]);
        let mut request = id.to_le_bytes().to_vec();
        request.extend(b"ping");
        assert_eq!(handle(until(|| service.call(4, &request, 4))), 4);
        assert_eq!(until(|| service.call(5, &id.to_le_bytes(), 16)), b"\0pong");
        worker.join().unwrap();
    }
    #[test]
    fn private_endpoints_need_explicit_permission_and_transfer_budgets_are_bounded() {
        let mut p = permissions();
        p.network = "multiplayer".into();
        p.network_hosts = vec!["127.0.0.1:1234".into()];
        let mut service = Service::new(p);
        let id = handle(service.call(2, b"127.0.0.1:1234", 4));
        assert_eq!(until(|| service.call(1, &id.to_le_bytes(), 0)), [3]);
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
        p.network = "https".into();
        p.network_hosts = vec!["example.com:443".into()];
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
