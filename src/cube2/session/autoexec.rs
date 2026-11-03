//! Original user startup configuration must survive the scoped save boundary.
use super::*;
fn tap(session: &mut Session, key: u32) {
    session.key(key, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(key, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_autoexec_survives_snapshot_and_overrides_saved_config_after_restart() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let binary = super::super::build::compile().unwrap();
    let config=b"exec data/defaults.cfg\nname AutoexecSaved\nbind F9 [writecfg autoexec.cfg]\nbind F10 [name ConfigLater]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
        || Ok(binary.clone()),
        None,
        "; mode 0; map complex".into(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..20 {
        session.tick(0, 0, 0).unwrap();
    }
    tap(&mut session, 66); // Original writecfg writes a real scoped autoexec.cfg.
    tap(&mut session, 67); // Keep config.cfg distinguishable from that startup file.
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|file| file.path == "config.cfg").unwrap();
    assert!(String::from_utf8_lossy(config.bytes).contains("name \"ConfigLater\""));
    let autoexec = files
        .iter()
        .find(|file| file.path == "autoexec.cfg")
        .expect("Original autoexec.cfg was silently omitted from the snapshot")
        .bytes
        .to_vec();
    assert!(String::from_utf8_lossy(&autoexec).contains("name \"AutoexecSaved\""));
    drop(session);
    let mut restored = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &saved,
        || Ok(binary),
        None,
        "; mode 0; map complex".into(),
    )
    .unwrap();
    restored.tick(0, 0, 0).unwrap();
    restored.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..10 {
        restored.tick(0, 0, 0).unwrap();
    }
    let saved = restored.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == "autoexec.cfg")
            .unwrap()
            .bytes,
        autoexec
    );
    let config = files.iter().find(|file| file.path == "config.cfg").unwrap();
    assert!(
        String::from_utf8_lossy(config.bytes).contains("name \"AutoexecSaved\""),
        "Original startup did not execute autoexec after the saved config"
    );
    restored.close().unwrap();
}
