//! Uses a private signed seven-ROM recipe; synthetic patch graphics, player IWAD.
use gamepkg::{drawing::Command, runtime::Game};
fn inputs(
    wad: &std::path::Path,
    first: &std::path::Path,
    last: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let mut files = vec![wad.to_path_buf()];
    files.extend(std::iter::repeat_n(first.to_path_buf(), 5));
    files.push(last.to_path_buf());
    files
}
fn copy_private_library(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "saves" || entry.file_name() == "progress" {
            continue;
        }
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            copy_private_library(&entry.path(), &to.join(entry.file_name()));
        } else {
            assert!(kind.is_file());
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
fn original_slot_zero(root: &std::path::Path) -> Vec<u8> {
    let archive = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .find_map(|entry| {
            let data = std::fs::read(entry.unwrap().path()).ok()?;
            (data.len() >= 44 && data.starts_with(b"\x89GPD2\r\n\x1a")).then_some(data)
        })
        .expect("Original save bundle missing");
    let length = u32::from_le_bytes(archive[12..16].try_into().unwrap()) as usize;
    assert!(length > 0 && length <= 512 * 1024 && length <= archive.len() - 44);
    let slot = archive[44..44 + length].to_vec();
    assert!(
        slot.starts_with(b"GAMEPKG") && slot.last() == Some(&0x1d),
        "Original save description/EOF missing"
    );
    slot
}
fn rendered_image(game: &Game) -> Vec<u8> {
    game.frame()
        .commands
        .iter()
        .find_map(|command| match command {
            Command::Image { image, .. } => Some(image.pixels().to_vec()),
            _ => None,
        })
        .expect("original engine did not render")
        .to_owned()
}
#[test]
#[ignore = "requires private GAMEPKG_DOOM_MOD_ROOT, WAD and patch fixtures"]
fn original_patch_wad_order_title_and_gameplay() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let a = root.join("a.wad");
    let b = root.join("b.wad");
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let file_first = std::env::var("GAMEPKG_DOOM_TEST_MODE").as_deref() == Ok("mixed_file_first");
    let mut results = Vec::new();
    for patches in [[&a, &a], [&b, &b], [&a, &b], [&b, &a]] {
        let mut game = Game::load_target(
            &root,
            "com.gamepkg.doom",
            &inputs(&wad, patches[0], patches[1]),
            Some(native),
        )
        .unwrap();
        for _ in 0..20 {
            game.step(0).unwrap();
        }
        results.push(rendered_image(&game));
    }
    assert!(
        results[0] != results[1],
        "Synthetic patch images were not loaded"
    );
    assert!(
        results[2] == results[usize::from(!file_first)],
        "Original loading groups did not select the expected A/B title"
    );
    assert!(
        results[3] == results[usize::from(file_first)],
        "Reversing resources did not preserve original group priority"
    );
    let map_a = root.join("map-a.wad");
    let map_b = root.join("map-b.wad");
    for (first, last, expected_x) in [(&map_a, &map_b, 32u16), (&map_b, &map_a, 0u16)] {
        let expected_x = if file_first {
            expected_x ^ 32
        } else {
            expected_x
        };
        let map_fixture = tempfile::tempdir().unwrap();
        copy_private_library(&root, map_fixture.path());
        let map_root = map_fixture.path();
        let mut room = Game::load_target(
            map_root,
            "com.gamepkg.doom",
            &inputs(&wad, first, last),
            Some(native),
        )
        .unwrap();
        for _ in 0..120 {
            room.step(0).unwrap();
        }
        for _ in 0..4 {
            room.step(64).unwrap();
            for _ in 0..12 {
                room.step(0).unwrap();
            }
        }
        for _ in 0..120 {
            room.step(0).unwrap();
        }
        let state = room.builtin_state().unwrap().unwrap();
        assert_eq!(
            state.0 & 0xffff,
            0,
            "PWAD map did not enter original gameplay"
        );
        assert_eq!(
            state.1, expected_x,
            "Original loading groups did not supply the expected exact player start"
        );
        assert_eq!(state.2, 0, "PWAD map player start Y changed");
        let pixels = rendered_image(&room);
        assert!(
            pixels[..320 * 150 * 4]
                .as_chunks::<4>()
                .0
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 32,
            "Synthetic PWAD room rendered blank"
        );
        for _ in 0..60 {
            room.step(4 << 8).unwrap();
        }
        let moved = room.builtin_state().unwrap().unwrap();
        assert!(
            moved.1 > expected_x && moved.1 <= 112,
            "PWAD room movement/collision failed: {moved:?}"
        );
        for _ in 0..180 {
            room.step(4 << 8).unwrap();
        }
        let blocked = room.builtin_state().unwrap().unwrap();
        assert!(
            blocked.1 >= moved.1 && blocked.1 <= 112,
            "Player crossed the original solid wall: {blocked:?}"
        );
        assert_eq!(
            blocked.2, 0,
            "Straight movement drifted from the generated map corridor"
        );
        for _ in 0..60 {
            room.step(0).unwrap();
        }
        room.doom_input(gamepkg::doom_input::Event::Key {
            code: 255,
            down: true,
        })
        .unwrap();
        room.step(0).unwrap();
        room.doom_input(gamepkg::doom_input::Event::Key {
            code: 255,
            down: false,
        })
        .unwrap();
        for _ in 0..12 {
            room.step(0).unwrap();
        }
        assert_ne!(
            room.builtin_controller().unwrap().unwrap() & (1 << 26),
            0,
            "Original Pause did not freeze the save witness"
        );
        room.step((1 << 18) | (1 << 19) | 64).unwrap();
        for _ in 0..30 {
            room.step(0).unwrap();
        }
        let saved_position = room.builtin_state().unwrap().unwrap();
        let saved_slot = original_slot_zero(map_root);
        drop(room);
        let mut resumed = Game::load_target(
            map_root,
            "com.gamepkg.doom",
            &inputs(&wad, first, last),
            Some(native),
        )
        .unwrap();
        for _ in 0..30 {
            resumed.step(0).unwrap();
        }
        let restored = resumed.builtin_state().unwrap().unwrap();
        assert_eq!(
            resumed.builtin_controller().unwrap().unwrap() & (1 << 26),
            0,
            "Fresh loaded game remained paused"
        );
        assert_eq!(
            restored.0 & 0xffff,
            0,
            "Generated PWAD save did not resume gameplay"
        );
        assert_eq!(
            (restored.1, restored.2),
            (saved_position.1, saved_position.2),
            "Original PWAD save lost the moved player position"
        );
        assert_eq!(
            original_slot_zero(map_root),
            saved_slot,
            "Loading modified the original saved slot"
        );
        for _ in 0..120 {
            resumed.step(4 << 8).unwrap();
        }
        let blocked = resumed.builtin_state().unwrap().unwrap();
        assert!(
            blocked.1 <= 112 && blocked.2 == 0,
            "Restored PWAD map lost solid-wall collision"
        );
    }
    let mut game = Game::load_target(
        &root,
        "com.gamepkg.doom",
        &inputs(&wad, &a, &b),
        Some(native),
    )
    .unwrap();
    for _ in 0..120 {
        game.step(0).unwrap();
    }
    for _ in 0..4 {
        game.step(64).unwrap();
        for _ in 0..12 {
            game.step(0).unwrap();
        }
    }
    for _ in 0..120 {
        game.step(0).unwrap();
    }
    let before = game.builtin_state().unwrap().unwrap();
    assert_eq!(
        before.0 & 0xffff,
        0,
        "did not enter original gameplay: {before:?}"
    );
    for _ in 0..60 {
        game.step(4 << 8).unwrap();
    }
    let after = game.builtin_state().unwrap().unwrap();
    assert_ne!(
        (before.1, before.2),
        (after.1, after.2),
        "movement failed after PWAD loading"
    );
    game.step((1 << 18) | (1 << 19) | 64).unwrap();
    for _ in 0..60 {
        game.step(0).unwrap();
    }
    let saves = root.join("saves");
    assert!(
        std::fs::read_dir(&saves).unwrap().any(|entry| {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            bytes.len() >= 44
                && bytes.starts_with(b"\x89GPD2\r\n\x1a")
                && u32::from_le_bytes(bytes[12..16].try_into().unwrap()) > 1000
        }),
        "Original slot zero was not saved with six persistent PWAD handles"
    );
    drop(game);
    let mut resumed = Game::load_target(
        &root,
        "com.gamepkg.doom",
        &inputs(&wad, &a, &b),
        Some(native),
    )
    .unwrap();
    for _ in 0..30 {
        resumed.step(0).unwrap();
    }
    assert_eq!(
        resumed.builtin_state().unwrap().unwrap().0 & 0xffff,
        0,
        "Patched original save did not resume gameplay"
    );
    drop(resumed);

    let invalid = root.join("invalid.wad");
    std::fs::write(&invalid, b"PWADinvalid").unwrap();
    assert!(Game::load_target(
        &root,
        "com.gamepkg.doom",
        &inputs(&wad, &a, &invalid),
        Some(native)
    )
    .is_err());
}

