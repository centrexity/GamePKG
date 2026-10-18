//! Original bindings must observe one effective press/release across input sources.
use super::*;
fn fps(session: &mut Session) -> u32 {
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|f| f.path == "config.cfg").unwrap();
    std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("maxfps "))
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_bindings_combine_physical_and_controller_keys_without_early_release() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config=b"exec data/defaults.cfg\nmaxfps 100\nbind W [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\nbind LSHIFT [maxfps (+ $maxfps 1); onrelease [maxfps (+ $maxfps 10)]]\n";
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
    let mut actual = Vec::new();
    for (key, button, physical_mods) in [(26, 1, 0), (225, 1 << 5, 1)] {
        // Physical first; releasing it must not release the controller's matching action.
        session.key(key, true, false, physical_mods).unwrap();
        session.tick(button, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.key(key, false, false, 0).unwrap();
        session.tick(button, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.tick(0, 0, 0).unwrap();
        actual.push(fps(&mut session));
        // Controller first, with the opposite release order.
        session.tick(button, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.key(key, true, false, physical_mods).unwrap();
        session.tick(button, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.tick(0, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.key(key, false, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        actual.push(fps(&mut session));
        // Focus reset must release both sources once, with no later duplicate up.
        session.key(key, true, false, physical_mods).unwrap();
        session.tick(button, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.queue(protocol::RESET_INPUT.to_vec()).unwrap();
        session.tick(0, 0, 0).unwrap();
        actual.push(fps(&mut session));
        session.key(key, false, false, 0).unwrap();
        session.tick(0, 0, 0).unwrap();
        actual.push(fps(&mut session));
    }
    session.close().unwrap();
    assert_eq!(
        actual,
        [
            101, 101, 111, 112, 112, 112, 122, 123, 133, 133, 134, 134, 144, 145, 145, 145, 155,
            156, 166, 166
        ]
    );
}
