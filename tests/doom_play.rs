//! Requires a local player-owned IWAD; no game data is stored in the repository.
use gamepkg::{drawing::Command, runtime::Game};
fn load_result(root: &std::path::Path, wad: &std::path::Path) -> anyhow::Result<Game> {
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET")
        .ok()
        .map(|target| match target.as_str() {
            "native" => true,
            "wasm" => false,
            _ => panic!("test target must be native or wasm"),
        });
    Game::load_target(root, "com.gamepkg.doom", &[wad.to_path_buf()], native)
}
fn load(root: &std::path::Path, wad: &std::path::Path) -> Game {
    load_result(root, wad).unwrap()
}
fn copy(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "saves" || name == "progress" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(name));
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
fn run(game: &mut Game, frames: u32, input: u32) -> bool {
    let mut audible = false;
    for i in 0..frames {
        game.step(input)
            .unwrap_or_else(|e| panic!("frame {i}: {e:#}"));
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(game.audio_frame().len(), 1600);
        audible |= game.audio_frame().iter().any(|v| *v != 0);
    }
    audible
}
fn press(game: &mut Game, input: u32) {
    run(game, 1, input);
    run(game, 12, 0);
}
#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn real_wad_gameplay_audio_save_and_reload() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let root = fixture.path().to_path_buf();
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let before = gamepkg::verify::digest(&std::fs::read(&wad).unwrap());
    assert!(Game::load(&root, "com.gamepkg.doom").is_err());
    let mut game = load(&root, &wad);
    run(&mut game, 120, 0);
    for _ in 0..4 {
        press(&mut game, 64);
    }
    run(&mut game, 120, 0);
    let initial = game.builtin_state().unwrap().unwrap();
    assert_eq!(
        initial.0 & 0xffff,
        0,
        "did not start actual single-player gameplay: {initial:?}"
    );
    run(&mut game, 60, 4 << 8);
    let moved = game.builtin_state().unwrap().unwrap();
    assert_ne!(
        (initial.1, initial.2),
        (moved.1, moved.2),
        "held Up did not move the player"
    );
    assert_ne!(game.builtin_controller().unwrap().unwrap() & 4, 0);
    let firing = run(&mut game, 60, 16 << 8);
    assert!(firing, "firing produced no mixed WAD sound effects");
    run(&mut game, 1, (1 << 22) | (1 << 23) | 64);
    // Sample the checkpoint when it is committed, before later enemy damage
    // or movement can change the live player's position.
    for _ in 0..60 {
        run(&mut game, 1, 0);
        if root.join("saves").exists()
            && std::fs::read_dir(root.join("saves"))
                .unwrap()
                .next()
                .is_some()
        {
            break;
        }
    }
    let saved_position = game.builtin_state().unwrap().unwrap();
    let saves: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let bytes = std::fs::read(&saves[0]).unwrap();
    assert!(bytes.len() > 1000 && bytes.len() <= 512 * 1024);
    let frame = game.frame();
    let image = frame
        .commands
        .iter()
        .find_map(|c| {
            if let Command::Image { image, .. } = c {
                Some(image)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(image.pixels().len(), 320 * 200 * 4);
    let unique: std::collections::BTreeSet<_> = image.pixels().as_chunks::<4>().0.iter().collect();
    assert!(unique.len() > 64, "gameplay frame is blank");
    if let Some(path) = std::env::var_os("GAMEPKG_DOOM_PROOF") {
        image::save_buffer(path, image.pixels(), 320, 200, image::ColorType::Rgba8).unwrap();
    }
    run(&mut game, 60, 8 << 8);
    let away = game.builtin_state().unwrap().unwrap();
    assert_ne!((away.1, away.2), (saved_position.1, saved_position.2));
    drop(game);
    let mut resumed = load(&root, &wad);
    for _ in 0..30 {
        run(&mut resumed, 1, 0);
        if resumed.builtin_state().unwrap().unwrap().0 & 0xffff == 0 {
            break;
        }
    }
    let loaded = resumed.builtin_state().unwrap().unwrap();
    assert_eq!(loaded.0 & 0xffff, 0);
    // Saved momentum continues during load; compare within two map units,
    // far smaller than the deliberate movement away from the checkpoint.
    assert!(
        loaded.1.abs_diff(saved_position.1) <= 2 && loaded.2.abs_diff(saved_position.2) <= 2,
        "quicksave did not restore the checkpoint: {loaded:?} versus {saved_position:?}"
    );
    resumed.state = 0;
    run(&mut resumed, 10, 0);
    let restarted = resumed.builtin_state().unwrap().unwrap();
    assert_eq!(restarted.0 & 0xffff, 0);
    assert_ne!(
        (restarted.1, restarted.2),
        (loaded.1, loaded.2),
        "Restart did not reset the level"
    );
    run(&mut resumed, 1, 128);
    run(&mut resumed, 5, 0); // menu is usable after restore
    assert_eq!(
        gamepkg::verify::digest(&std::fs::read(&wad).unwrap()),
        before,
        "WAD was modified"
    );
}

#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn original_attract_demo_has_spatial_stereo() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let mut game = load(fixture.path(), &wad);
    let mut demo = false;
    let mut separated = false;
    for _ in 0..3600 {
        game.step(0).unwrap();
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(game.audio_frame().len(), 1600);
        demo |= game.builtin_state().unwrap().unwrap().0 & 256 != 0;
        separated |= game
            .audio_frame()
            .chunks_exact(2)
            .any(|pair| pair[0] != pair[1]);
        if demo && separated {
            break;
        }
    }
    assert!(demo, "original attract demo did not begin");
    assert!(separated, "original demo sounds lost spatial separation");
    game.finish().unwrap();
}

#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn original_title_music_plays_before_any_demo_sound_effects() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let mut game = load(fixture.path(), &wad);
    let mut audible = false;
    let mut title_frames = 0;
    for _ in 0..300 {
        game.step(0).unwrap();
        if game.builtin_state().unwrap().unwrap().0 & 256 != 0 {
            break;
        }
        title_frames += 1;
        audible |= game.audio_frame().iter().any(|sample| *sample != 0);
    }
    assert!(
        title_frames >= 30,
        "title phase did not last long enough to check music"
    );
    assert!(
        audible,
        "original title music is silent before the attract demo"
    );
    game.finish().unwrap();
}

