//! Maintainer regression: two original engines on a private, isolated Ethernet LAN.
use super::*;
const TEST: &str = "cube2::session::lan::original_lan_discovery_on_private_network";
const PORT: u16 = 32123;

fn seed(config: &str) -> Vec<u8> {
    let mut saved = b"GPS1".to_vec();
    saved.extend((config.len() as u32).to_le_bytes());
    saved.extend([0; 8]);
    saved.extend(config.as_bytes());
    saved
}
fn press(session: &mut Session, key: u32) {
    session.key(key, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(key, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
}
fn saved_text(saved: &[u8], name: &str) -> String {
    let files = protocol::save_files(saved).unwrap();
    files
        .iter()
        .find(|file| file.path == name)
        .map(|file| String::from_utf8(file.bytes.to_vec()).unwrap())
        .unwrap_or_default()
}
fn screenshot(session: &mut Session, proof: &Path, name: &str) {
    let frame = session.tick(0, 0, 0).unwrap();
    image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba)
        .unwrap()
        .save(proof.join(name))
        .unwrap();
}
fn role(role: &str, state: &Path, proof: &Path, binary: &[u8], data: &Path) {
    let mut permissions =
        crate::manifest::Manifest::parse(include_bytes!("../../../ports/cube2/manifest.toml"))
            .unwrap()
            .permissions;
    assert!(permissions.network_hosts.is_empty() && permissions.network_listen.is_empty());
    if role == "server" {
        let config = seed("exec data/defaults.cfg\nname LANHost\nbind F11 [name (getclientname (getclientnum LANClient1))]\n");
        let mut session = Session::start(data, 2 * 1024 * 1048576, Duration::from_secs(60), &config, || Ok(binary.to_vec()), Some(permissions.into()),
            format!("; mastername \"\"; serverip \"\"; serverport {PORT}; serverdesc GamePKGLANTest; mode 0; map complex; startlistenserver 0")).unwrap();
        session.tick(0, 0, 0).unwrap();
        // Cold remote resources must announce loading under the normal tick budget.
        session.set_deadline(Duration::from_millis(500)).unwrap();
        fs::write(state.join("server-ready"), b"ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(100);
        let mut observed = false;
        while !state.join("stop").exists() {
            assert!(Instant::now() < deadline, "LAN client timed out");
            session.tick(0, 0, 0).unwrap();
            if !observed && state.join("client-connected").exists() {
                press(&mut session, 68);
                let saved = session.snapshot().unwrap();
                fs::write(
                    proof.join("server-identity.cfg"),
                    saved_text(&saved, "config.cfg"),
                )
                .unwrap();
                fs::write(
                    proof.join("server-diagnostics.log"),
                    session.diagnostics.lock().unwrap().as_slice(),
                )
                .unwrap();
                if saved_text(&saved, "config.cfg").contains("name \"LANClient1\"") {
                    observed = true;
                    fs::write(state.join("server-observed"), b"ready").unwrap();
                }
            }
        }
        assert!(
            observed,
            "server never received the discovered client's identity"
        );
        session.snapshot().unwrap();
        session.close().unwrap();
        return;
    }
    assert_eq!(role, "client");
    let deadline = Instant::now() + Duration::from_secs(60);
    while !state.join("server-ready").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    for (phase, enabled, search) in [
        ("offline", false, 1),
        ("search-off", true, 0),
        ("search-on", true, 1),
    ] {
        fs::write(state.join("phase"), phase).unwrap();
        permissions.network = enabled;
        let config = seed(&format!("exec data/defaults.cfg\nname LANClient0\nguisens 1\nsearchlan {search}\nautoupdateservers 0\nservpingrate 1000\nbind F10 [clearservers 1; showgui serverbrowser]\nbind F11 [name (concatword LANClient (&& (isconnected 0 0) (=s (getclientname (getclientnum LANHost)) LANHost)))]\n"));
        let mut session = Session::start(
            data,
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &config,
            || Ok(binary.to_vec()),
            Some(permissions.clone().into()),
            "; mastername \"\"".into(),
        )
        .unwrap();
        session.tick(0, 0, 0).unwrap();
        session.set_deadline(Duration::from_millis(500)).unwrap();
        press(&mut session, 67);
        let until =
            Instant::now() + Duration::from_secs(if enabled && search == 1 { 12 } else { 2 });
        let mut discovered = false;
        let mut frames = 0;
        while Instant::now() < until {
            session.tick(0, 0, 0).unwrap();
            frames += 1;
            if frames % 10 == 0 {
                let saved = session.snapshot().unwrap();
                let list = saved_text(&saved, "servers.cfg");
                discovered = list.contains(&format!("10.77.0.1 {PORT}"));
                if discovered {
                    break;
                }
                assert!(
                    !list.contains("addserver "),
                    "unexpected server in phase {phase}: {list}"
                );
            }
        }
        screenshot(&mut session, proof, &format!("{phase}.png"));
        let saved = session.snapshot().unwrap();
        fs::write(
            proof.join(format!("{phase}-servers.cfg")),
            saved_text(&saved, "servers.cfg"),
        )
        .unwrap();
        if !(enabled && search == 1) {
            assert!(
                !discovered,
                "disabled LAN discovery found a server in {phase}"
            );
            session.close().unwrap();
            continue;
        }
        assert!(
            discovered,
            "LAN broadcast never populated the original browser: {}",
            String::from_utf8_lossy(&session.diagnostics.lock().unwrap())
        );
        // Original 2D GUI starts at its centered cursor. Move to the server row
        // with normal relative input, then click the original browser entry.
        session.tick(0, 0, 25).unwrap();
        session.tick(1 << 16, 0, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        let until = Instant::now() + Duration::from_secs(12);
        let mut connected = false;
        while Instant::now() < until {
            session.tick(0, 0, 0).unwrap();
            press(&mut session, 68);
            let saved = session.snapshot().unwrap();
            if saved_text(&saved, "config.cfg").contains("name \"LANClient1\"") {
                connected = true;
                break;
            }
        }
        screenshot(&mut session, proof, "connected.png");
        assert!(
            connected,
            "original browser click did not join the discovered server: {}",
            String::from_utf8_lossy(&session.diagnostics.lock().unwrap())
        );
        fs::write(state.join("client-connected"), b"ready").unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        while !state.join("server-observed").exists() {
            assert!(Instant::now() < until);
            session.tick(0, 0, 0).unwrap();
        }
        session.snapshot().unwrap();
        session.close().unwrap();
    }
    fs::write(state.join("client-finished"), b"ready").unwrap();
}

#[test]
#[ignore = "requires original data, compiler and private user/network namespaces"]
fn original_lan_discovery_on_private_network() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    if let Ok(which) = std::env::var("GAMEPKG_LAN_TEST_ROLE") {
        let state = std::env::var_os("GAMEPKG_LAN_TEST_STATE").unwrap();
        let proof = std::env::var_os("GAMEPKG_LAN_TEST_PROOF").unwrap();
        let binary = fs::read(std::env::var_os("GAMEPKG_LAN_TEST_ENGINE").unwrap()).unwrap();
        assert_ne!(
            std::env::var("GAMEPKG_LAN_PARENT_NETNS").unwrap(),
            fs::read_link("/proc/self/ns/net")
                .unwrap()
                .to_string_lossy()
        );
        role(
            &which,
            Path::new(&state),
            Path::new(&proof),
            &binary,
            Path::new(&data),
        );
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let engine = temp.path().join("engine");
    fs::write(&engine, super::super::build::compile().unwrap()).unwrap();
    let proof_base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/cube2-lan-proof");
    let proof = proof_base.join(format!(
        "run-{}",
        temp.path().file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&proof).unwrap();
    fs::write(
        proof_base.join("current.txt"),
        proof.to_string_lossy().as_bytes(),
    )
    .unwrap();
    let mut command = std::process::Command::new("python3");
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cube2_lan.py"))
        .arg("--engine")
        .arg(engine)
        .arg("--binary")
        .arg(std::env::current_exe().unwrap())
        .arg("--state")
        .arg(temp.path())
        .arg("--proof")
        .arg(&proof)
        .arg("--test")
        .arg(TEST);
    let output = crate::compiler::run_timed(&mut command, Duration::from_secs(160)).unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        output.stdout,
        output.stderr
    );
    fs::write(
        proof_base.join("verified.txt"),
        proof.to_string_lossy().as_bytes(),
    )
    .unwrap();
}
