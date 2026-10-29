//! Original demo recording, scoped persistence and playback across restart.
use super::*;
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_demo_recording_survives_snapshot_and_plays_after_restart() {
    verify_demo_restart("demo", "PortFidelity.dmo");
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_root_demo_with_uppercase_extension_survives_restart() {
    verify_demo_restart("", "PortRoot.DMO");
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_custom_demo_directory_survives_restart() {
    verify_demo_restart("recordings", "PortCustom.dMo");
}
fn verify_demo_restart(directory: &str, filename: &str) {
    let saved_path = if directory.is_empty() {
        filename.to_owned()
    } else {
        format!("{directory}/{filename}")
    };
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
    let binary = super::super::build::compile().unwrap();
    let config = format!("exec data/defaults.cfg\nbind F9 [stopdemo; getdemo 1 {filename}]\n");
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config.as_bytes());
    let mut session = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
        || Ok(binary.clone()),
        None,
        format!("; demodir \"{directory}\"; autorecorddemo 1; mode 0; map complex"),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..120 {
        session.tick(1 | (1 << 16), 3, 0).unwrap();
    }
    session.key(66, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(66, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let saved = loop {
        let saved = session.snapshot().unwrap();
        if protocol::save_files(&saved)
            .unwrap()
            .iter()
            .any(|file| file.path == saved_path)
        {
            break saved;
        }
        assert!(
            Instant::now() < deadline,
            "No recorded demo was saved: {}",
            String::from_utf8_lossy(&session.diagnostics.lock().unwrap())
        );
        session.tick(0, 0, 0).unwrap();
    };
    let files = protocol::save_files(&saved).unwrap();
    let demo = files
        .iter()
        .find(|file| file.path == saved_path)
        .unwrap()
        .bytes
        .to_vec();
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(demo.as_slice())
        .take(1048576)
        .read_to_end(&mut decoded)
        .unwrap();
    assert!(
        decoded.len() > 36,
        "Recording has no original packet stream"
    );
    assert_eq!(&decoded[..16], b"SAUERBRATEN_DEMO");
    assert_eq!(u32::from_le_bytes(decoded[16..20].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(decoded[20..24].try_into().unwrap()), 260);
    let mut at = 24usize;
    let mut packets = 0usize;
    let mut previous_stamp = 0i32;
    while at < decoded.len() {
        assert!(at + 12 <= decoded.len(), "Truncated demo packet header");
        let stamp = i32::from_le_bytes(decoded[at..at + 4].try_into().unwrap());
        let channel = i32::from_le_bytes(decoded[at + 4..at + 8].try_into().unwrap());
        let length = i32::from_le_bytes(decoded[at + 8..at + 12].try_into().unwrap());
        assert!(stamp >= previous_stamp && (0..=2).contains(&channel) && length > 0);
        previous_stamp = stamp;
        at += 12 + usize::try_from(length).unwrap();
        assert!(at <= decoded.len(), "Truncated demo packet body");
        packets += 1;
    }
    assert!(
        packets > 1 && previous_stamp > 0,
        "Recording contains no timed gameplay"
    );
    let saved = crate::cube2::recordings::test_import_roundtrip(&saved, &saved_path, &demo);
    drop(session);
    let mut replay = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &saved,
        || Ok(binary),
        None,
        format!("; demodir \"{directory}\"; autorecorddemo 0; demo {filename}"),
    )
    .unwrap();
    replay.tick(0, 0, 0).unwrap();
    replay.set_deadline(Duration::from_millis(500)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        replay.tick(0, 0, 0).unwrap();
        let diagnostics = replay.diagnostics.lock().unwrap();
        let message = String::from_utf8_lossy(&diagnostics);
        assert!(
            !message.contains("could not read demo") && !message.contains("is not a demo file"),
            "{message}"
        );
        if message.contains(&format!("playing demo \"{filename}\"")) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Original playback never started: {message}"
        );
    }
    use sha2::{Digest, Sha256};
    let mut views = std::collections::BTreeSet::new();
    for _ in 0..60 {
        let frame = replay.tick(0, 0, 0).unwrap();
        assert_eq!(
            frame.rgba.len(),
            frame.width as usize * frame.height as usize * 4
        );
        views.insert(Sha256::digest(&frame.rgba));
    }
    assert!(
        views.len() > 1,
        "Demo playback never changed its rendered frame"
    );
    let roundtrip = replay.snapshot().unwrap();
    let files = protocol::save_files(&roundtrip).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == saved_path)
            .unwrap()
            .bytes,
        demo
    );
}