#[test]
#[ignore = "requires private signed flat-mode fixtures and player IWAD"]
fn original_flat_merge_modes_include_or_exclude_floor_pixels() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let mode = std::env::var("GAMEPKG_DOOM_TEST_MODE").unwrap();
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let map = root.join("map-a.wad");
    let mut frames = Vec::new();
    for flat in ["flat-a.wad", "flat-a.wad", "flat-b.wad"] {
        let mut game = Game::load_target(
            &root,
            "com.gamepkg.doom",
            &inputs(&wad, &map, &root.join(flat)),
            Some(native),
        )
        .unwrap();
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        for _ in 0..4 {
            game.step(64).unwrap();
            for _ in 0..12 {
                game.step(0).unwrap();
            }
        }
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        let state = game.builtin_state().unwrap().unwrap();
        assert_eq!((state.0 & 0xffff, state.1, state.2), (0, 0, 0));
        let image = rendered_image(&game);
        let mut floor = Vec::new();
        for row in 110..150 {
            floor.extend_from_slice(&image[(row * 320 + 20) * 4..(row * 320 + 100) * 4]);
        }
        frames.push(floor);
    }
    assert!(
        frames[0] == frames[1],
        "Flat rendering changed between identical fresh engines"
    );
    match mode.as_str() {
        "merge" | "flats" | "both" => assert!(
            frames[0] != frames[2],
            "Original flat merge did not affect visible floor pixels"
        ),
        "sprites" | "nwt_merge" => assert!(
            frames[0] == frames[2],
            "Sprite-only original mode unexpectedly merged flat content"
        ),
        _ => panic!("unsupported flat witness mode"),
    }
}

