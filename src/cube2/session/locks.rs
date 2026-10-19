//! Actual engine must accept lock bits on every supported input packet.
use super::*;
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_engine_accepts_caps_and_num_lock_input_and_saves_normally() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config = b"exec data/defaults.cfg\nbind F6 [name LockInputOK]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
        super::super::build::compile,
        None,
        "; mode 0; map complex; showgui \"\"".to_owned(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for bits in [0x1000, 0x2000, 0x3000, 0] {
        session.modifiers(bits).unwrap();
        session.key(63, true, false, bits).unwrap(); // Original F6 binding.
        session
            .queue(protocol::mouse_button_event(2, true, bits).unwrap())
            .unwrap();
        session
            .queue(protocol::wheel_event(0, 1, bits).unwrap())
            .unwrap();
        session.tick(0, 0, 0).unwrap();
        session.key(63, false, false, bits).unwrap();
        session
            .queue(protocol::mouse_button_event(2, false, bits).unwrap())
            .unwrap();
        session.tick(0, 0, 0).unwrap();
    }
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|f| f.path == "config.cfg").unwrap();
    let text = std::str::from_utf8(config.bytes).unwrap();
    assert!(text.contains("name \"LockInputOK\""), "{text}");
    session.close().unwrap();
}
