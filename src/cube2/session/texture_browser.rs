//! The original F2 texture browser is built in despite an upstream stale include.
use super::*;

fn tap(session: &mut Session, key: u32) {
    session.key(key, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(key, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
}
fn assert_browser(session: &mut Session, open: bool) {
    tap(session, 66); // F9 evaluates the original cleartexgui return value.
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files
        .iter()
        .find(|entry| entry.path == "config.cfg")
        .unwrap();
    let expected = format!("name \"TextureOpen{}\"", u8::from(open));
    assert!(
        String::from_utf8_lossy(config.bytes).contains(&expected),
        "Original texture-browser state did not match {expected}"
    );
}
fn saved_name(session: &mut Session) -> String {
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files
        .iter()
        .find(|entry| entry.path == "config.cfg")
        .unwrap();
    std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("name \"")
                .and_then(|s| s.strip_suffix('"'))
        })
        .unwrap()
        .to_owned()
}
fn click(session: &mut Session) {
    session
        .queue(protocol::mouse_button_event(1, true, 0).unwrap())
        .unwrap();
    session.tick(0, 0, 0).unwrap();
    session
        .queue(protocol::mouse_button_event(1, false, 0).unwrap())
        .unwrap();
    session.tick(0, 0, 0).unwrap();
}

#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_texture_thumbnail_changes_geometry_and_survives_restart() {
    verify_texture_history(true);
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_face_only_thumbnail_undo_redo_survives_restart() {
    verify_texture_history(false);
}
fn verify_texture_history(all_faces: bool) {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let binary = super::super::build::compile().unwrap();
    let config=b"exec data/defaults.cfg\nallfaces 1\nguisens 1\neditinview 0\neditbind F8 [setselpos 512 512 504; selsave; name (strreplace (getselsavedsize) \" \" \"_\")]\neditbind F9 [name (format \"H%1_T%2\" $texguinum (getslottex $texguinum))]\neditbind F10 [name (strreplace (getselpos) \" \" \"_\")]\neditbind F11 [cleartexgui; maptitle TexturePersist; savemap TexturePersist]\neditbind F12 [name (concatword T (getseltex))]\n";
    let config = std::str::from_utf8(config).unwrap().replace(
        "allfaces 1",
        if all_faces {
            "allfaces 1"
        } else {
            "allfaces 0"
        },
    );
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
        "; mode 1; newmap 10; if (= $editing 0) [edittoggle]; showgui \"\"".into(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..20 {
        session.tick(0, 0, 0).unwrap();
    }
    session.tick(0, 0, 1000).unwrap();
    click(&mut session); // Create a real nonempty selection via original edit-drag.
    tap(&mut session, 65); // Move that existing selection onto the known floor cube.
    assert_eq!(
        saved_name(&mut session),
        "1_1_1",
        "Fixture did not select exactly one original cube"
    );
    tap(&mut session, 67);
    let initial = saved_name(&mut session);
    let coordinates: Vec<_> = initial
        .split('_')
        .map(|value| value.parse::<i32>().expect("original selected coordinate"))
        .collect();
    assert_eq!(coordinates, [512, 512, 504]);
    let position = coordinates
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    tap(&mut session, 69);
    let old = saved_name(&mut session)
        .strip_prefix('T')
        .unwrap()
        .parse::<i32>()
        .unwrap();
    tap(&mut session, 59);
    for _ in 0..20 {
        session.tick(0, 0, 0).unwrap();
    }
    // Locate a real thumbnail through the engine's hover result rather than
    // assuming the centre of every browser layout lands inside a cell.
    let frame = session.tick(0, 0, 0).unwrap();
    let aspect = frame.width as f64 / frame.height as f64;
    session.tick(0, -2048, -2048).unwrap();
    let mut previous = (0.0, 0.0);
    let mut selected = None;
    'rows: for y in [0.25, 0.35, 0.45, 0.55, 0.65, 0.75] {
        for x in [0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8] {
            session
                .tick(
                    0,
                    ((x - previous.0) * 500.0 * aspect).round() as i32,
                    ((y - previous.1) * 500.0_f64).round() as i32,
                )
                .unwrap();
            previous = (x, y);
            session.tick(0, 0, 0).unwrap();
            tap(&mut session, 66);
            let hover = saved_name(&mut session);
            let fields: Vec<_> = hover.split('_').collect();
            let slot = fields[0].strip_prefix('H').unwrap().parse::<i32>().unwrap();
            let chosen = fields[1].strip_prefix('T').unwrap().parse::<i32>().unwrap();
            if slot >= 2 && chosen >= 0 && chosen != old {
                selected = Some(chosen);
                break 'rows;
            }
        }
    }
    let chosen =
        selected.expect("No distinct original texture thumbnail under the tested browser grid");
    click(&mut session);
    tap(&mut session, 69);
    assert_eq!(
        saved_name(&mut session),
        format!("T{chosen}"),
        "Thumbnail selection did not change actual geometry"
    );
    tap(&mut session, 67);
    assert_eq!(
        saved_name(&mut session),
        initial,
        "Thumbnail click changed the map selection"
    );
    // Exercise the unchanged default U/I bindings, not direct replacement
    // commands or a host-owned history. Each side must alter real cube data.
    for _ in 0..3 {
        tap(&mut session, 24); // U: original undo.
        tap(&mut session, 69);
        assert_eq!(
            saved_name(&mut session),
            format!("T{old}"),
            "Undo did not restore the original cube texture"
        );
        tap(&mut session, 12); // I: original redo.
        tap(&mut session, 69);
        assert_eq!(
            saved_name(&mut session),
            format!("T{chosen}"),
            "Redo did not restore the chosen thumbnail texture"
        );
        tap(&mut session, 67);
        assert_eq!(
            saved_name(&mut session),
            initial,
            "Undo/redo changed the selected cube position"
        );
    }
    tap(&mut session, 68);
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let map = files
        .iter()
        .find(|entry| entry.path == "packages/base/TexturePersist.ogz")
        .expect("Edited map was not persisted")
        .bytes
        .to_vec();
    drop(session);
    let mut restored=Session::start(Path::new(&data),2*1024*1048576,Duration::from_secs(60),&saved,||Ok(binary),None,
        format!("; mode 1; map TexturePersist; if (= $editing 0) [edittoggle]; setselpos {position}; showgui \"\"")).unwrap();
    restored.tick(0, 0, 0).unwrap();
    restored.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..10 {
        restored.tick(0, 0, 0).unwrap();
    }
    tap(&mut restored, 69);
    // A fresh selection's original orientation is face zero; a floor edit
    // changes face five. Face-only painting must leave that side unchanged.
    assert_eq!(
        saved_name(&mut restored),
        format!("T{}", if all_faces { chosen } else { old }),
        "Restored unselected face has the wrong texture"
    );
    restored.tick(0, 0, 1000).unwrap();
    click(&mut restored);
    tap(&mut restored, 65);
    assert_eq!(saved_name(&mut restored), "1_1_1");
    tap(&mut restored, 69);
    assert_eq!(
        saved_name(&mut restored),
        format!("T{chosen}"),
        "Original reloaded floor face lost its redone texture"
    );
    tap(&mut restored, 67);
    assert_eq!(saved_name(&mut restored), initial);
    let saved = restored.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|entry| entry.path == "packages/base/TexturePersist.ogz")
            .unwrap()
            .bytes,
        map
    );
    restored.close().unwrap();
}
#[test]
#[ignore = "requires original Cube 2 data, compiler/runtime namespaces and Tesseract"]
fn original_f2_texture_browser_opens_renders_and_toggles_without_missing_script() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let config =
        b"exec data/defaults.cfg\neditbind F9 [name (concatword TextureOpen (cleartexgui))]\n";
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
        "; mode 1; map complex; if (= $editing 0) [edittoggle]; showgui \"\"".into(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..20 {
        session.tick(0, 0, 0).unwrap();
    }
    assert_browser(&mut session, false);
    for _ in 0..3 {
        tap(&mut session, 59); // The unchanged original default F2 edit binding.
        for _ in 0..10 {
            let frame = session.tick(0, 0, 0).unwrap();
            assert!(frame.width > 0 && frame.height > 0);
            assert_eq!(
                frame.rgba.len(),
                frame.width as usize * frame.height as usize * 4
            );
        }
        let frame = session.tick(0, 0, 0).unwrap();
        let image = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba).unwrap();
        let image = image::imageops::resize(
            &image,
            image.width() * 3,
            image.height() * 3,
            image::imageops::FilterType::Nearest,
        );
        let proof = tempfile::tempdir().unwrap();
        let path = proof.path().join("textures.png");
        image.save(&path).unwrap();
        let text = std::process::Command::new("tesseract")
            .arg(&path)
            .args(["stdout", "--psm", "11"])
            .output()
            .unwrap();
        assert!(text.status.success());
        assert!(
            String::from_utf8_lossy(&text.stdout)
                .to_ascii_lowercase()
                .contains("textures"),
            "Original texture browser label was not rendered: {}",
            String::from_utf8_lossy(&text.stdout)
        );
        assert_browser(&mut session, true);
        assert_browser(&mut session, false);
    }
    tap(&mut session, 59);
    tap(&mut session, 59);
    assert_browser(&mut session, false);
    session.close().unwrap();
}
