//! Original console navigation must use modifiers at each event, not batch end.
use super::*;
fn tap(session: &mut Session, key: u32, mods: u32) {
    session.key(key, true, false, mods).unwrap();
    session.key(key, false, false, mods).unwrap();
}
fn name(session: &mut Session) -> String {
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|f| f.path == "config.cfg").unwrap();
    std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("name \"")
                .and_then(|s| s.strip_suffix('"'))
        })
        .expect("original saved player name")
        .to_owned()
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_console_uses_event_time_modifiers_within_one_input_batch() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config = b"exec data/defaults.cfg\ntextinputfilter 0\nbind F6 [saycommand /name abcdef]\n";
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
    for later_press in [true, false] {
        tap(&mut session, 63, 0); // Original F6 opens a known editable command.
        session.tick(0, 0, 0).unwrap();
        if later_press {
            tap(&mut session, 80, 0); // Plain Left before a later Ctrl press.
            session.key(228, true, false, 128).unwrap();
            session.modifiers(128).unwrap();
        } else {
            session.key(228, true, false, 128).unwrap();
            tap(&mut session, 80, 128); // Ctrl+Left before a later Ctrl release.
            session.key(228, false, false, 0).unwrap();
            session.modifiers(0).unwrap();
        }
        session.tick(0, 0, 0).unwrap();
        session.key(228, false, false, 0).unwrap();
        session
            .queue(protocol::text_event(if later_press { "X" } else { "Y" }).unwrap())
            .unwrap();
        tap(&mut session, 40, 0); // Execute the edited original console command.
        session.tick(0, 0, 0).unwrap();
        actual.push(name(&mut session));
    }
    session.close().unwrap();
    assert_eq!(actual, ["abcdeXf", "Yabcdef"]);
}