#[test]
#[ignore = "requires private signed sprite-mode fixtures and player IWAD"]
fn original_sprite_merge_modes_include_or_exclude_weapon_pixels() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let mode = std::env::var("GAMEPKG_DOOM_TEST_MODE").unwrap();
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let map = root.join("map-a.wad");
    let mut frames = Vec::new();
    for sprite in ["sprite-a.wad", "sprite-a.wad", "sprite-b.wad"] {
        let mut game = Game::load_target(
            &root,
            "com.gamepkg.doom",
            &inputs(&wad, &map, &root.join(sprite)),
            Some(native),
        )
        .unwrap();
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        for _ in 0..4 {
            game.step(64).unwrap();
            for _ in 0..12 {
                game.step(0).unwrap();
            }
        }
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        let state = game.builtin_state().unwrap().unwrap();
        assert_eq!((state.0 & 0xffff, state.1, state.2), (0, 0, 0));
        let image = rendered_image(&game);
        let mut weapon = Vec::new();
        for row in 130..160 {
            weapon.extend_from_slice(&image[(row * 320 + 130) * 4..(row * 320 + 180) * 4]);
        }
        frames.push(weapon);
    }
    assert!(
        frames[0] == frames[1],
        "Sprite rendering changed between identical fresh engines"
    );
    match mode.as_str() {
        "merge" | "sprites" | "both" => assert!(
            frames[0] != frames[2],
            "Original sprite merge did not affect visible weapon pixels"
        ),
        "flats" => assert!(
            frames[0] == frames[2],
            "Flat-only original mode unexpectedly merged sprite content"
        ),
        _ => panic!("unsupported sprite witness mode"),
    }
}

