//! Real original-engine retry with the production clock and TCP broker.
use super::*;
#[test]
#[ignore = "requires original data, compiler/runtime namespaces and five real minutes"]
fn original_master_recovers_after_real_refusal_and_five_minute_backoff() {
    use std::net::{TcpListener, UdpSocket};
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
    let binary = super::super::build::compile().unwrap();
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let (port, first, second) = loop {
        let first = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = first.local_addr().unwrap().port();
        if port < 65535 && port != address.port() {
            if let Ok(second) = UdpSocket::bind(("127.0.0.1", port + 1)) {
                break (port, first, second);
            }
        }
    };
    let permissions =
        crate::manifest::Manifest::parse(include_bytes!("../../../ports/cube2/manifest.toml"))
            .unwrap()
            .permissions;
    drop(first);
    drop(second);
    drop(reserved);
    let started = Instant::now();
    let mut session = Session::start(Path::new(&data), 2 * 1024 * 1048576, Duration::from_secs(60), &[], || Ok(binary), Some(permissions.into()),
        format!("; mastername \"127.0.0.1\"; masterport {}; mode 0; map complex; serverip \"127.0.0.1\"; serverport {port}; startlistenserver 1",address.port())).unwrap();
    let refused_by = Instant::now() + Duration::from_secs(20);
    loop {
        session.tick(0, 0, 0).unwrap();
        let diagnostic = session.diagnostics.lock().unwrap();
        if String::from_utf8_lossy(&diagnostic).contains("could not connect to master server") {
            break;
        }
        assert!(
            Instant::now() < refused_by,
            "No original refusal diagnostic: {}",
            String::from_utf8_lossy(&diagnostic)
        );
    }
    session.set_deadline(Duration::from_millis(500)).unwrap();
    let listener = TcpListener::bind(address).unwrap();
    listener.set_nonblocking(true).unwrap();
    let deadline = started + Duration::from_secs(335);
    let (mut stream, _) = loop {
        session.tick(0, 0, 0).unwrap();
        match listener.accept() {
            Ok(connection) => {
                assert!(
                    started.elapsed() >= Duration::from_secs(300),
                    "Original backoff was bypassed"
                );
                break connection;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("master accept: {error}"),
        }
        assert!(
            Instant::now() < deadline,
            "Original engine never retried: {}",
            String::from_utf8_lossy(&session.diagnostics.lock().unwrap())
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    stream.set_nonblocking(true).unwrap();
    let mut received = Vec::new();
    let registration_by = Instant::now() + Duration::from_secs(10);
    loop {
        session.tick(0, 0, 0).unwrap();
        let mut bytes = [0; 128];
        match stream.read(&mut bytes) {
            Ok(0) => panic!("Retry connection closed before registration"),
            Ok(size) => received.extend_from_slice(&bytes[..size]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("registration read: {error}"),
        }
        if received.ends_with(b"\n") {
            break;
        }
        assert!(
            Instant::now() < registration_by,
            "No registration after retry"
        );
    }
    assert_eq!(received, format!("regserv {port}\n").as_bytes());
    stream.write_all(b"succreg\n").unwrap();
    for _ in 0..10 {
        session.tick(0, 0, 0).unwrap();
    }
    session.snapshot().unwrap();
}
