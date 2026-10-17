//! Host-owned, bounded master-list retrieval. No master text is executed.
use anyhow::{ensure, Context, Result};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
const MAX_BYTES: usize = 65536;
const MAX_SERVERS: usize = 512;
static JOBS: AtomicUsize = AtomicUsize::new(0);
struct Job;
impl Job {
    fn acquire() -> Result<Self> {
        let previous = JOBS.fetch_add(1, Ordering::SeqCst);
        if previous >= 16 {
            JOBS.fetch_sub(1, Ordering::SeqCst);
            anyhow::bail!("too many master requests");
        }
        Ok(Self)
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        JOBS.fetch_sub(1, Ordering::SeqCst);
    }
}
#[derive(Clone)]
pub(super) struct Master {
    pub name: String,
    pub address: SocketAddr,
    pub local: bool,
}
pub(super) struct List {
    pub bytes: Vec<u8>,
    pub endpoints: BTreeSet<SocketAddr>,
}
impl Master {
    pub fn resolve(text: &str, local: bool) -> Result<Self> {
        let policy = crate::network::selected_multiplayer(text, "", local)?;
        let (name, address) = crate::network::multiplayer_endpoints(&policy, false)?
            .into_iter()
            .next()
            .context("master address missing")?;
        Ok(Self {
            name,
            address,
            local,
        })
    }
    pub fn fetch(&self) -> Result<List> {
        let _job = Job::acquire()?;
        let limit = Duration::from_secs(5);
        let start = Instant::now();
        let mut stream = TcpStream::connect_timeout(&self.address, Duration::from_secs(2))?;
        stream.set_write_timeout(Some(Duration::from_secs(1)))?;
        stream.write_all(b"list\n")?;
        let mut bytes = Vec::new();
        loop {
            let remaining = limit
                .checked_sub(start.elapsed())
                .context("master request timed out")?;
            stream.set_read_timeout(Some(remaining))?;
            let mut part = [0; 4096];
            let n = stream.read(&mut part)?;
            if n == 0 {
                break;
            }
            ensure!(bytes.len() + n <= MAX_BYTES, "master list exceeds quota");
            bytes.extend_from_slice(&part[..n]);
        }
        parse(&bytes, self.local)
    }
}
fn parse(bytes: &[u8], local: bool) -> Result<List> {
    ensure!(bytes.len() <= MAX_BYTES, "master list exceeds quota");
    let text = std::str::from_utf8(bytes)?;
    ensure!(
        text.is_empty() || text.ends_with('\n'),
        "truncated master list"
    );
    let mut servers = BTreeSet::new();
    for line in text.lines() {
        ensure!(
            line.len() <= 512 && !line.chars().any(|c| c.is_control() && c != '\t'),
            "invalid master line"
        );
        let mut fields = line.split_ascii_whitespace();
        if fields.next() != Some("addserver") {
            continue;
        }
        let Some(ip) = fields.next().and_then(|s| s.parse::<Ipv4Addr>().ok()) else {
            continue;
        };
        let Some(port) = fields.next().and_then(|s| s.parse::<u16>().ok()) else {
            continue;
        };
        if fields.next().is_some()
            || !(1..65535).contains(&port)
            || !crate::network::allowed_ip(ip.into(), local)
        {
            continue;
        }
        servers.insert(SocketAddrV4::new(ip, port));
        ensure!(servers.len() <= MAX_SERVERS, "too many discovered servers");
    }
    let mut output = List {
        bytes: Vec::new(),
        endpoints: BTreeSet::new(),
    };
    for server in servers {
        output
            .bytes
            .extend_from_slice(format!("addserver {} {}\n", server.ip(), server.port()).as_bytes());
        output.endpoints.insert(server.into());
        output
            .endpoints
            .insert(SocketAddrV4::new(*server.ip(), server.port() + 1).into());
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_master_text_cannot_add_commands_names_private_or_unbounded_grants() {
        let list = parse(b"echo hello\nquit\naddserver 8.8.8.8 28785\naddserver 8.8.8.8 28785\naddserver evil.example 1\naddserver 127.0.0.1 2\naddserver 0.0.0.0 2\naddserver 224.0.0.1 2\naddserver 8.8.4.4 0\naddserver 8.8.4.4 65535\naddserver 8.8.4.4 2;quit\naddserver 8.8.4.4 2 extra\n", false).unwrap();
        assert_eq!(list.bytes, b"addserver 8.8.8.8 28785\n");
        assert_eq!(list.endpoints.len(), 2);
        assert!(parse(b"addserver 8.8.8.8 1", false).is_err());
        assert!(parse(b"echo a\0b\n", false).is_err());
        assert!(parse(&[b'x'; MAX_BYTES + 1], false).is_err());
        assert!(parse(&[b'x'; 513], false).is_err());
        let many = (0..513)
            .map(|n| format!("addserver 8.8.8.8 {}\n", n + 1))
            .collect::<String>();
        assert!(parse(many.as_bytes(), false).is_err());
        assert_eq!(
            parse(b"addserver 127.0.0.1 2\n", true)
                .unwrap()
                .endpoints
                .len(),
            2
        );
    }
    #[test]
    fn private_mock_master_receives_only_list_and_returns_sanitized_endpoints() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(&request, b"list\n");
            stream
                .write_all(b"echo harmless\naddserver 127.0.0.1 28785\n")
                .unwrap();
        });
        let master = Master::resolve(&address.to_string(), true).unwrap();
        assert_eq!(master.name, "127.0.0.1");
        let list = master.fetch().unwrap();
        assert_eq!(list.bytes, b"addserver 127.0.0.1 28785\n");
        worker.join().unwrap();
        assert!(Master::resolve(&address.to_string(), false).is_err());
    }
}