#[test]
#[ignore = "requires private signed NWT mask/reload fixtures and player IWAD"]
fn original_nwt_mask_and_file_reload_render_weapon() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let map = root.join("map-a.wad");
    let mut frames = Vec::new();
    for name in ["nwt-sprite-a.wad", "nwt-sprite-a.wad", "nwt-sprite-b.wad"] {
        let sprite = root.join(name);
        let mut selected = vec![wad.clone()];
        selected.extend(std::iter::repeat_n(map.clone(), 4));
        selected.extend([sprite.clone(), sprite]);
        let mut game =
            Game::load_target(&root, "com.gamepkg.doom", &selected, Some(native)).unwrap();
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        for _ in 0..4 {
            game.step(64).unwrap();
            for _ in 0..12 {
                game.step(0).unwrap();
            }
        }
        for _ in 0..120 {
            game.step(0).unwrap();
        }
        let state = game.builtin_state().unwrap().unwrap();
        assert_eq!((state.0 & 0xffff, state.1, state.2), (0, 0, 0));
        let image = rendered_image(&game);
        let mut weapon = Vec::new();
        for row in 130..160 {
            weapon.extend_from_slice(&image[(row * 320 + 130) * 4..(row * 320 + 180) * 4]);
        }
        let mut animated = false;
        for _ in 0..60 {
            game.step(16 << 8).unwrap();
            let frame = rendered_image(&game);
            let mut firing = Vec::new();
            for row in 130..160 {
                firing.extend_from_slice(&frame[(row * 320 + 130) * 4..(row * 320 + 180) * 4]);
            }
            animated |= firing != weapon;
        }
        assert!(animated, "NWT reload lost original pistol firing animation");
        assert_ne!(
            game.builtin_controller().unwrap().unwrap() & (1 << 24),
            0,
            "Original fire control did not reach the NWT game"
        );
        for _ in 0..90 {
            game.step(0).unwrap();
        }
        assert_eq!(game.builtin_controller().unwrap().unwrap() & (1 << 24), 0);
        let frame = rendered_image(&game);
        let mut idle = Vec::new();
        for row in 130..160 {
            idle.extend_from_slice(&frame[(row * 320 + 130) * 4..(row * 320 + 180) * 4]);
        }
        assert!(
            idle == weapon,
            "Pistol did not return to exact replacement idle pixels after original firing"
        );
        frames.push(weapon);
    }
    assert!(
        frames[0] == frames[1],
        "NWT mask/reload changed between identical fresh engines"
    );
    assert!(
        frames[0] != frames[2],
        "NWT mask followed by file did not display replacement sprite"
    );
}

#[test]
#[ignore = "requires private signed new-flat fixtures and player IWAD"]
fn original_new_flat_insertion_requires_merge() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let mode = std::env::var("GAMEPKG_DOOM_TEST_MODE").unwrap();
    assert!(["merge", "flats", "sprites", "both", "nwt_merge"].contains(&mode.as_str()));
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let map = root.join("map-newflat.wad");
    let mut frames = Vec::new();
    for name in ["newflat-a.wad", "newflat-a.wad", "newflat-b.wad"] {
        let mut game = Game::load_target(
            &root,
            "com.gamepkg.doom",
            &inputs(&wad, &map, &root.join(name)),
            Some(native),
        )
        .unwrap();
        let loaded = (|| -> anyhow::Result<()> {
            for _ in 0..120 {
                game.step(0)?;
            }
            for _ in 0..4 {
                game.step(64)?;
                for _ in 0..12 {
                    game.step(0)?;
                }
            }
            for _ in 0..120 {
                game.step(0)?;
            }
            Ok(())
        })();
        if mode != "merge" {
            let error = loaded.expect_err(
                "Replacement-only mode inserted a missing flat or silently substituted a map",
            );
            let message = format!("{error:#}");
            assert!(
                message.contains("PKGFLAT"),
                "Missing-flat rejection did not identify its resource: {message}"
            );
            continue;
        }
        loaded.unwrap();
        let state = game.builtin_state().unwrap().unwrap();
        assert_eq!((state.0 & 0xffff, state.1, state.2), (0, 0, 0));
        let pixels = rendered_image(&game);
        let mut floor = Vec::new();
        for row in 110..150 {
            floor.extend_from_slice(&pixels[(row * 320 + 20) * 4..(row * 320 + 100) * 4]);
        }
        frames.push(floor);
        for _ in 0..60 {
            game.step(4 << 8).unwrap();
        }
        let moved = game.builtin_state().unwrap().unwrap();
        assert!(moved.1 > 0 && moved.1 <= 112);
    }
    if mode == "merge" {
        assert!(
            frames[0] == frames[1],
            "Inserted flat rendering is not repeatable"
        );
        assert!(
            frames[0] != frames[2],
            "Inserted flat content was not visible"
        );
    }
}