fn bundle_parts(root: &std::path::Path) -> Vec<Vec<u8>> {
    let files: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let bytes = std::fs::read(&files[0]).unwrap();
    assert_eq!(&bytes[..8], b"\x89GPD2\r\n\x1a");
    let mut offset = 44;
    let mut parts = Vec::new();
    for index in 0..8 {
        let length =
            u32::from_le_bytes(bytes[12 + index * 4..16 + index * 4].try_into().unwrap()) as usize;
        parts.push(bytes[offset..offset + length].to_vec());
        offset += length;
    }
    assert_eq!(offset, bytes.len());
    parts
}
fn config_value(bytes: &[u8], name: &str) -> String {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .find_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some(name)).then(|| words.next().unwrap().to_owned())
        })
        .unwrap_or_else(|| panic!("missing config {name}"))
}
#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn all_six_original_save_slots_and_sound_settings_survive_relaunch() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&root, fixture.path());
    let root = fixture.path();
    let mut game = load(root, &wad);
    run(&mut game, 120, 0);
    for _ in 0..4 {
        press(&mut game, 64);
    }
    run(&mut game, 120, 0);
    assert_eq!(game.builtin_state().unwrap().unwrap().0 & 0xffff, 0);
    let initial_position = game.builtin_state().unwrap().unwrap();
    for slot in 0..6 {
        if slot == 5 {
            run(&mut game, 20, 4 << 8);
            run(&mut game, 60, 0); // Save a standing player after original movement friction settles.
        }
        press(&mut game, 128);
        if slot == 0 {
            for _ in 0..3 {
                press(&mut game, 8);
            }
        }
        press(&mut game, 64);
        if slot > 0 {
            press(&mut game, 8);
        }
        press(&mut game, 64);
        press(&mut game, 1 << 23);
        press(&mut game, 64);
        run(&mut game, 60, 0);
        let parts = bundle_parts(root);
        assert!(
            parts[slot].len() > 1000,
            "slot {slot} was not saved through the original menu"
        );
    }
    let last_position = game.builtin_state().unwrap().unwrap();
    assert_ne!(
        (initial_position.1, initial_position.2),
        (last_position.1, last_position.2)
    );
    let before = bundle_parts(root);
    let old_sfx = config_value(&before[6], "sfx_volume")
        .parse::<i32>()
        .unwrap();
    let old_music = config_value(&before[6], "music_volume")
        .parse::<i32>()
        .unwrap();
    press(&mut game, 128); // Main menu remembers Save Game (index 3).
    press(&mut game, 4);
    press(&mut game, 4);
    press(&mut game, 64); // Options.
    press(&mut game, 4);
    press(&mut game, 64); // Wrap from End Game to Sound Volume.
    press(&mut game, 1); // Reduce SFX volume.
    press(&mut game, 8);
    press(&mut game, 1); // Reduce music volume.
    let changed = bundle_parts(root);
    assert_eq!(
        config_value(&changed[6], "sfx_volume"),
        (old_sfx - 1).to_string()
    );
    assert_eq!(
        config_value(&changed[6], "music_volume"),
        (old_music - 1).to_string()
    );
    game.finish().unwrap();
    drop(game);
    let mut resumed = load(root, &wad);
    run(&mut resumed, 30, 0);
    assert_eq!(
        resumed.builtin_state().unwrap().unwrap().0 & 0xffff,
        0,
        "last saved slot was not resumed"
    );
    let restored = resumed.builtin_state().unwrap().unwrap();
    assert!(
        restored.1.abs_diff(last_position.1) <= 2 && restored.2.abs_diff(last_position.2) <= 2,
        "last saved slot position {restored:?} differs from {last_position:?}; initial {initial_position:?}"
    );
    assert_eq!(
        bundle_parts(root),
        changed,
        "slots or settings changed during relaunch"
    );
    resumed.finish().unwrap();
    drop(resumed);
    // Earlier GamePKG versions stored only the raw slot-0 .dsg bytes.
    let blob = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(&blob, &changed[0]).unwrap();
    let mut legacy = load(root, &wad);
    run(&mut legacy, 30, 0);
    assert_eq!(legacy.builtin_state().unwrap().unwrap().0 & 0xffff, 0);
    let migrated = bundle_parts(root);
    assert_eq!(
        migrated[0], changed[0],
        "legacy raw slot was changed during migration"
    );
    assert!(migrated[1..6].iter().all(Vec::is_empty));
    legacy.finish().unwrap();
    drop(legacy);
    // Valid original saves tolerate trailing padding. Exercise the full six-file
    // budget through the real worker/read/commit path, beyond the ordinary 1 MiB cap.
    let mut padded = b"\x89GPD2\r\n\x1a".to_vec();
    padded.extend(5u32.to_le_bytes());
    for _ in 0..6 {
        padded.extend((512u32 * 1024).to_le_bytes());
    }
    padded.extend((changed[6].len() as u32).to_le_bytes());
    padded.extend((changed[7].len() as u32).to_le_bytes());
    for _ in 0..6 {
        padded.extend(&changed[0]);
        padded.resize(padded.len() + 512 * 1024 - changed[0].len(), 0);
    }
    padded.extend(&changed[6]);
    padded.extend(&changed[7]);
    assert!(padded.len() > 3 * 1024 * 1024);
    std::fs::write(&blob, padded).unwrap();
    let mut large = load(root, &wad);
    run(&mut large, 30, 0);
    assert_eq!(large.builtin_state().unwrap().unwrap().0 & 0xffff, 0);
    press(&mut large, 128);
    press(&mut large, 8);
    press(&mut large, 64);
    press(&mut large, 4);
    press(&mut large, 64);
    press(&mut large, 1);
    let persisted = bundle_parts(root);
    assert!(persisted[..6]
        .iter()
        .all(|slot| slot.len() == 512 * 1024 && slot.starts_with(&changed[0])));
    assert_eq!(
        config_value(&persisted[6], "sfx_volume"),
        (old_sfx - 2).to_string()
    );
    large.finish().unwrap();
    drop(large);
    let mut corrupt = std::fs::read(&blob).unwrap();
    corrupt[8..12].copy_from_slice(&6u32.to_le_bytes());
    std::fs::write(&blob, &corrupt).unwrap();
    assert!(
        load_result(root, &wad).is_err(),
        "corrupt saved state was silently accepted"
    );
    assert_eq!(
        std::fs::read(blob).unwrap(),
        corrupt,
        "failed load changed saved state"
    );
}
fn desktop_key(game: &mut Game, code: u8) {
    game.doom_input(gamepkg::doom_input::Event::Key { code, down: true })
        .unwrap();
    run(game, 6, 0);
    game.doom_input(gamepkg::doom_input::Event::Key { code, down: false })
        .unwrap();
    run(game, 12, 0);
}
fn replace_config(bytes: &[u8], name: &str, value: &str) -> Vec<u8> {
    let mut found = false;
    let output = std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| {
            if line.split_whitespace().next() == Some(name) {
                found = true;
                format!("{name:<30}{value}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect::<String>();
    assert!(found, "missing config {name}");
    output.into_bytes()
}
fn write_bundle(root: &std::path::Path, parts: &[Vec<u8>], selected: u32) {
    let mut bytes = b"\x89GPD2\r\n\x1a".to_vec();
    bytes.extend(selected.to_le_bytes());
    for part in parts {
        bytes.extend((part.len() as u32).to_le_bytes());
    }
    for part in parts {
        bytes.extend(part);
    }
    let path = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::write(path, bytes).unwrap();
}
#[test]
#[ignore = "requires GAMEPKG_DOOM_TEST_ROOT and GAMEPKG_DOOM_TEST_WAD"]
fn original_desktop_keyboard_mouse_binding_migration_and_quit() {
    use gamepkg::doom_input::Event;
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let fixture = tempfile::tempdir().unwrap();
    copy(&source, fixture.path());
    let root = fixture.path();
    let mut game = load(root, &wad);
    run(&mut game, 120, 0);
    for _ in 0..4 {
        desktop_key(&mut game, 13);
    }
    run(&mut game, 120, 0);
    assert_eq!(game.builtin_state().unwrap().unwrap().0 & 0xffff, 0);
    println!("desktop phase: save name");
    desktop_key(&mut game, 0xbc);
    desktop_key(&mut game, 13); // Original F2 save menu.
    for code in b"rust nami" {
        desktop_key(&mut game, *code);
    }
    desktop_key(&mut game, 127);
    desktop_key(&mut game, b'e');
    desktop_key(&mut game, 13);
    run(&mut game, 60, 0);
    let mut parts = bundle_parts(root);
    assert!(
        parts[0].starts_with(b"RUST NAME\0"),
        "original save-name typing/backspace failed"
    );
    for (name, value) in [
        ("key_fire", "29"),
        ("key_use", "57"),
        ("key_strafeleft", "51"),
        ("key_straferight", "52"),
    ] {
        assert_eq!(config_value(&parts[6], name), value);
        parts[6] = replace_config(&parts[6], name, "0");
    }
    parts[7] = replace_config(&parts[7], "mouseb_nextweapon", "3");
    parts[7] = replace_config(&parts[7], "mouseb_prevweapon", "4");
    game.finish().unwrap();
    drop(game);
    write_bundle(root, &parts, 0);
    let mut game = load(root, &wad);
    run(&mut game, 120, 0);
    println!("desktop phase: reload and fire");
    let repaired = bundle_parts(root);
    assert_eq!(config_value(&repaired[6], "key_fire"), "29");
    game.doom_input(Event::Key {
        code: 0x9d,
        down: true,
    })
    .unwrap();
    run(&mut game, 30, 0);
    assert_ne!(
        game.builtin_controller().unwrap().unwrap() & (1 << 24),
        0,
        "Ctrl did not fire after legacy key migration/reload"
    );
    game.doom_release_input().unwrap();
    run(&mut game, 90, 0);
    assert_eq!(
        game.builtin_controller().unwrap().unwrap() & (1 << 24),
        0,
        "focus release left fire held"
    );
    println!("desktop phase: pause");
    desktop_key(&mut game, 255);
    assert_ne!(
        game.builtin_controller().unwrap().unwrap() & (1 << 26),
        0,
        "Pause key did not pause"
    );
    desktop_key(&mut game, 255);
    assert_eq!(
        game.builtin_controller().unwrap().unwrap() & (1 << 26),
        0,
        "Pause key did not resume"
    );
    println!("desktop phase: weapons");
    desktop_key(&mut game, b'1');
    run(&mut game, 120, 0);
    assert_eq!(
        (game.builtin_controller().unwrap().unwrap() >> 27) & 15,
        0,
        "weapon 1 was not selected"
    );
    game.doom_input(Event::Mouse {
        buttons: 8,
        dx: 0,
        dy: 0,
    })
    .unwrap();
    game.doom_input(Event::Mouse {
        buttons: 0,
        dx: 0,
        dy: 0,
    })
    .unwrap();
    run(&mut game, 120, 0);
    assert_eq!(
        (game.builtin_controller().unwrap().unwrap() >> 27) & 15,
        1,
        "configured mouse wheel did not select the next weapon"
    );
    game.doom_input(Event::Mouse {
        buttons: 1,
        dx: 0,
        dy: 0,
    })
    .unwrap();
    run(&mut game, 30, 0);
    assert_ne!(
        game.builtin_controller().unwrap().unwrap() & (1 << 24),
        0,
        "left mouse did not fire"
    );
    game.doom_input(Event::Mouse {
        buttons: 0,
        dx: 0,
        dy: 0,
    })
    .unwrap();
    run(&mut game, 90, 0);
    println!("desktop phase: mouse look");
    let before = game.builtin_state().unwrap().unwrap();
    game.doom_input(Event::Mouse {
        buttons: 0,
        dx: 300,
        dy: 0,
    })
    .unwrap();
    run(&mut game, 6, 0);
    game.doom_input(Event::Key {
        code: 0xad,
        down: true,
    })
    .unwrap();
    run(&mut game, 30, 0);
    game.doom_release_input().unwrap();
    run(&mut game, 60, 0);
    let after = game.builtin_state().unwrap().unwrap();
    assert_ne!(
        before.1, after.1,
        "relative mouse turning did not change forward movement direction"
    );
    println!("desktop phase: quit");
    desktop_key(&mut game, 0xc4);
    desktop_key(&mut game, b'y'); // Original F10 + confirmation.
    run(&mut game, 120, 0);
    assert!(!game.exited(), "original ENDOOM was skipped");
    assert_ne!(
        game.builtin_controller().unwrap().unwrap() & (1 << 25),
        0,
        "original ENDOOM screen was not displayed"
    );
    desktop_key(&mut game, 13);
    assert!(
        game.exited(),
        "original quit menu did not close the game cleanly"
    );
    game.finish().unwrap();
}