#[test]
#[ignore = "requires private signed missing-resource recovery fixtures and player IWAD"]
fn original_missing_flat_preserves_saved_game_and_recovers() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_MOD_ROOT").unwrap());
    let wad = std::path::PathBuf::from(std::env::var_os("GAMEPKG_DOOM_TEST_WAD").unwrap());
    let native = std::env::var("GAMEPKG_DOOM_TEST_TARGET").unwrap() == "native";
    let patch = root.join("newflat-a.wad");
    let good_map = root.join("map-a.wad");
    let selected = inputs(&wad, &good_map, &patch);
    let mut game = Game::load_target(&root, "com.gamepkg.doom", &selected, Some(native)).unwrap();
    for _ in 0..120 {
        game.step(0).unwrap();
    }
    for _ in 0..4 {
        game.step(64).unwrap();
        for _ in 0..12 {
            game.step(0).unwrap();
        }
    }
    for _ in 0..120 {
        game.step(0).unwrap();
    }
    assert_eq!(game.builtin_state().unwrap().unwrap().0 & 0xffff, 0);
    for _ in 0..60 {
        game.step(4 << 8).unwrap();
    }
    for _ in 0..60 {
        game.step(0).unwrap();
    }
    game.doom_input(gamepkg::doom_input::Event::Key {
        code: 255,
        down: true,
    })
    .unwrap();
    game.step(0).unwrap();
    game.doom_input(gamepkg::doom_input::Event::Key {
        code: 255,
        down: false,
    })
    .unwrap();
    for _ in 0..12 {
        game.step(0).unwrap();
    }
    assert_ne!(game.builtin_controller().unwrap().unwrap() & (1 << 26), 0);
    game.step((1 << 18) | (1 << 19) | 64).unwrap();
    for _ in 0..30 {
        game.step(0).unwrap();
    }
    let position = game.builtin_state().unwrap().unwrap();
    assert!(position.1 > 0);
    let slot = original_slot_zero(&root);
    drop(game);
    fn saves(root: &std::path::Path) -> std::collections::BTreeMap<std::ffi::OsString, Vec<u8>> {
        std::fs::read_dir(root.join("saves"))
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                (entry.file_name(), std::fs::read(entry.path()).unwrap())
            })
            .collect()
    }
    let before = saves(&root);
    let error = match Game::load_target(
        &root,
        "com.gamepkg.doom",
        &inputs(&wad, &root.join("map-newflat.wad"), &patch),
        Some(native),
    ) {
        Err(error) => error,
        Ok(mut failed) => {
            let mut failure = None;
            for _ in 0..30 {
                if let Err(error) = failed.step(0) {
                    failure = Some(error);
                    break;
                }
            }
            drop(failed);
            failure.expect("Missing resource did not stop saved-game loading")
        }
    };
    assert!(
        format!("{error:#}").contains("PKGFLAT"),
        "Wrong failure cause: {error:#}"
    );
    assert!(
        saves(&root) == before,
        "Failed engine load changed the last committed save files"
    );
    let mut recovered =
        Game::load_target(&root, "com.gamepkg.doom", &selected, Some(native)).unwrap();
    for _ in 0..30 {
        recovered.step(0).unwrap();
    }
    let after = recovered.builtin_state().unwrap().unwrap();
    assert_eq!(after.0 & 0xffff, 0);
    assert_eq!(
        (after.1, after.2),
        (position.1, position.2),
        "Fresh engine did not recover exact saved coordinates"
    );
    assert!(
        original_slot_zero(&root) == slot,
        "Recovery changed original saved slot bytes"
    );
    assert_eq!(
        recovered.builtin_controller().unwrap().unwrap() & (1 << 26),
        0,
        "Recovered game remained paused"
    );
    for _ in 0..60 {
        recovered.step(8 << 8).unwrap();
    }
    let moved = recovered.builtin_state().unwrap().unwrap();
    let moved_x = i16::from_le_bytes(moved.1.to_le_bytes());
    let saved_x = i16::from_le_bytes(position.1.to_le_bytes());
    assert!(
        moved_x < saved_x && moved_x >= -112,
        "Recovered game did not accept original backward movement"
    );
}
