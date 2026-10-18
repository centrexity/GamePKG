#[path = "support/arkanoo_canonical.rs"]
mod canonical;
use gamepkg::{drawing::Command, runtime::Game};
fn original_gravity_caption(commands: &[Command]) -> bool {
    commands.iter().any(|command|matches!(command,Command::LinearImage{x,y,width,height,image} if (*x,*y,*width,*height)==(281.0,229.5,78.0,15.5) && image.size()==[156,31]))
}
fn ball_position(command: &Command) -> Option<(f32, f32)> {
    if let Command::LinearImage {
        x,
        y,
        width,
        height,
        image,
    } = command
    {
        if (*width, *height) == (6.0, 6.0) && image.size() == [12, 12] {
            return Some((*x + 3.0, *y + 3.0));
        }
    }
    None
}
fn is_ball(command: &Command) -> bool {
    ball_position(command).is_some()
}
fn normal_brick_position(command: &Command) -> Option<(f32, f32)> {
    if let Command::LinearImage {
        x,
        y,
        width,
        height,
        image,
    } = command
    {
        if (*width, *height) == (30.0, 10.0) && image.size() == [60, 20] {
            return Some((*x, *y));
        }
    }
    None
}
fn is_normal_brick(command: &Command, color: usize, left: f32, top: f32) -> bool {
    if normal_brick_position(command) != Some((left, top)) {
        return false;
    }
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-blocks.json")).unwrap();
    let expected = hex::decode(fixture["images"][color]["rgba_hex"].as_str().unwrap()).unwrap();
    matches!(command, Command::LinearImage{image,..} if image.pixels()==expected.as_slice())
}
fn copy_library(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "saves" || name == "progress" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy_library(&entry.path(), &to.join(name));
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
fn isolated_library() -> tempfile::TempDir {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();
    copy_library(&source, root.path());
    root
}
fn load_menu(root: &std::path::Path, id: &str) -> anyhow::Result<Game> {
    let mut game = Game::load(root, id)?;
    if [
        "com.gamepkg.arkanoo",
        "com.gamepkg.arkanoo-files-component",
        "com.gamepkg.arkanoo-nofiles",
        "com.gamepkg.arkanoo-nosave",
        "com.gamepkg.arkanoo-nosave-component",
    ]
    .contains(&id)
    {
        game.input_event(gamepkg::input_events::Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 40,
        })?;
        game.step(0)?;
    }
    Ok(game)
}
#[test]
#[ignore = "requires the signed heart-animation fixture prepared by tests/arkanoo.py"]
fn original_heart_loss_states_rectangle_art_and_texture_positions_reach_runtime() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-penguin.json")).unwrap();
    let colors = [
        0xffc800ff, 0xffff96ff, 0x1e1e28ff, 0xf0f0faff, 0x646478ff, 0xff8c00ff, 0xdc143cff,
    ];
    for trace in fixture["traces"].as_array().unwrap() {
        let root = isolated_library();
        let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
        let lives = trace["lives"].as_u64().unwrap() as u32;
        for (index, frame) in trace["frames"].as_array().unwrap().iter().enumerate() {
            let state = game
                .step(1073741824 | if index == 0 { 1 | (lives << 4) } else { 0 })
                .unwrap();
            assert_eq!(state & 255, u64::from(lives - 1));
            assert_eq!((state >> 8) & 1, frame[4].as_f64().unwrap() as u64);
            let expected_state = frame[2].as_f64().unwrap() as u64;
            assert_eq!((state >> 16) & 15, expected_state);
            assert_eq!(state >> 24, frame[3].as_f64().unwrap() as u64);
            let actual: Vec<_> = game
                .frame()
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::Rect {
                        x,
                        y,
                        width,
                        height,
                        rgba,
                    } if colors.contains(rgba)
                        || *rgba == 0xffffffff && *width == 1.5 && *height == 1.5 =>
                    {
                        Some(([*x, *y, *width, *height], *rgba))
                    }
                    _ => None,
                })
                .collect();
            let expected: Vec<_> = trace["art"][index]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    (
                        [
                            r[0].as_i64().unwrap() as f32 * 0.5,
                            60.0 + r[1].as_i64().unwrap() as f32 * 0.5,
                            r[2].as_u64().unwrap() as f32 * 0.5,
                            r[3].as_u64().unwrap() as f32 * 0.5,
                        ],
                        r[4].as_u64().unwrap() as u32,
                    )
                })
                .collect();
            assert_eq!(actual, expected, "life {lives} frame {index}");
            let hearts: Vec<_> = game
                .frame()
                .commands
                .iter()
                .filter_map(|c| {
                    if let Command::StraightLinearImage {
                        x,
                        y,
                        width,
                        height,
                        image,
                    } = c
                    {
                        if image.size() != [512, 472] {
                            return None;
                        }
                        Some((*x, *y, *width, *height, image))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                hearts.len(),
                (lives - 1) as usize + frame[4].as_f64().unwrap() as usize
            );
            for (i, (x, y, w, h, image)) in hearts.iter().enumerate() {
                assert_eq!((*y, *w, *h), (67.5, 10.0, 10.0));
                assert_eq!(image.size(), [512, 472]);
                assert_eq!(*x, 620.0 - i as f32 * 12.5);
            }
            assert!(game.frame().commands.len() < 600);
        }
    }
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let before = game.step(1073741824 | 1 | (3 << 4)).unwrap();
    assert_eq!(
        game.step(1073741824 | 2).unwrap() & !0xf000,
        before & !0xf000
    );
    let resumed = game.step(1073741824 | 4).unwrap();
    assert_eq!(resumed >> 24, 1);
}
#[test]
#[ignore = "requires the signed asset-filter fixture prepared by tests/arkanoo.py"]
fn filtered_asset_sdk_preserves_pixels_cache_budgets_paths_and_failed_tick_rollback() {
    use gamepkg::drawing::image::Image;
    let expected = Image::from_png(include_bytes!("../ports/arkanoo/assets/heart.png")).unwrap();
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let mut previous = None;
    for _ in 0..2 {
        assert_eq!(game.step(536870912).unwrap(), 63);
        let Command::LinearImage { image: linear, .. } = &game.frame().commands[0] else {
            panic!("linear draw lost filtering")
        };
        let Command::Image { image: nearest, .. } = &game.frame().commands[1] else {
            panic!("nearest draw inherited linear filtering")
        };
        assert_eq!(linear.as_ref(), &expected);
        assert!(std::sync::Arc::ptr_eq(linear, nearest));
        if let Some(old) = &previous {
            assert!(
                std::sync::Arc::ptr_eq(old, linear),
                "immutable asset was decoded again"
            );
        }
        previous = Some(linear.clone());
        assert_eq!(game.frame().commands.len(), 2);
    }
    assert!(game.step(536870913).is_err());
    assert!(game.frame().commands.is_empty());
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(4).unwrap();
        let hearts: Vec<_> = game
            .frame()
            .commands
            .iter()
            .filter_map(|c| {
                if let Command::StraightLinearImage { image, .. } = c {
                    if image.size() != [512, 472] {
                        return None;
                    }
                    Some(image)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(hearts.len(), 3);
        assert_eq!(hearts[0].as_ref(), &expected);
        assert!(std::sync::Arc::ptr_eq(hearts[0], hearts[1]));
    }
}
#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn actual_splash_timer_skip_restart_and_editor_resume_preserve_original_flow() {
    use gamepkg::input_events::Event;
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = Game::load(root.path(), id).unwrap();
        for _ in 0..269 {
            game.step(0).unwrap();
            assert_eq!(game.frame().background, Some(0xffffffff));
            assert!(!game.frame().commands.iter().any(is_ball));
            assert!(game.window_state().visible && !game.window_state().confined);
        }
        game.step(0).unwrap();
        assert_eq!(game.frame().background, Some(0x101b35ff));
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 210.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        // The original main-loop timer has already expired, so restart returns
        // immediately to the paused menu rather than replaying a fresh splash.
        assert_eq!(game.frame().background, Some(0x101b35ff));
        drop(game);
        let mut game = Game::load(root.path(), id).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 185.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert_eq!(
            game.frame().background,
            Some(0x101b35ff),
            "skip click also started the game"
        );
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 185.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.frame().commands.iter().any(is_ball));
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        let paused = game.frame().commands.clone();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 260.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code: 27,
        })
        .unwrap();
        game.step(0).unwrap();
        assert_eq!(
            game.frame().commands,
            paused,
            "editor return lost the Resume menu/run"
        );
    }
}
#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn real_gameplay_audio_progress_and_restart() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.len() < 600);
    game.step(4).unwrap();
    game.step(8).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.step(8).unwrap();
    let mut audio = false;
    let mut broken = false;
    assert_eq!(
        game.frame()
            .commands
            .iter()
            .filter(|c| normal_brick_position(c).is_some())
            .count(),
        200
    );
    for _ in 0..2200 {
        game.step(0).unwrap();
        audio |= game.audio_frame().iter().any(|sample| *sample != 0);
        let bricks = game
            .frame()
            .commands
            .iter()
            .filter(|command| normal_brick_position(command).is_some())
            .count();
        broken |= bricks > 0 && bricks < 200 && game.frame().commands.iter().any(is_ball);
        assert!(game.frame().commands.len() < 600);
    }
    assert!(audio, "no synthesized collision audio");
    assert!(broken, "no bricks broken by actual game physics");
    drop(game);
    let saves: Vec<_> = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(saves.len(), 1);
    let bytes = std::fs::read(&saves[0]).unwrap();
    assert_eq!(&bytes[..4], b"ARK3");
    let score = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert!(score >= 10, "checkpoint did not retain the high score");
    let records = std::fs::read_dir(root.path().join("progress")).unwrap();
    assert!(records.filter_map(Result::ok).any(|entry| {
        std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains("first_brick"))
    }));
    let mut reloaded = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    reloaded.step(0).unwrap();
    reloaded.step(4).unwrap();
    assert!(reloaded.frame().commands.iter().any(is_ball));
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn gravity_option_is_visible_and_persists_through_the_real_save_api() {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();

    copy_library(&source, root.path());
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(131072).unwrap();
    let path = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(
        u32::from_le_bytes(std::fs::read(&path).unwrap()[12..16].try_into().unwrap()) & 1,
        1
    );
    game.finish().unwrap();
    drop(game);
    let mut resumed = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    resumed.step(0).unwrap();
    assert!(
        original_gravity_caption(&resumed.frame().commands),
        "original gravity caption absent after relaunch"
    );
    resumed.step(131072).unwrap();
    assert_eq!(
        u32::from_le_bytes(std::fs::read(path).unwrap()[12..16].try_into().unwrap()) & 1,
        0
    );
    resumed.finish().unwrap();
}

#[test]
#[ignore = "requires the production-physics portal fixture prepared by tests/arkanoo.py"]
fn portal_threshold_orbit_gravity_and_delayed_transition_reach_the_real_renderer() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(0).unwrap();
    assert_eq!(state >> 32, 5000);
    assert_eq!(state & 1, 1);
    assert_eq!((state >> 8) & 255, 3);
    let circles = game
        .frame()
        .commands
        .iter()
        .filter(|command| matches!(command, Command::Circle { .. }))
        .count();
    assert_eq!(circles, 0, "simplified portal circles are still rendered");
    assert!(
        game.frame().commands.iter().any(|command| matches!(command,
        Command::Image { width, height, .. } if *width > 100.0 && *height > 100.0)),
        "original portal layer was not rendered"
    );
    assert!(
        game.frame().commands.iter().any(is_ball),
        "orbiting ball was not rendered"
    );
    assert_eq!(game.step(131072).unwrap() & 2, 2);
    game.step(8).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.step(8).unwrap();
    game.step(64).unwrap();
    for _ in 0..268 {
        assert_eq!(game.step(0).unwrap() & 4, 0);
    }
    assert_eq!(game.step(0).unwrap() & 4, 4);
    let next = game.step(4).unwrap();
    assert_eq!(next & 1, 0);
    assert_eq!((next >> 16) & 255, 2);
    game.finish().unwrap();
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn multiball_ejection_and_shockwave_reach_the_real_renderer() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(4096).unwrap();
    assert_eq!(
        state & 8,
        8,
        "original upward/separating response did not run"
    );
    assert_eq!(state & 1, 0, "contact incorrectly activated the portal");
    assert_eq!(
        state >> 32,
        41,
        "five record particles plus the original 36-particle shockwave"
    );
    let shockwave = game
        .frame()
        .commands
        .iter()
        .filter(|command| matches!(command, Command::Image { image, .. } if image.pixels().chunks_exact(4).any(|p|p[3]==0) && image.pixels().chunks_exact(4).any(|p|p[..3]==[200,255,255] && p[3]>0)))
        .count();
    assert_eq!(shockwave, 1, "missing batched original shockwave shards");
}

#[test]
#[ignore = "requires the signed production-effects fixture prepared by tests/arkanoo.py"]
fn original_particle_points_and_alpha_reach_native_and_wasm_renderers_without_caps() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-particles.json")).unwrap();
    for (case_index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        for (frame_index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
            game.step(33554432 | case_index as u32 | ((frame_index as u32) << 2))
                .unwrap();
            let alpha = frame["state"][4].as_f64().unwrap() as u32;
            let mut expected = std::collections::BTreeMap::new();
            for point in frame["points"].as_array().unwrap() {
                let x = point[0].as_i64().unwrap() as i32;
                let y = point[1].as_i64().unwrap() as i32;
                if (0..1280).contains(&x) && (0..720).contains(&y) && alpha != 0 {
                    let pixel = expected.entry((x, y)).or_insert([25u8, 100, 200, 0]);
                    pixel[3] = ((alpha * 255 + u32::from(pixel[3]) * (255 - alpha)) / 255) as u8;
                }
            }
            let mut actual = std::collections::BTreeMap::new();
            for command in &game.frame().commands {
                if let Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = command
                {
                    let [w, h] = image.size();
                    assert_eq!((*width, *height), (w as f32 * 0.5, h as f32 * 0.5));
                    let left = (*x * 2.0) as i32;
                    let top = ((*y - 60.0) * 2.0) as i32;
                    for (index, pixel) in image.pixels().chunks_exact(4).enumerate() {
                        if pixel[3] != 0 {
                            actual.insert(
                                (left + (index % w) as i32, top + (index / w) as i32),
                                [pixel[0], pixel[1], pixel[2], pixel[3]],
                            );
                        }
                    }
                }
            }
            assert_eq!(actual, expected, "case {case_index} frame {frame_index}");
            assert!(game.frame().commands.len() <= 1);
        }
    }
    let count = game.step(67108864).unwrap();
    assert!(
        (2000..=3000).contains(&count),
        "full-board effects were capped: {count}"
    );
    assert!(game.frame().commands.len() < 600);
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{image,..} if image.pixels().chunks_exact(4).any(|p|p[3]>0))));
    assert!(
        game.frame()
            .commands
            .iter()
            .filter(|command| matches!(command,Command::Image{image,..} if image.size()[1]>50))
            .count()
            >= 2,
        "wide particle layer was not tiled"
    );
    let count = game.step(67108865).unwrap();
    assert!((2000..=3000).contains(&count));
    let mut corners = std::collections::BTreeSet::new();
    let mut tiles = 0;
    for command in &game.frame().commands {
        if let Command::Image { x, y, image, .. } = command {
            let [w, h] = image.size();
            if h <= 50 || image.pixels().chunks_exact(4).all(|pixel| pixel[3] == 255) {
                continue;
            }
            tiles += 1;
            assert!(w <= 512 && h <= 512);
            let left = (*x * 2.0) as i32;
            let top = ((*y - 60.0) * 2.0) as i32;
            for (index, pixel) in image.pixels().chunks_exact(4).enumerate() {
                let point = (left + (index % w) as i32, top + (index / w) as i32);
                if pixel[3] > 0 && [(0, 0), (1279, 0), (0, 719), (1279, 719)].contains(&point) {
                    corners.insert(point);
                }
            }
        }
    }
    assert!(
        (4..=6).contains(&tiles),
        "full-size particle layer lost tiles: {tiles}"
    );
    assert_eq!(
        corners.len(),
        4,
        "full-size particle layer lost visible corner shards"
    );
}
#[test]
#[ignore = "requires the signed production-trails fixture prepared by tests/arkanoo.py"]
fn original_ball_trails_comet_and_pulsing_ring_pixels_match_in_native_and_wasm() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_eq!(
        game.step(268435456).unwrap(),
        u64::from(u32::MAX) + 1,
        "game clock wrapped at 32 bits"
    );
    assert_eq!(
        game.step(268435457).unwrap(),
        u64::from(u32::MAX),
        "paused game clock advanced"
    );
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-trails.json")).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        game.step(134217728 | index as u32).unwrap();
        let mut expected = std::collections::BTreeMap::new();
        for point in case["points"].as_array().unwrap() {
            let x = point[0].as_i64().unwrap() as i32;
            let y = point[1].as_i64().unwrap() as i32;
            let color = point[2].as_u64().unwrap() as u32;
            let alpha = color & 255;
            if !(0..1280).contains(&x) || !(0..720).contains(&y) || alpha == 0 {
                continue;
            }
            let pixel = expected.entry((x, y)).or_insert([0u8; 4]);
            let old = u32::from(pixel[3]);
            let denominator = alpha * 255 + old * (255 - alpha);
            for (channel, value) in pixel.iter_mut().enumerate().take(3) {
                let source = (color >> (24 - channel * 8)) & 255;
                *value = ((source * alpha * 255 + u32::from(*value) * old * (255 - alpha))
                    / denominator) as u8;
            }
            pixel[3] = (denominator / 255) as u8;
        }
        let mut actual = std::collections::BTreeMap::new();
        for command in &game.frame().commands {
            if let Command::Image {
                x,
                y,
                width,
                height,
                image,
            } = command
            {
                let [w, h] = image.size();
                assert!(w <= 512 && h <= 512);
                assert_eq!((*width, *height), (w as f32 * 0.5, h as f32 * 0.5));
                let left = (*x * 2.0) as i32;
                let top = ((*y - 60.0) * 2.0) as i32;
                for (offset, pixel) in image.pixels().chunks_exact(4).enumerate() {
                    if pixel[3] != 0 {
                        actual.insert(
                            (left + (offset % w) as i32, top + (offset / w) as i32),
                            [pixel[0], pixel[1], pixel[2], pixel[3]],
                        );
                    }
                }
            }
        }
        assert_eq!(actual, expected, "original effect case {index}");
        assert!(game.frame().commands.len() <= 6);
    }
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn original_brick_geometry_and_pair_before_paddle_order_reach_real_runtime() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(1024).unwrap();
    assert_eq!(
        state & 16,
        16,
        "integer brick collision did not match original geometry"
    );
    assert!(game
        .frame()
        .commands
        .iter()
        .any(|command| ball_position(command) == Some((34.0, 113.0))));
    drop(game);
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_eq!(
        game.step(2048).unwrap() & 32,
        32,
        "pair contacts did not precede original paddle response"
    );
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn per_ball_launch_delay_and_life_victory_flow_work_in_real_runtime() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_ne!(game.step(8192).unwrap() & 64, 0);
    for _ in 1..30 {
        assert_ne!(game.step(0).unwrap() & 64, 0);
    }
    let state = game.step(0).unwrap();
    assert_eq!(state & 64, 0);
    assert_ne!(
        state & 128,
        0,
        "auto-launch moved before the original next frame"
    );
    game.step(0).unwrap();
    assert!(game
        .frame()
        .commands
        .iter()
        .any(|command| ball_position(command).is_some_and(|(_, y)| y == 390.0)));
    let state = game.step(16384).unwrap();
    assert_eq!(state >> 32, 80);
    assert_eq!((state >> 8) & 3, 2);
    assert_ne!(state & 2048, 0, "life loss erased active bonuses");
    let state = game.step(64).unwrap();
    assert_eq!((state >> 8) & 3, 3);
    assert_ne!(state & 1024, 0, "level nine did not enter victory");
    let state = game.step(4).unwrap();
    assert_eq!((state >> 16) & 65535, 10);
    assert_eq!(state & 2048, 0);
    assert_ne!(state & 64, 0);
    let state = game.step(32768).unwrap();
    assert_eq!(state >> 32, 10000004);
    let state = game.step(4).unwrap();
    assert_eq!((state >> 16) & 65535, 1000);
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn legacy_save_keeps_large_best_score_and_levels_above_the_old_cap() {
    let source = std::path::PathBuf::from(std::env::var_os("GAMEPKG_ARKANOO_TEST_ROOT").unwrap());
    let root = tempfile::tempdir().unwrap();
    copy_library(&source, root.path());
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(131072).unwrap();
    game.finish().unwrap();
    drop(game);
    let save = std::fs::read_dir(root.path().join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = b"ARK1".to_vec();
    bytes.extend_from_slice(&10000004u32.to_le_bytes());
    bytes.extend_from_slice(&1000u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    std::fs::write(&save, bytes).unwrap();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(4).unwrap();

    game.step(4).unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-hud.json")).unwrap();
    let expected = &reference["labels"]["Level 1000"];
    let width = expected["width"].as_u64().unwrap() as usize;
    let height = expected["height"].as_u64().unwrap() as usize;
    let encoded = hex::decode(expected["rgba_zlib_hex"].as_str().unwrap()).unwrap();
    let mut glyphs = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::ZlibDecoder::new(encoded.as_slice()),
        &mut glyphs,
    )
    .unwrap();
    let level = game
        .frame()
        .commands
        .iter()
        .find_map(|c| match c {
            Command::LinearImage { x, y, image, .. }
                if (*x, *y) == ((640 - width as i32 / 2) as f32 * 0.5, 65.0)
                    && image.size() == [width, height] =>
            {
                Some(image)
            }
            _ => None,
        })
        .expect("original Level 1000 label missing");
    assert_eq!(
        level
            .pixels()
            .chunks_exact(4)
            .map(|p| p[3])
            .collect::<Vec<_>>(),
        glyphs.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>(),
        "displayed level was clamped"
    );
    game.step(131072).unwrap();
    game.finish().unwrap();
    drop(game);
    let bytes = std::fs::read(&save).unwrap();
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        10000004
    );
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1000);
}

#[test]
#[ignore = "requires the production-physics fixture prepared by tests/arkanoo.py"]
fn special_bricks_ice_damage_and_world_radius_blasts_reach_real_renderers() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let state = game.step(65536).unwrap();
    assert_ne!(state & 4096, 0);
    assert_eq!(state >> 32, 0);
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==30.5 && *height==10.5 && image.pixels().chunks_exact(4).any(|pixel|pixel==[255,255,255,220]))),"damaged ice crack missing");
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,image,..} if *x==50.0 && *y==100.0 && image.pixels().chunks_exact(4).any(|pixel|pixel==[30,35,40,255]) && image.pixels().chunks_exact(4).any(|pixel|pixel==[150,150,160,255]))),"metal brick/rivets missing");
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x:80.0,y:100.0,width:30.5,height:10.5,image} if image.size()==[61,21] && image.pixels().chunks_exact(4).any(|p|p[0]>=220 && p[1]>=80 && p[3]==255))),"explosive brick missing");
    let state = game.step(262144).unwrap();
    assert_eq!(state >> 32, 10);
    assert_eq!(state & 4096, 0);
    let state = game.step(524288).unwrap();
    assert_eq!(state >> 32, 40);
    assert_ne!(
        state & 8192,
        0,
        "world-distance blast or no-chain rule incorrect"
    );
    assert!(
        game.frame()
            .commands
            .iter()
            .any(|command| matches!(command,Command::Image{x,y,..} if *x==20.0 && *y==160.5)),
        "outside-radius ice brick was removed"
    );
    game.step(1048576).unwrap();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands.iter().filter(|command|matches!(command,Command::Image{width,height,..} if *width==30.0 && *height==10.0)).count(),200);
    assert!(
        game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS,
        "special brick layers exceeded the existing drawing limits"
    );
}

#[test]
#[ignore = "requires the production-layout fixture prepared by tests/arkanoo.py"]
fn original_layouts_match_in_native_and_32_bit_wasm_through_the_real_renderer() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-levels.json")).unwrap();
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-levels").unwrap();
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap();
        assert_eq!(
            game.step(level as u32)
                .unwrap_or_else(|error| panic!("Original layout level {level}: {error:#}")),
            level
        );
        let mut actual = [0u8; 200];
        for command in &game.frame().commands {
            if let Some((x, y)) = normal_brick_position(command) {
                let col = ((x - 20.0) / 30.0) as usize;
                let row = ((y - 100.0) / 10.0) as usize;
                assert!(col < 20 && row < 10);
                assert_eq!(
                    (x, y),
                    (20.0 + col as f32 * 30.0, 100.0 + row as f32 * 10.0)
                );
                assert_eq!(actual[row * 20 + col], 0);
                actual[row * 20 + col] = 1;
            }
        }
        for (actual, expected) in actual
            .iter()
            .zip(fixture["cells"].as_str().unwrap().bytes())
        {
            assert_eq!(*actual, expected - b'0', "level {level}");
        }
    }
}

#[test]
#[ignore = "requires signed controlled-input fixtures prepared by tests/arkanoo.py"]
fn sdk_input_events_preserve_pointer_keys_unicode_and_permission_on_every_profile() {
    use gamepkg::input_events::Event;
    for suffix in ["events", "events-component"] {
        let root = isolated_library();
        let id = format!("com.gamepkg.arkanoo-{suffix}");
        let mut game = load_menu(root.path(), &id).unwrap();
        game.input_event(Event {
            kind: 1,
            x: 123.5,
            y: 44.25,
            code: 3,
        })
        .unwrap();
        assert_eq!(game.step(0).unwrap(), (1u64 << 32) | 3);
        assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Rect{x,y,width,height,..} if *x==123.5 && *y==44.25 && *width==1.0 && *height==1.0)));
        for (kind, code) in [
            (5, 65536 + 511),
            (7, 0x1f600),
            (7, 0x10ffff),
            (9, 0x80000000 + 114),
            (10, 0x40000000 + 80),
        ] {
            game.input_event(Event {
                kind,
                x: 0.0,
                y: 0.0,
                code,
            })
            .unwrap();
            assert_eq!(
                game.step(0).unwrap(),
                (u64::from(kind) << 32) | u64::from(code)
            );
        }
        assert_eq!(game.step(0).unwrap(), 0);
        drop(game);
        let mut denied = load_menu(root.path(), &format!("{id}-denied")).unwrap();
        assert!(denied.input_event(Event::RESET).is_err());
        assert_eq!(denied.step(0).unwrap(), u64::MAX);
    }
}

#[test]
#[ignore = "requires the signed Arkanoo fixture prepared by tests/arkanoo.py"]
fn actual_source_game_uses_mouse_position_and_original_space_escape_controls() {
    use gamepkg::input_events::Event;
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(4).unwrap();
    game.input_event(Event {
        kind: 1,
        x: 500.4,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 44,
    })
    .unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::LinearImage{x,y,width,height,image} if *x==465.0 && *y==395.0 && *width==70.0 && *height==11.0 && image.size()==[140,22])));
    assert!(
        game.frame()
            .commands
            .iter()
            .any(|command| ball_position(command).is_some_and(|(_, y)| y == 390.0)),
        "Space did not launch immediately"
    );
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 41,
    })
    .unwrap();
    game.step(0).unwrap();
    let paused = game.frame().commands.clone();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.input_event(Event {
        kind: 1,
        x: 100.0,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.step(0).unwrap();
    assert_eq!(game.frame().commands, paused);
    game.input_event(Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 41,
    })
    .unwrap();
    game.input_event(Event {
        kind: 1,
        x: 100.0,
        y: 200.0,
        code: 0,
    })
    .unwrap();
    game.step(0).unwrap();
    assert!(game.frame().commands.iter().any(|command|matches!(command,Command::LinearImage{x,y,width,height,image} if *x==65.0 && *y==395.0 && *width==70.0 && *height==11.0 && image.size()==[140,22])));
}

fn editor_key(game: &mut Game, key: u32) {
    game.input_event(gamepkg::input_events::Event {
        kind: 9,
        x: 0.0,
        y: 0.0,
        code: key,
    })
    .unwrap();
    game.step(0).unwrap();
}
fn editor_click(game: &mut Game, x: f32, y: f32, code: u32) {
    game.input_event(gamepkg::input_events::Event {
        kind: 2,
        x,
        y,
        code,
    })
    .unwrap();
    game.input_event(gamepkg::input_events::Event {
        kind: 3,
        x,
        y,
        code,
    })
    .unwrap();
    game.step(0).unwrap();
}
fn editor_archive(root: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(root.join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}
#[test]
#[ignore = "requires signed editor fixtures prepared by tests/arkanoo.py"]
fn original_editor_workflow_persists_patterns_and_preserves_corrupt_saves_in_real_runtimes() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==600.5 && *height==0.5 && image.size()==[1,1] && image.pixels()==[80,80,100,255])));
    for (index, key) in [54, 55, 56, 53].iter().enumerate() {
        editor_key(&mut game, *key);
        editor_click(&mut game, 20.0 + index as f32 * 30.0, 100.0, 1);
    }
    assert!(
        game.frame()
            .commands
            .iter()
            .any(|c| is_normal_brick(c, 5, 110.0, 100.0)),
        "original violet brick color was lost"
    );
    editor_key(&mut game, 115);
    let path = editor_archive(root.path());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"ARK3");
    assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 1);
    let name_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    assert_eq!(&bytes[24..24 + name_len], b"my_pattern");
    assert_eq!(&bytes[24 + name_len..28 + name_len], &[6, 7, 8, 5]);
    editor_key(&mut game, 116); // Test the actual edited board, then clear through the original Q action.
    game.step(8).unwrap(); // Pause and return to editing without completing the test.
    editor_key(&mut game, 101);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==600.5 && *height==0.5 && image.size()==[1,1] && image.pixels()==[80,80,100,255])));
    editor_key(&mut game, 116);
    assert_eq!(
        game.frame()
            .commands
            .iter()
            .filter(
                |c| matches!(c,Command::Image{width,height,image,..} if (*width==30.0 && *height==10.0)||(*width==30.5 && *height==10.5 && image.pixels().chunks_exact(4).any(|p|p[3]==255 && p[..3]!=[255,255,255])))
            )
            .count(),
        3
    );
    editor_key(&mut game, 113);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==600.5 && *height==0.5 && image.size()==[1,1] && image.pixels()==[80,80,100,255])));
    drop(game);
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 108);
    editor_key(&mut game, 13);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==30.0 && *height==10.0 && !image.pixels().chunks_exact(4).any(|pixel|pixel==[255,255,255,220]))),"loaded ice incorrectly appears damaged before being hit");
    assert_eq!(
        game.frame()
            .commands
            .iter()
            .filter(
                |c| matches!(c,Command::Image{width,height,image,..} if (*width==30.0 && *height==10.0)||(*width==30.5 && *height==10.5 && image.pixels().chunks_exact(4).any(|p|p[3]==255 && p[..3]!=[255,255,255])))
            )
            .count(),
        3
    );
    editor_key(&mut game, 115);
    let stored_again = std::fs::read(&path).unwrap();
    assert_eq!(
        &stored_again[16..],
        &bytes[16..],
        "saved pattern changed during test play"
    );
    assert_eq!(
        u32::from_le_bytes(stored_again[8..12].try_into().unwrap()),
        2
    );
    drop(game);
    let mut damaged = bytes.clone();
    *damaged.last_mut().unwrap() = 9;
    std::fs::write(&path, &damaged).unwrap();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    drop(game);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        damaged,
        "invalid save was overwritten"
    );
}
#[test]
#[ignore = "requires signed editor fixtures prepared by tests/arkanoo.py"]
fn denied_editor_saving_does_not_create_a_save_file() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-nosave").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    game.step(0).unwrap();
    editor_key(&mut game, 108);
    assert!(game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if (*x,*y,*width,*height)==(170.0,115.0,300.0,250.0) && image.pixels()==[40,45,52,255])),"saved-pattern browser did not open");
    assert!(!game.frame().commands.iter().any(|c|matches!(c,Command::Image{x,y,width,height,image} if *x==180.0 && (145.0..320.0).contains(y) && *width==280.0 && *height==15.0 && image.size()==[1,1])),"denied save appeared as a saved pattern row");
    drop(game);
    assert!(
        !root.path().join("saves").exists()
            || std::fs::read_dir(root.path().join("saves"))
                .unwrap()
                .next()
                .is_none()
    );
}
#[test]
#[ignore = "requires signed custom-level fixtures prepared by tests/arkanoo.py"]
fn original_custom_and_procedural_pool_matches_in_native_and_32_bit_wasm() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-custom-levels.json")).unwrap();
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-custom").unwrap();
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap() as u32;
        let expected = fixture["cells"]
            .as_array()
            .unwrap()
            .iter()
            .fold(14695981039346656037u64, |hash, cell| {
                (hash ^ cell.as_u64().unwrap()).wrapping_mul(1099511628211)
            });
        assert_eq!(
            game.step(level)
                .unwrap_or_else(|error| panic!("Custom layout level {level}: {error:#}")),
            expected,
            "level {level}"
        );
    }
}

#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn ascii_file_import_and_export_use_only_player_selected_bytes_across_all_profiles() {
    use gamepkg::user_files::{self, Kind, Outcome};
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.enable_user_files();
        game.step(0).unwrap();
        editor_key(&mut game, 101);
        editor_key(&mut game, 105);
        let cancelled = game.take_user_file_job().expect("import request");
        assert!(matches!(cancelled.kind, Kind::Import));
        assert!(game.complete_user_file(&cancelled, Outcome::Cancelled));
        game.step(0).unwrap();
        let selected = tempfile::tempdir().unwrap();
        let input = selected.path().join("Imported.TXT");
        let original = format!("# {}\n012345678IEU\n", "x".repeat(70000));
        std::fs::write(&input, &original).unwrap();
        editor_key(&mut game, 105);
        let import = game.take_user_file_job().unwrap();
        assert_ne!(import.handle, cancelled.handle);
        assert!(game.complete_user_file(&import, user_files::execute(&import, Some(&input))));
        game.step(0).unwrap();
        assert_eq!(
            std::fs::read_to_string(&input).unwrap(),
            original,
            "import changed the selected source"
        );
        editor_key(&mut game, 115);
        let saved = std::fs::read(editor_archive(root.path())).unwrap();
        let length = u32::from_le_bytes(saved[20..24].try_into().unwrap()) as usize;
        assert_eq!(&saved[24..24 + length], b"Imported");
        assert_eq!(
            &saved[24 + length..36 + length],
            &[0, 1, 2, 3, 4, 5, 6, 7, 8, 6, 7, 8]
        );
        editor_key(&mut game, 120);
        let export = game.take_user_file_job().unwrap();
        let Kind::Export { name, bytes } = &export.kind else {
            panic!("not an export");
        };
        assert_eq!(name, "Imported.txt");
        let mut expected=String::from("# Pattern: Imported\n# * = empty, 0-5 = normal, 6 = Ice, 7 = Explosive, 8 = Undestroyable\n\n012345678678********\n");
        for _ in 0..9 {
            expected.push_str("********************\n");
        }
        assert_eq!(bytes, expected.as_bytes());
        let output = selected.path().join("chosen export.txt");
        assert!(game.complete_user_file(&export, user_files::execute(&export, Some(&output))));
        game.step(0).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), expected.as_bytes());
        assert!(!selected.path().join("Imported.txt").exists());
        editor_key(&mut game, 105);
        let bad = game.take_user_file_job().unwrap();
        assert!(game.complete_user_file(
            &bad,
            Outcome::Imported {
                name: "bad.txt".into(),
                bytes: vec![255, 254]
            }
        ));
        game.step(0).unwrap();
        editor_key(&mut game, 115);
        assert_eq!(
            std::fs::read(editor_archive(root.path())).unwrap(),
            saved,
            "invalid imported text replaced the working pattern"
        );
    }
}
#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn editor_file_controls_do_not_grant_undeclared_access_or_open_headless_dialogs() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-nofiles").unwrap();
    game.enable_user_files();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_none());
    editor_key(&mut game, 120);
    assert!(game.take_user_file_job().is_none());
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_none());
    game.enable_user_files();
    game.step(0).unwrap();
    assert!(game.take_user_file_job().is_none());
    editor_key(&mut game, 105);
    assert!(game.take_user_file_job().is_some());
}
#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn pattern_archives_larger_than_one_read_chunk_reload_without_being_marked_corrupt() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_click(&mut game, 20.0, 100.0, 1);
    editor_key(&mut game, 115);
    let path = editor_archive(root.path());
    let initial = std::fs::read(&path).unwrap();
    drop(game);
    let length = u32::from_le_bytes(initial[20..24].try_into().unwrap()) as usize;
    let mut large = initial[..20].to_vec();
    large.extend(70000u32.to_le_bytes());
    large.extend(vec![b'x'; 70000]);
    large.extend(&initial[24 + length..]);
    std::fs::write(&path, &large).unwrap();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.step(0).unwrap();
    editor_key(&mut game, 101);
    editor_key(&mut game, 108);
    editor_key(&mut game, 13);
    editor_key(&mut game, 115);
    assert_eq!(std::fs::read(&path).unwrap(), large);
    assert!(
        game.frame()
            .commands
            .iter()
            .any(|c| is_normal_brick(c, 0, 20.0, 100.0)),
        "large archive's pattern was not loaded"
    );
}

#[test]
#[ignore = "requires signed user-file fixtures prepared by tests/arkanoo.py"]
fn play_history_failure_does_not_open_a_new_file_dialog() {
    let root = isolated_library();
    let mut setup = load_menu(root.path(), "com.gamepkg.arkanoo").unwrap();
    setup.step(0).unwrap();
    editor_key(&mut setup, 101);
    editor_click(&mut setup, 20.0, 100.0, 1);
    editor_key(&mut setup, 115);
    let path = editor_archive(root.path());
    drop(setup);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let progress = root.path().join("progress");
    if progress.exists() {
        std::fs::remove_dir_all(&progress).unwrap();
    }
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo").unwrap();
    game.enable_user_files();
    game.input_event(gamepkg::input_events::Event {
        kind: 5,
        x: 0.0,
        y: 0.0,
        code: 40,
    })
    .unwrap();
    std::fs::write(&progress, b"block progress commits").unwrap();
    for code in [101, 105] {
        game.input_event(gamepkg::input_events::Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code,
        })
        .unwrap();
    }
    assert!(game.step(0).is_err(), "progress failure was not exercised");
    assert!(
        game.take_user_file_job().is_none(),
        "a failed host tick exposed a file dialog"
    );
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn original_window_bindings_work_across_runtime_profiles() {
    use gamepkg::input_events::Event;
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().visible);
        game.step(4).unwrap();
        assert!(!game.window_state().visible);
        assert!(game.window_state().confined);
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 68,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().fullscreen_toggle);
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 68 | 65536,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(
            game.window_state().fullscreen_toggle,
            "key repeat toggled fullscreen"
        );
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().visible);
        assert!(!game.window_state().confined);
        game.input_event(Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code: 113,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.exited(), "original Q-to-quit was ignored in pause");
        assert!(game.take_user_file_job().is_none());
    }
}

#[test]
#[ignore = "requires the signed, isolated-built fixture prepared by tests/arkanoo.py"]
fn actual_menu_clicks_start_resume_restart_gravity_editor_settings_fullscreen_and_quit() {
    use gamepkg::input_events::Event;
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        assert_eq!(game.frame().background, Some(0x101b35ff));
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 235.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(original_gravity_caption(&game.frame().commands));
        assert_eq!(
            u32::from_le_bytes(
                std::fs::read(editor_archive(root.path())).unwrap()[12..16]
                    .try_into()
                    .unwrap()
            ) & 1,
            1,
            "gravity click did not persist its value"
        );
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 285.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 270.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().fullscreen_toggle);
        assert!(game.window_state().visible && !game.window_state().confined);
        let labels: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/arkanoo-menu-controls.json")).unwrap();
        let caption_matches = |commands: &[Command], fullscreen: bool| {
            let label = &labels["labels"][if fullscreen { "Fullscreen" } else { "Windowed" }];
            let width = label["width"].as_u64().unwrap() as usize;
            let compressed = hex::decode(label["rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut rgba = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
                &mut rgba,
            )
            .unwrap();
            commands.iter().any(|c|matches!(c,Command::LinearImage{x,y,width:w,height:h,image} if (*x,*y,*w,*h)==((640-width as i32/2) as f32*0.5,262.0,width as f32*0.5,15.5) && image.size()==[width,31] && image.pixels().chunks_exact(4).zip(rgba.chunks_exact(4)).all(|(a,b)|a[3]==b[3])))
        };
        game.step(0).unwrap();
        assert!(
            caption_matches(&game.frame().commands, false),
            "accepted request falsely predicted fullscreen without observation"
        );
        for (observation, expected) in [
            (Some(false), false),
            (Some(true), true),
            (None, true),
            (Some(false), false),
            (Some(true), true),
        ] {
            game.observe_fullscreen(observation);
            game.step(0).unwrap();
            assert!(
                caption_matches(&game.frame().commands, expected),
                "fullscreen caption did not follow actual observation {observation:?}"
            );
        }
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 450.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 185.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(!game.window_state().visible && game.window_state().confined);
        game.step(512).unwrap();
        let positions = game
            .frame()
            .commands
            .iter()
            .filter(|command| is_ball(command))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 185.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.window_state().confined);
        assert_eq!(
            game.frame()
                .commands
                .iter()
                .filter(|command| is_ball(command))
                .cloned()
                .collect::<Vec<_>>(),
            positions,
            "Resume reset or moved the paused ball"
        );
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 260.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.frame().commands.iter().any(|command|matches!(command,Command::Image{x,y,width,height,image} if *x==20.0 && *y==100.0 && *width==600.5 && *height==0.5 && image.size()==[1,1] && image.pixels()==[80,80,100,255])));
        game.input_event(Event {
            kind: 9,
            x: 0.0,
            y: 0.0,
            code: 27,
        })
        .unwrap();
        game.step(0).unwrap();
        game.step(4).unwrap();
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 210.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert_eq!(game.frame().background, Some(0xffffffff));
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 40,
        })
        .unwrap();
        game.step(0).unwrap();
        assert_eq!(game.frame().background, Some(0x101b35ff));
        assert!(game.frame().commands.iter().any(|command|matches!(command,Command::LinearImage{x,y,width,height,image} if (*x,*y,*width,*height)==(295.5,150.0,49.5,15.5) && image.size()==[99,31])),"Restart did not restore original PAUSED title");
        assert!(game.frame().commands.iter().any(|command|matches!(command,Command::LinearImage{x,y,width,height,image} if (*x,*y,*width,*height)==(296.5,179.5,47.0,15.5) && image.size()==[94,31])),"Restart did not restore original Resume caption");
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 185.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(
            game.frame()
                .commands
                .iter()
                .any(|command| ball_position(command).is_some_and(|(x, _)| x == 320.0)),
            "Restart kept the old paddle/ball position"
        );
        game.input_event(Event {
            kind: 5,
            x: 0.0,
            y: 0.0,
            code: 41,
        })
        .unwrap();
        game.step(0).unwrap();
        game.input_event(Event {
            kind: 2,
            x: 320.0,
            y: 310.0,
            code: 1,
        })
        .unwrap();
        game.step(0).unwrap();
        assert!(game.exited());
    }
}

fn advance_until_brick_break(game: &mut Game) {
    let before = game
        .frame()
        .commands
        .iter()
        .filter(|c| normal_brick_position(c).is_some())
        .count();
    assert!(before > 0);
    for _ in 0..480 {
        game.step(0).unwrap();
        let after = game
            .frame()
            .commands
            .iter()
            .filter(|c| normal_brick_position(c).is_some())
            .count();
        if after > 0 && after < before && game.frame().commands.iter().any(is_ball) {
            return;
        }
    }
    panic!("The launched ball did not break a brick within 480 actual ticks");
}

#[test]
#[ignore = "requires the signed isolated fixtures prepared by tests/arkanoo.py"]
fn effects_settings_change_pcm_and_survive_restarts_across_runtime_profiles() {
    fn event(game: &mut Game, kind: u32, x: f32, y: f32, code: u32) {
        game.input_event(gamepkg::input_events::Event { kind, x, y, code })
            .unwrap();
        game.step(0).unwrap();
    }
    fn slider(game: &mut Game, x: f32) {
        event(game, 2, 320.0, 245.0, 1);
        event(game, 1, 320.0, 245.0, 1);
        event(game, 1, x, 245.0, 1);
        event(game, 3, x, 245.0, 1);
    }
    fn launch(game: &mut Game) {
        event(game, 2, 320.0, 450.0, 1);
        event(game, 2, 320.0, 210.0, 1);
        event(game, 5, 0.0, 0.0, 40);
        event(game, 2, 320.0, 185.0, 1);
        event(game, 5, 0.0, 0.0, 44);
    }
    fn settings(game: &mut Game) {
        event(game, 5, 0.0, 0.0, 41);
        event(game, 2, 320.0, 285.0, 1);
    }
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        event(&mut game, 2, 320.0, 285.0, 1);
        slider(&mut game, 295.0);
        launch(&mut game);
        advance_until_brick_break(&mut game);
        assert_eq!(game.audio_frame().len(), 1600);
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(
            game.audio_frame().iter().map(|s| i32::from(*s).abs()).max(),
            Some(2000)
        );
        settings(&mut game);
        event(&mut game, 2, 320.0, 225.0, 1);
        slider(&mut game, 320.0);
        launch(&mut game);
        advance_until_brick_break(&mut game);
        assert!(
            game.audio_frame().is_empty(),
            "muted effects were submitted"
        );
        game.finish().unwrap();
        drop(game);
        let save = std::fs::read_dir(root.path().join("saves"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let bytes = std::fs::read(&save).unwrap();
        assert_eq!(&bytes[..4], b"ARK3");
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 640);
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        event(&mut game, 2, 320.0, 185.0, 1);
        event(&mut game, 5, 0.0, 0.0, 44);
        advance_until_brick_break(&mut game);
        assert!(
            game.audio_frame().is_empty(),
            "muting did not survive reopen/New Game"
        );
        settings(&mut game);
        event(&mut game, 2, 320.0, 225.0, 1);
        slider(&mut game, 390.0);
        launch(&mut game);
        advance_until_brick_break(&mut game);
        assert_eq!(
            game.audio_frame().iter().map(|s| i32::from(*s).abs()).max(),
            Some(8000)
        );
        settings(&mut game);
        slider(&mut game, 250.0);
        launch(&mut game);
        advance_until_brick_break(&mut game);
        assert!(game.audio_frame().is_empty(), "zero volume was audible");
        game.finish().unwrap();
        drop(game);
        let bytes = std::fs::read(&save).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()), 0);
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        event(&mut game, 2, 320.0, 185.0, 1);
        event(&mut game, 5, 0.0, 0.0, 44);
        advance_until_brick_break(&mut game);
        assert!(
            game.audio_frame().is_empty(),
            "saved zero volume became the legacy default"
        );
    }
}
#[test]
#[ignore = "requires the signed isolated fixtures prepared by tests/arkanoo.py"]
fn legacy_audio_defaults_migrate_and_denied_or_damaged_settings_preserve_data() {
    fn event(game: &mut Game, kind: u32, x: f32, y: f32, code: u32) {
        game.input_event(gamepkg::input_events::Event { kind, x, y, code })
            .unwrap();
        game.step(0).unwrap();
    }
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut seed = load_menu(root.path(), id).unwrap();
        seed.step(131072).unwrap();
        seed.finish().unwrap();
        drop(seed);
        let save = std::fs::read_dir(root.path().join("saves"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        for magic in [b"ARK1", b"ARK2"] {
            let mut legacy = magic.to_vec();
            for value in [17u32, 7, 0] {
                legacy.extend(value.to_le_bytes());
            }
            if magic == b"ARK2" {
                legacy.extend(0u32.to_le_bytes());
            }
            std::fs::write(&save, &legacy).unwrap();
            let mut game = load_menu(root.path(), id).unwrap();
            game.step(0).unwrap();
            event(&mut game, 2, 320.0, 185.0, 1);
            event(&mut game, 5, 0.0, 0.0, 44);
            advance_until_brick_break(&mut game);
            assert_eq!(
                game.audio_frame().iter().map(|s| i32::from(*s).abs()).max(),
                Some(4000)
            );
            game.finish().unwrap();
            drop(game);
            let bytes = std::fs::read(&save).unwrap();
            assert_eq!(&bytes[..4], b"ARK3");
            assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 17);
            assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 7);
        }
        let mut damaged = b"ARK3".to_vec();
        for value in [17u32, 7, 2048, 0] {
            damaged.extend(value.to_le_bytes());
        }
        std::fs::write(&save, &damaged).unwrap();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        event(&mut game, 2, 320.0, 285.0, 1);
        event(&mut game, 2, 320.0, 245.0, 1);
        event(&mut game, 1, 295.0, 245.0, 1);
        game.step(0).unwrap();
        assert!(
            game.frame()
                .commands
                .iter()
                .any(|command| matches!(command,Command::Image{y,..} if *y==330.0)),
            "damaged-save warning is absent"
        );
        game.finish().unwrap();
        drop(game);
        assert_eq!(std::fs::read(&save).unwrap(), damaged);
    }
    for id in [
        "com.gamepkg.arkanoo-nosave",
        "com.gamepkg.arkanoo-nosave-component",
    ] {
        let root = isolated_library();
        let mut game = load_menu(root.path(), id).unwrap();
        game.step(0).unwrap();
        event(&mut game, 2, 320.0, 285.0, 1);
        event(&mut game, 2, 320.0, 245.0, 1);
        event(&mut game, 1, 295.0, 245.0, 1);
        game.step(0).unwrap();
        assert!(
            game.frame()
                .commands
                .iter()
                .any(|command| matches!(command,Command::Image{y,..} if *y==330.0)),
            "denied-save warning is absent"
        );
        game.finish().unwrap();
        assert!(
            !root.path().join("saves").exists()
                || std::fs::read_dir(root.path().join("saves"))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }
}

#[test]
#[ignore = "requires the production audio fixture prepared by tests/arkanoo.py"]
fn effects_phase_at_maximum_frame_matches_in_native_and_wasm32() {
    let root = isolated_library();
    let mut game = load_menu(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (frame, input) in [(u64::MAX, 16777216), (u64::from(u32::MAX), 16777217)] {
        assert_eq!(game.step(input).unwrap(), 1);
        assert_eq!(game.audio_channels(), 1);
        assert_eq!(game.audio_frame().len(), 800);
        assert_eq!(game.audio_channels(), 1);
        for (index, sample) in game.audio_frame().iter().enumerate() {
            let sign = if (u128::from(frame) + index as u128) % 100 < 50 {
                1
            } else {
                -1
            };
            assert_eq!(
                *sample,
                (sign * (800 - index) as i32 * 10) as i16,
                "sample {index}"
            );
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_ball_texture_and_integer_geometry_reach_native_and_wasm() {
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-ball.json")).unwrap();
    let expected: Vec<u8> = fixture["rgba"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, (x, y)) in [(0.0, 60.0), (160.0, 180.0), (0.0, 60.0), (-0.5, 59.5)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(game.step(2147483648 | index as u32).unwrap(), 1);
        let images: Vec<_> = game
            .frame()
            .commands
            .iter()
            .filter_map(|command| {
                if let Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = command
                {
                    Some((*x, *y, *width, *height, image))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(images.len(), 1);
        let (left, top, width, height, image) = images[0];
        assert_eq!((left, top, width, height), (x, y, 6.0, 6.0));
        assert_eq!(image.size(), [12, 12]);
        assert_eq!(image.pixels(), expected.as_slice());
        assert!(!game
            .frame()
            .commands
            .iter()
            .any(|c| matches!(c, Command::Circle { .. })));
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_paddle_body_ghost_independence_and_spin_overlays_reach_native_and_wasm() {
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-paddle.json")).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let mut normal = Vec::new();
    for variant in 0..8 {
        assert_eq!(game.step(2147483656 | variant).unwrap(), 1);
        let commands = &game.frame().commands;
        let images: Vec<_> = commands
            .iter()
            .filter_map(|c| {
                if let Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = c
                {
                    Some((*x, *y, *width, *height, image))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(images.len(), 1);
        let (x, y, width, height, image) = images[0];
        let long = usize::from(variant == 1);
        assert_eq!(
            (x, y, width, height),
            (100.5, 395.0, if long == 1 { 90.0 } else { 70.0 }, 11.0)
        );
        assert_eq!(image.size(), [if long == 1 { 180 } else { 140 }, 22]);
        let expected: Vec<u8> = fixture["images"][long]["rgba"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(image.pixels(), expected.as_slice());
        let rects: Vec<_> = commands
            .iter()
            .filter_map(|c| {
                if let Command::Rect {
                    x,
                    y,
                    width,
                    height,
                    rgba,
                } = c
                {
                    Some((*x, *y, *width, *height, *rgba))
                } else {
                    None
                }
            })
            .collect();
        let reference_index = match variant {
            0 | 2 => 0,
            1 => 1,
            3 | 7 => 2,
            4 => 3,
            5 => 4,
            _ => 5,
        };
        let mut expected_rects = Vec::new();
        for rectangle in fixture["overlays"][reference_index]["rectangles"]
            .as_array()
            .unwrap()
        {
            let r = rectangle["rect"].as_array().unwrap();
            let x = r[0].as_f64().unwrap() as f32 * 0.5;
            let y = 60.0 + r[1].as_f64().unwrap() as f32 * 0.5;
            let width = r[2].as_f64().unwrap() as f32 * 0.5;
            let height = r[3].as_f64().unwrap() as f32 * 0.5;
            let color = rectangle["rgba"]
                .as_array()
                .unwrap()
                .iter()
                .fold(0u32, |v, c| (v << 8) | c.as_u64().unwrap() as u32);
            if rectangle["outline"].as_bool().unwrap() {
                expected_rects.extend([
                    (x, y, width, 0.5, color),
                    (x, y + height - 0.5, width, 0.5, color),
                    (x, y + 0.5, 0.5, height - 1.0, color),
                    (x + width - 0.5, y + 0.5, 0.5, height - 1.0, color),
                ]);
            } else {
                expected_rects.push((x, y, width, height, color));
            }
        }
        assert_eq!(
            rects, expected_rects,
            "Original overlay commands differ for variant {variant}"
        );
        if variant == 0 {
            normal = rects.clone();
        }
        if variant == 2 {
            assert_eq!(rects, normal, "Ghost ball must not recolor the paddle");
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_normal_brick_pixels_filtering_and_integer_rectangles_reach_native_and_wasm() {
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-blocks.json")).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for color in 0..6 {
        let expected = hex::decode(fixture["images"][color]["rgba_hex"].as_str().unwrap()).unwrap();
        for (position, (left, top)) in [(0.0, 60.0), (100.0, 100.0), (0.0, 60.0), (-0.5, 59.5)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                game.step(2147483664 | color as u32 | ((position as u32) << 5))
                    .unwrap(),
                1
            );
            let commands = &game.frame().commands;
            assert_eq!(commands.len(), 1);
            let Command::LinearImage {
                x,
                y,
                width,
                height,
                image,
            } = &commands[0]
            else {
                panic!("Normal brick did not use the filtered texture API")
            };
            assert_eq!((*x, *y, *width, *height), (left, top, 30.0, 10.0));
            assert_eq!(image.size(), [60, 20]);
            assert_eq!(image.pixels(), expected.as_slice());
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_metal_layers_preserve_outside_pixels_order_and_full_board_budgets() {
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-metal.json")).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (position, (left, top)) in [(0.0, 60.0), (100.0, 100.0), (0.0, 60.0), (-0.5, 59.5)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(game.step(2147483776 | position as u32).unwrap(), 1);
        assert_eq!(game.frame().commands.len(), 3);
        for (index, command) in game.frame().commands.iter().enumerate() {
            let Command::Image {
                x,
                y,
                width,
                height,
                image,
            } = command
            else {
                panic!("Metal primitive layers must use nearest sampling")
            };
            let layer = &fixture["layers"][index];
            let expected = hex::decode(layer["rgba_hex"].as_str().unwrap()).unwrap();
            assert_eq!(
                (*x, *y, *width, *height),
                if index == 0 {
                    (left, top, 30.0, 10.0)
                } else {
                    (left - 10.0, top, 45.5, 10.5)
                }
            );
            assert_eq!(image.size(), if index == 0 { [60, 20] } else { [91, 21] });
            assert_eq!(image.pixels(), expected.as_slice());
        }
    }
    for _ in 0..2 {
        let mut identities: [Option<std::sync::Arc<gamepkg::drawing::image::Image>>; 3] =
            [None, None, None];
        assert_eq!(game.step(2147483776 | 8).unwrap(), 200);
        let mut bodies = 0;
        let mut outside = 0;
        let mut bytes = 0;
        for command in &game.frame().commands {
            if let Command::Image { image, .. }
            | Command::LinearImage { image, .. }
            | Command::StraightLinearImage { image, .. } = command
            {
                bytes += image.pixels().len();
                let index = if image.size() == [60, 20] {
                    bodies += 1;
                    Some(0)
                } else if image.size() == [91, 21] {
                    outside += 1;
                    Some(if image.pixels()[80 * 4 + 3] == 30 {
                        1
                    } else {
                        2
                    })
                } else {
                    None
                };
                if let Some(index) = index {
                    if let Some(previous) = &identities[index] {
                        assert!(
                            std::sync::Arc::ptr_eq(previous, image),
                            "Alternating immutable assets were decoded again"
                        );
                    } else {
                        identities[index] = Some(image.clone());
                    }
                }
            }
        }
        assert_eq!((bodies, outside), (200, 400));
        assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
        assert!(bytes < gamepkg::drawing::MAX_IMAGE_BYTES);
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_ice_glints_refractions_cracks_and_clipped_shine_reach_real_runtimes() {
    let root = isolated_library();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-ice.json")).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    assert_eq!(reference["cases"].as_array().unwrap().len(), 96);
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(2147483904 | index as u32).unwrap(), 1);
        let commands = &game.frame().commands;
        assert_eq!(commands.len(), 3);
        let x = case["x"].as_i64().unwrap() as f32 * 0.5;
        let y = 60.0 + case["y"].as_i64().unwrap() as f32 * 0.5;
        let expected_alpha = case["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v[0] == "C")
            .unwrap()[4]
            .as_u64()
            .unwrap();
        assert!(
            matches!(commands[0], Command::Rect{x:left,y:top,width:30.0,height:10.0,rgba} if left==x && top==y && rgba==(0xc8f0ff00 | expected_alpha as u32))
        );
        for (layer, name) in [(1, "overlay"), (2, "shine")] {
            let Command::Image {
                x: left,
                y: top,
                width,
                height,
                image,
            } = &commands[layer]
            else {
                panic!("Ice primitives must use nearest sampling")
            };
            let size = if layer == 1 { [61, 21] } else { [60, 20] };
            assert_eq!(
                (*left, *top, *width, *height),
                (x, y, size[0] as f32 * 0.5, size[1] as f32 * 0.5)
            );
            assert_eq!(image.size(), size);
            let expected = hex::decode(
                reference["masks"][case[name].as_str().unwrap()]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            let actual: Vec<_> = image.pixels().chunks_exact(4).map(|p| p[3]).collect();
            let mismatch = actual.iter().zip(&expected).position(|(a, b)| a != b);
            assert!(
                mismatch.is_none(),
                "Ice mask case {index}, layer {name}: {:?}",
                mismatch.map(|at| (at, actual[at], expected[at]))
            );
            assert!(image
                .pixels()
                .chunks_exact(4)
                .all(|p| p[..3] == [255, 255, 255]));
        }
    }
    assert_eq!(game.step(2147483904 | 512).unwrap(), (5u64 << 32) | 1);
    let clock_case = &reference["cases"][16];
    assert_eq!(clock_case["frame"], 5);
    let expected = hex::decode(
        reference["masks"][clock_case["shine"].as_str().unwrap()]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let image = game
        .frame()
        .commands
        .iter()
        .find_map(|c| match c {
            Command::Image {
                x: 20.0,
                y: 100.0,
                image,
                ..
            } if image.size() == [60, 20] => Some(image),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        image
            .pixels()
            .chunks_exact(4)
            .map(|p| p[3])
            .collect::<Vec<_>>(),
        expected,
        "Editor must use its own animation clock"
    );
    for _ in 0..2 {
        assert_eq!(game.step(2147483904 | 128).unwrap(), 200);
        assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
        let images: Vec<_> = game
            .frame()
            .commands
            .iter()
            .filter_map(|c| match c {
                Command::Image { image, .. }
                    if image.size() == [61, 21] || image.size() == [60, 20] =>
                {
                    Some(image)
                }
                _ => None,
            })
            .collect();
        assert_eq!(images.len(), 400);
        assert!(
            images.iter().map(|i| i.pixels().len()).sum::<usize>()
                < gamepkg::drawing::MAX_IMAGE_BYTES
        );
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_fire_glass_blob_and_clipped_shine_match_real_sdl_wrappers() {
    use std::io::Read;
    let root = isolated_library();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-fire.json")).unwrap();
    assert_eq!(reference["sdl2_version"], "0.37.0");
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 64);
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(game.step(2147484672 | index as u32).unwrap(), 1);
        let commands = &game.frame().commands;
        assert_eq!(commands.len(), 1);
        let x = case["x"].as_i64().unwrap() as f32 * 0.5;
        let y = 60.0 + case["y"].as_i64().unwrap() as f32 * 0.5;
        let Command::Image {
            x: left,
            y: top,
            width,
            height,
            image,
        } = &commands[0]
        else {
            panic!("Fire brick must retain nearest source-resolution sampling")
        };
        assert_eq!((*left, *top, *width, *height), (x, y, 30.5, 10.5));
        assert_eq!(image.size(), [61, 21]);
        // Compare the complete composited brick against the unchanged original
        // SDL calls, including the moving one-pixel blob and translucent edges.
        for composite in case["composites"].as_array().unwrap() {
            let background = hex::decode(composite["background"].as_str().unwrap()).unwrap();
            let compressed =
                hex::decode(composite["original_rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(expected.len(), image.pixels().len());
            for (pixel, (source, target)) in image
                .pixels()
                .chunks_exact(4)
                .zip(expected.chunks_exact(4))
                .enumerate()
            {
                let alpha = u32::from(source[3]);
                for channel in 0..4 {
                    let actual = if channel == 3 {
                        alpha + (u32::from(background[3]) * (255 - alpha) + 127) / 255
                    } else {
                        (u32::from(source[channel]) * alpha
                            + u32::from(background[channel]) * (255 - alpha)
                            + 127)
                            / 255
                    } as u8;
                    assert!(actual.abs_diff(target[channel]) <= 3,
                        "Fire case {index}, background {}, pixel {pixel}, channel {channel}: {actual} vs {}",
                        composite["background"], target[channel]);
                }
            }
        }
        assert_eq!(case["composites"].as_array().unwrap().len(), 6);
        for composite in case["composites"].as_array().unwrap() {
            assert!(composite["blob_changed_bytes"].as_u64().unwrap() > 0);
            assert!(composite["max_channel_delta"].as_u64().unwrap() <= 3);
        }
    }
    for mixed in [0, 64] {
        for phase in 0..3 {
            for _ in 0..2 {
                assert_eq!(game.step(2147484672 | 128 | mixed | phase).unwrap(), 200);
                assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
                let images: Vec<_> = game
                    .frame()
                    .commands
                    .iter()
                    .filter_map(|c| match c {
                        Command::Image { image, .. }
                        | Command::LinearImage { image, .. }
                        | Command::StraightLinearImage { image, .. } => Some(image),
                        _ => None,
                    })
                    .collect();
                assert!(
                    images.iter().map(|i| i.pixels().len()).sum::<usize>()
                        < gamepkg::drawing::MAX_IMAGE_BYTES
                );
                if mixed == 0 {
                    assert_eq!(images.iter().filter(|i| i.size() == [61, 21]).count(), 200);
                    assert_eq!(images.iter().filter(|i| i.size() == [60, 20]).count(), 0);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_glass_bonus_layers_preserve_types_symbols_order_and_full_drop_budgets() {
    let root = isolated_library();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-bonus.json")).unwrap();
    assert_eq!(reference["canonical_renderer"], "opengl");
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 16);
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(game.step(2147485696 | index as u32).unwrap(), 1);
        let [Command::Image {
            x,
            y,
            width: 20.5,
            height: 20.5,
            image,
        }] = game.frame().commands.as_slice()
        else {
            panic!("Bonus must retain inclusive nearest-sampled bounds");
        };
        assert_eq!(
            (*x, *y),
            (
                case["x"].as_i64().unwrap() as f32 * 0.5,
                60.0 + case["y"].as_i64().unwrap() as f32 * 0.5
            )
        );
        assert_eq!(image.size(), [41, 41]);
        let expected = &reference["layers"][case["combined"].as_str().unwrap()];
        assert_eq!(
            image.pixels(),
            hex::decode(expected["rgba_hex"].as_str().unwrap()).unwrap()
        );
        for composite in case["composites"].as_array().unwrap() {
            let background = hex::decode(composite["background"].as_str().unwrap()).unwrap();
            let compressed =
                hex::decode(composite["original_rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut original = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
                &mut original,
            )
            .unwrap();
            assert_eq!(original.len(), image.pixels().len());
            for (pixel, (source, target)) in image
                .pixels()
                .chunks_exact(4)
                .zip(original.chunks_exact(4))
                .enumerate()
            {
                let alpha = u32::from(source[3]);
                for channel in 0..4 {
                    let actual = if channel == 3 {
                        alpha + (u32::from(background[3]) * (255 - alpha) + 127) / 255
                    } else {
                        (u32::from(source[channel]) * alpha
                            + u32::from(background[channel]) * (255 - alpha)
                            + 127)
                            / 255
                    } as u8;
                    assert!(actual.abs_diff(target[channel])<=3,"Bonus case {index}, background {}, pixel {pixel}, channel {channel}: {actual} versus {}",composite["background"],target[channel]);
                }
            }
        }
    }
    assert_eq!(game.step(2147485696 | 16).unwrap(), 0);
    assert!(game.frame().commands.is_empty());
    let mut stress = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let mut identities =
        std::collections::BTreeMap::<String, std::sync::Arc<gamepkg::drawing::image::Image>>::new();
    for _ in 0..2 {
        identities.clear(); // Background legitimately evicts earlier LRU entries.
        assert_eq!(stress.step(2147485696 | 32).unwrap(), 200);
        let commands = &stress.frame().commands;
        let clips: Vec<_> = commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                if let Command::Clip { rect } = command {
                    Some((index, *rect))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(clips[0].1, Some([0.0, 60.0, 640.0, 360.0]));
        assert_eq!(clips[1].1, None);
        assert!(
            clips[0].0 > 0 && clips[1].0 + 1 < commands.len(),
            "HUD must follow the object clip"
        );
        let score=commands.iter().position(|c|matches!(c,Command::LinearImage{x,y,height,..} if (*x,*y,*height)==(5.0,65.0,15.5))).expect("original score label missing");
        assert!(
            score > clips[1].0,
            "original HUD must follow gameplay objects"
        );
        assert!(
            matches!(
                commands[score - 1],
                Command::Clip {
                    rect: Some([5.0, 65.0, _, 15.5])
                }
            ),
            "score must use its world-scoped label clip"
        );
        let bonus_start = commands
            .iter()
            .position(|command| matches!(command,Command::Image{image,..} if image.size()==[41,41]))
            .unwrap();
        let bonus_end = bonus_start + 200;
        assert!(
            bonus_start > clips[0].0 && bonus_end <= clips[1].0,
            "Every bonus layer must be inside the world clip"
        );
        assert!(commands[bonus_start..bonus_end].iter().all(|command|matches!(command,Command::Image{image,..} if image.size()[0]<=41 && image.size()[1]<=41)));
        assert!(stress.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
        let bytes: usize = commands
            .iter()
            .filter_map(|command| match command {
                Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } => Some(image.pixels().len()),
                _ => None,
            })
            .sum();
        let mut count = 0;
        for command in &stress.frame().commands[bonus_start..bonus_end] {
            if let Command::Image { image, .. }
            | Command::LinearImage { image, .. }
            | Command::StraightLinearImage { image, .. } = command
            {
                if matches!(command, Command::Image { .. })
                    && image.size()[0] <= 41
                    && image.size()[1] <= 41
                {
                    let name = reference["cases"][(count % 4) * 4]["combined"]
                        .as_str()
                        .unwrap()
                        .to_owned();
                    if let Some(previous) = identities.get(&name) {
                        assert!(
                            std::sync::Arc::ptr_eq(previous, image),
                            "Bonus asset was decoded again: {name}"
                        );
                    } else {
                        identities.insert(name, image.clone());
                    }
                    count += 1;
                }
            }
        }
        assert_eq!(count, 200);
        assert_eq!(identities.len(), 4);
        assert!(bytes < gamepkg::drawing::MAX_IMAGE_BYTES);
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_rocket_art_and_integer_positions_reach_the_signed_runtime() {
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-rocket.json")).unwrap();
    let expected = hex::decode(fixture["rgba_hex"].as_str().unwrap()).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(2147485696 | 64 | index as u32).unwrap(), 1);
        let [Command::Image {
            x,
            y,
            width: 5.0,
            height: 10.0,
            image,
        }] = game.frame().commands.as_slice()
        else {
            panic!("Rocket must use one nearest-sampled original sprite")
        };
        assert_eq!(
            (*x, *y),
            (
                case["origin"][0].as_i64().unwrap() as f32 * 0.5,
                60.0 + case["origin"][1].as_i64().unwrap() as f32 * 0.5
            )
        );
        assert_eq!(image.size(), [10, 20]);
        assert_eq!(image.pixels(), expected);
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn production_rocket_rendering_uses_shared_art_within_unchanged_frame_budgets() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for _ in 0..2 {
        let mut previous = None; // Background legitimately evicts earlier LRU entries.
        assert_eq!(game.step(2147485696 | 128).unwrap(), 200);
        let mut count = 0;
        let mut bytes = 0;
        assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
        for command in &game.frame().commands {
            if let Command::Image {
                image,
                width: 5.0,
                height: 10.0,
                ..
            } = command
            {
                assert_eq!(image.size(), [10, 20]);
                if let Some(ref old) = previous {
                    assert!(std::sync::Arc::ptr_eq(old, image));
                }
                previous = Some(image.clone());
                count += 1;
            }
            if let Command::Image { image, .. }
            | Command::LinearImage { image, .. }
            | Command::StraightLinearImage { image, .. } = command
            {
                bytes += image.pixels().len();
            }
        }
        assert_eq!(count, 200);
        assert!(bytes < gamepkg::drawing::MAX_IMAGE_BYTES);
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_four_portal_stages_reach_signed_runtimes_with_unchanged_budgets() {
    use std::io::Read;
    let root = isolated_library();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-portal.json")).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(2147487744 | index as u32).unwrap(), 1);
        if case["bounds"].is_null() {
            assert!(game.frame().commands.is_empty());
            continue;
        }
        let [Command::Image {
            x,
            y,
            width,
            height,
            image,
        }] = game.frame().commands.as_slice()
        else {
            panic!("Each original portal layer fits one bounded nearest image")
        };
        let bounds: Vec<_> = case["bounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as f32)
            .collect();
        assert_eq!(
            (*x, *y, *width, *height),
            (
                bounds[0] * 0.5,
                60.0 + bounds[1] * 0.5,
                bounds[2] * 0.5,
                bounds[3] * 0.5
            )
        );
        assert_eq!(image.size(), [bounds[2] as usize, bounds[3] as usize]);
        let compressed = hex::decode(case["sdl_dark_rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut original = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut original)
            .unwrap();
        assert_eq!(original.len(), image.pixels().len());
        let mut delta = 0;
        for (pixel, source) in image.pixels().chunks_exact(4).zip(original.chunks_exact(4)) {
            for channel in 0..3 {
                let value = (u32::from(pixel[channel]) * u32::from(pixel[3])
                    + [11, 16, 32][channel] * (255 - u32::from(pixel[3])))
                    / 255;
                delta = delta.max(value.abs_diff(u32::from(source[channel])));
            }
        }
        assert!(
            delta <= 4,
            "portal SDL rounding delta {delta}, case {index}"
        );
        assert!(image.pixels().len() < gamepkg::drawing::MAX_IMAGE_BYTES);
    }
    for index in [0, 4, 5, 7, 8, 9, 10, 12] {
        for _ in 0..2 {
            assert_eq!(game.step(2147487744 | 32 | index).unwrap(), 1);
            assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
            let mut bytes = 0;
            for command in &game.frame().commands {
                if let Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } = command
                {
                    bytes += image.pixels().len();
                }
            }
            assert!(bytes < gamepkg::drawing::MAX_IMAGE_BYTES);
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_campaign_background_tiles_preserve_pixels_and_full_scene_budgets() {
    check_background_runtime_fixture(
        include_str!("fixtures/arkanoo-background.json"),
        2147491840,
        &[0, 3, 7, 10, 14, 17],
        32,
    );
}

fn check_procedural_runtime_fixture(text: &str, flag: u32, scenes: &[u32]) {
    check_background_runtime_fixture(text, flag, scenes, 64);
}
fn check_background_runtime_fixture(text: &str, flag: u32, scenes: &[u32], stress: u32) {
    use std::io::Read;
    let root = isolated_library();
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(flag | index as u32).unwrap_or_else(|error|panic!("Canonical background flag {flag:#x}, case {index}, level {}, frame {}: {error:#}",case["level"],case["frame"])), 1);
        let color = case["background"].as_u64().unwrap() as u32;
        assert!(
            matches!(game.frame().commands.first(),Some(Command::Rect{x:0.0,y:60.0,width:640.0,height:360.0,rgba}) if *rgba==color)
        );
        let mut actual = Vec::new();
        for _ in 0..1280 * 720 {
            actual.extend(color.to_be_bytes());
        }
        let mut covered = vec![false; 1280 * 720];
        for command in game.frame().commands.iter().skip(1) {
            let Command::Image {
                x,
                y,
                width,
                height,
                image,
            } = command
            else {
                panic!("Background must use bounded nearest tiles")
            };
            let [w, h] = image.size();
            let tile = if case["level"].as_u64().unwrap() <= 9 {
                64
            } else {
                128
            };
            assert!(w <= tile && h <= tile);
            assert_eq!((*width, *height), (w as f32 * 0.5, h as f32 * 0.5));
            let left = (*x * 2.0) as usize;
            let top = ((*y - 60.0) * 2.0) as usize;
            for (i, pixel) in image.pixels().chunks_exact(4).enumerate() {
                let at = (top + i / w) * 1280 + left + i % w;
                assert!(!covered[at], "Overlapping tiles blend source points twice");
                covered[at] = true;
                for channel in 0..3 {
                    actual[at * 4 + channel] = ((u32::from(pixel[channel]) * u32::from(pixel[3])
                        + u32::from(actual[at * 4 + channel]) * (255 - u32::from(pixel[3])))
                        / 255) as u8;
                }
            }
        }
        let reference = if case.get("opengl_rgba_zlib_hex").is_some() {
            case
        } else {
            canonical::case(
                case["level"].as_u64().unwrap() as u32,
                case["frame"].as_u64().unwrap(),
            )
        };
        let compressed = hex::decode(reference["sdl_rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        let delta = actual
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .flat_map(|(a, b)| (0..3).map(move |channel| a[channel].abs_diff(b[channel])))
            .max()
            .unwrap();
        assert_eq!(
            u64::from(delta),
            reference["maximum_backend_rgb_delta"].as_u64().unwrap(),
            "Original SDL renderer difference changed, case {index}"
        );
        let data = hex::decode(
            reference["opengl_rgba_zlib_hex"]
                .as_str()
                .expect("Canonical original OpenGL reference required"),
        )
        .unwrap();
        let mut pixels = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut pixels)
            .unwrap();
        assert_eq!(actual.len(), pixels.len());
        let differing = actual.iter().zip(pixels).position(|(a, b)| *a != b);
        assert!(
            differing.is_none(),
            "Original default GL pixels differ, case {index}, byte {differing:?}"
        );
    }
    for &index in scenes {
        for _ in 0..2 {
            assert_eq!(game.step(flag | stress | index).unwrap(), 3000);
            assert!(game.frame().commands.len() < gamepkg::drawing::MAX_COMMANDS);
            let mut bytes = 0;
            let mut fire = 0;
            let mut corners = 0;
            for command in &game.frame().commands {
                if let Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } = command
                {
                    bytes += image.pixels().len();
                    if image.size() == [61, 21] {
                        fire += 1;
                    }
                    if image.size()[0] > 64
                        && image.size()[1] > 64
                        && image
                            .pixels()
                            .chunks_exact(4)
                            .any(|p| p[..3] == [25, 100, 200] && p[3] > 0)
                    {
                        corners += 1;
                    }
                }
            }
            assert_eq!(fire, 200, "Background crowded out original brick art");
            assert_eq!(corners, 4, "Background crowded out full-window particles");
            assert!(bytes < gamepkg::drawing::MAX_IMAGE_BYTES);
        }
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_procedural_background_tiles_preserve_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-procedural.json"),
        2147500032,
        &[0, 3, 11, 14, 22, 25],
    );
}
#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_fire_and_deep_sea_backgrounds_preserve_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-fire-sea-background.json"),
        2147516416,
        &[0, 3, 11, 14],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_supernova_and_space_wars_preserve_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-space-background.json"),
        2147549184,
        &[0, 3, 11, 14],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_star_voyage_preserves_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-star-voyage.json"),
        2147614720,
        &[0, 3, 7, 8, 9, 10],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn sdl_default_line_coverage_and_clipping_reach_native_and_wasm() {
    use std::io::Read;
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sdl-background-lines.json")).unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let count = game.step(2147745792 | index as u32).unwrap();
        let data =
            hex::decode(case["images"]["opengl"]["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut pixels = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut pixels)
            .unwrap();
        let expected: std::collections::BTreeSet<_> = pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, p)| p[..3] != [11, 16, 32])
            .map(|(i, _)| i)
            .collect();
        assert_eq!(count, expected.len() as u64);
        if expected.is_empty() {
            assert!(game.frame().commands.is_empty());
            continue;
        }
        let [Command::Image {
            x,
            y,
            width,
            height,
            image,
        }] = game.frame().commands.as_slice()
        else {
            panic!("Expected one bounded line layer")
        };
        let [w, h] = image.size();
        assert!(w <= 64 && h <= 64);
        assert_eq!((*width, *height), (w as f32 * 0.5, h as f32 * 0.5));
        let left = (*x * 2.0) as usize;
        let top = ((*y - 60.0) * 2.0) as usize;
        let mut actual = std::collections::BTreeSet::new();
        for (i, pixel) in image.pixels().chunks_exact(4).enumerate() {
            if pixel[3] > 0 {
                assert_eq!(pixel, [200, 100, 255, 150]);
                actual.insert((top + i / w) * 64 + left + i % w);
            }
        }
        assert_eq!(actual, expected, "SDL line case {index}");
    }
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_kaleidoscope_preserves_line_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-kaleidoscope.json"),
        2148007936,
        &[0, 3, 7, 8, 9, 10],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_rain_and_warp_preserve_line_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-rain-warp.json"),
        2148532224,
        &[0, 3, 11, 14],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_cyber_and_aurora_preserve_line_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-cyber-aurora.json"),
        2149580800,
        &[0, 3, 11, 14, 20, 21],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_matrix_and_nebula_preserve_rectangle_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-matrix-nebula.json"),
        2151677952,
        &[0, 3, 5, 11, 14, 20],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_hex_and_bubbles_preserve_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-hex-bubbles.json"),
        2155872256,
        &[0, 3, 9, 11, 14],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn original_night_circuit_and_maze_preserve_pixels_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-night-circuit-maze.json"),
        2164260864,
        &[0, 5, 21, 24, 32, 35],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn canonical_particle_backgrounds_preserve_exact_frames_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-canonical-particles.json"),
        2181038080,
        &[0, 3, 11, 14, 22, 25, 33, 36],
    );
}
#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn canonical_effect_backgrounds_preserve_exact_frames_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-canonical-effects.json"),
        2214592512,
        &[0, 3, 11, 14, 22, 25, 32, 33, 36],
    );
}

#[test]
#[ignore = "requires the signed production fixture prepared by tests/arkanoo.py"]
fn canonical_campaign_backgrounds_preserve_exact_frames_and_full_scene_budgets() {
    check_procedural_runtime_fixture(
        include_str!("fixtures/arkanoo-canonical-campaign.json"),
        2281701376,
        &[0, 3, 7, 10, 14, 17],
    );
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_ball_speed_font_pixels_and_integer_placement_reach_every_profile() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-speed-font.json")).unwrap();
    for (index, speed) in [0, 60, 300, 600, 1200, 1800].iter().enumerate() {
        let label = format!("{speed} px/s");
        let case = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["text"] == label)
            .unwrap();
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        let source_width = case["width"].as_u64().unwrap() as u32;
        let width = (source_width as f32 * 0.6) as u32;
        let height = (31.0f32 * 0.6) as u32;
        for (pose, [px, py]) in [
            [0.0f32, 0.0f32],
            [320.75, 240.25],
            [-0.75, -0.75],
            [-1.75, -1.75],
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(
                game.step(2415919104 | (index as u32 * 4 + pose as u32))
                    .unwrap(),
                1
            );
            let [Command::LinearImage {
                x,
                y,
                width: w,
                height: h,
                image,
            }] = game.frame().commands.as_slice()
            else {
                panic!("Speed label did not use filtered drawing")
            };
            assert_eq!(
                (*x, *y, *w, *h),
                (
                    ((*px as i32 + 6) - width as i32 / 2) as f32 * 0.5,
                    60.0 + (*py as i32 - height as i32 - 5) as f32 * 0.5,
                    width as f32 * 0.5,
                    height as f32 * 0.5
                )
            );
            assert_eq!(image.size(), [source_width as usize, 31]);
            assert_eq!(image.pixels(), expected);
        }
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_six_campaign_image_backgrounds_preserve_full_pixels_and_dense_scene_budgets() {
    use sha2::Digest;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-image-backgrounds.json")).unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let root = isolated_library();
        let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
        assert_eq!(game.step(2684354560 | index as u32).unwrap(), 1);
        let commands = &game.frame().commands;
        assert_eq!(commands.len(), 7);
        assert!(matches!(
            commands[6],
            Command::Rect {
                x: 0.0,
                y: 60.0,
                width: 640.0,
                height: 360.0,
                rgba: 191
            }
        ));
        let mut bright = vec![0u8; 1280 * 720 * 4];
        for (command, tile) in commands[..6].iter().zip(case["tiles"].as_array().unwrap()) {
            let Command::Image {
                x,
                y,
                width,
                height,
                image,
            } = command
            else {
                panic!("Image background tile missing")
            };
            let left = tile["origin"][0].as_u64().unwrap() as usize;
            let top = tile["origin"][1].as_u64().unwrap() as usize;
            let w = tile["size"][0].as_u64().unwrap() as usize;
            let h = tile["size"][1].as_u64().unwrap() as usize;
            assert_eq!(
                (*x, *y, *width, *height),
                (
                    left as f32 * 0.5,
                    60.0 + top as f32 * 0.5,
                    w as f32 * 0.5,
                    h as f32 * 0.5
                )
            );
            assert_eq!(image.size(), [w, h]);
            assert_eq!(
                hex::encode(sha2::Sha256::digest(image.pixels())),
                tile["rgba_sha256"].as_str().unwrap()
            );
            for row in 0..h {
                bright[((top + row) * 1280 + left) * 4..((top + row) * 1280 + left + w) * 4]
                    .copy_from_slice(&image.pixels()[row * w * 4..(row + 1) * w * 4]);
            }
        }
        assert_eq!(
            hex::encode(sha2::Sha256::digest(&bright)),
            case["bright_rgba_sha256"].as_str().unwrap()
        );
        let compressed = hex::decode(case["game_rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        for pixel in bright.chunks_exact_mut(4) {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 64 + 127) / 255) as u8;
            }
        }
        assert_eq!(
            bright,
            expected,
            "Canonical original campaign image {}",
            index + 1
        );
        for _ in 0..2 {
            assert_eq!(game.step(2684354560 | 256 | index as u32).unwrap(), 3000);
            let commands = &game.frame().commands;
            assert!(commands.len() < gamepkg::drawing::MAX_COMMANDS);
            assert!(commands[..6].iter().all(|c| matches!(c,Command::Image{image,..} if image.size()[0]>=256 && image.size()[1]>=208)));
            let bytes: usize = commands
                .iter()
                .filter_map(|c| match c {
                    Command::Image { image, .. }
                    | Command::LinearImage { image, .. }
                    | Command::StraightLinearImage { image, .. } => Some(image.pixels().len()),
                    _ => None,
                })
                .sum();
            assert!(bytes <= gamepkg::drawing::MAX_IMAGE_BYTES);
            assert_eq!(
                commands
                    .iter()
                    .filter(|c| matches!(c,Command::Image{image,..} if image.size()==[61,21]))
                    .count(),
                200,
                "Dense fire overlays were dropped"
            );
            assert!(
                commands
                    .iter()
                    .any(|c| matches!(c, Command::Clip { rect: None })),
                "Full-scene drawing ended before clip reset"
            );
        }
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_editor_background_brightness_and_full_metal_boards_reach_native_and_core_wasm() {
    use sha2::Digest;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-image-backgrounds.json")).unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let root = isolated_library();
        let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
        for dense in [false, true, true] {
            assert_eq!(
                game.step(2684354560 | 512 | if dense { 1024 } else { 0 } | index as u32)
                    .unwrap(),
                index as u64 + 1
            );
            let commands = &game.frame().commands;
            assert!(matches!(
                commands[6],
                Command::Rect {
                    x: 0.0,
                    y: 60.0,
                    width: 640.0,
                    height: 360.0,
                    rgba: 153
                }
            ));
            let mut pixels = vec![0u8; 1280 * 720 * 4];
            for (command, tile) in commands[..6].iter().zip(case["tiles"].as_array().unwrap()) {
                let Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = command
                else {
                    panic!("Editor image tile missing")
                };
                let left = tile["origin"][0].as_u64().unwrap() as usize;
                let top = tile["origin"][1].as_u64().unwrap() as usize;
                let w = tile["size"][0].as_u64().unwrap() as usize;
                let h = tile["size"][1].as_u64().unwrap() as usize;
                assert_eq!(
                    (*x, *y, *width, *height),
                    (
                        left as f32 * 0.5,
                        60.0 + top as f32 * 0.5,
                        w as f32 * 0.5,
                        h as f32 * 0.5
                    )
                );
                assert_eq!(image.size(), [w, h]);
                for row in 0..h {
                    pixels[((top + row) * 1280 + left) * 4..((top + row) * 1280 + left + w) * 4]
                        .copy_from_slice(&image.pixels()[row * w * 4..(row + 1) * w * 4]);
                }
            }
            for pixel in pixels.chunks_exact_mut(4) {
                for channel in &mut pixel[..3] {
                    *channel = ((u16::from(*channel) * 102 + 127) / 255) as u8;
                }
            }
            assert_eq!(
                hex::encode(sha2::Sha256::digest(&pixels)),
                case["editor_rgba_sha256"].as_str().unwrap()
            );
            let bytes: usize = commands
                .iter()
                .filter_map(|command| match command {
                    Command::Image { image, .. }
                    | Command::LinearImage { image, .. }
                    | Command::StraightLinearImage { image, .. } => Some(image.pixels().len()),
                    _ => None,
                })
                .sum();
            assert!(bytes <= gamepkg::drawing::MAX_IMAGE_BYTES);
            assert!(commands.len() < gamepkg::drawing::MAX_COMMANDS);
            assert!(
                commands.iter().any(|c| matches!(
                    c,
                    Command::Image {
                        x: 110.0,
                        y: 432.0,
                        ..
                    }
                )),
                "Editor footer dropped"
            );
            if dense {
                assert_eq!(
                    commands
                        .iter()
                        .filter(|c| matches!(c,Command::Image{image,..} if image.size()==[91,21]))
                        .count(),
                    400
                );
            }
        }
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_editor_grid_and_backdrop_match_sdl_logical_pixels_and_endpoints() {
    use sha2::Digest;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-image-backgrounds.json")).unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let root = isolated_library();
        let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
        assert_eq!(
            game.step(2684354560 | 512 | index as u32).unwrap(),
            index as u64 + 1
        );
        let mut pixels = vec![0u8; 1280 * 720 * 4];
        for (command, tile) in game.frame().commands[..6]
            .iter()
            .zip(case["tiles"].as_array().unwrap())
        {
            let Command::Image { image, .. } = command else {
                panic!("Missing backdrop")
            };
            let left = tile["origin"][0].as_u64().unwrap() as usize;
            let top = tile["origin"][1].as_u64().unwrap() as usize;
            let [w, h] = image.size();
            for row in 0..h {
                pixels[((top + row) * 1280 + left) * 4..((top + row) * 1280 + left + w) * 4]
                    .copy_from_slice(&image.pixels()[row * w * 4..(row + 1) * w * 4]);
            }
        }
        for pixel in pixels.chunks_exact_mut(4) {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 102 + 127) / 255) as u8;
            }
        }
        let lines: Vec<_> = game
            .frame()
            .commands
            .iter()
            .filter_map(|command| {
                if let Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = command
                {
                    if image.size() == [1, 1] && image.pixels() == [80, 80, 100, 255] {
                        return Some((*x, *y, *width, *height));
                    }
                }
                None
            })
            .collect();
        let expected: Vec<_> = (0..11)
            .map(|row| (20.0, 100.0 + row as f32 * 10.0, 600.5, 0.5))
            .chain((0..21).map(|col| (20.0 + col as f32 * 30.0, 100.0, 0.5, 100.5)))
            .collect();
        assert_eq!(
            lines, expected,
            "Grid must retain original opaque RGB, order and both endpoints"
        );
        for (x, y, w, h) in lines {
            let left = (x * 2.0) as usize;
            let top = ((y - 60.0) * 2.0) as usize;
            let width = (w * 2.0) as usize;
            let height = (h * 2.0) as usize;
            for row in top..top + height {
                for col in left..left + width {
                    pixels[(row * 1280 + col) * 4..(row * 1280 + col) * 4 + 4]
                        .copy_from_slice(&[80, 80, 100, 255]);
                }
            }
        }
        assert_eq!(
            hex::encode(sha2::Sha256::digest(&pixels)),
            case["editor_grid_rgba_sha256"].as_str().unwrap()
        );
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_editor_palette_colors_borders_digits_and_order_match_sdl_pixels() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-editor-palette.json")).unwrap();
    let backgrounds: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-image-backgrounds.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(0xc0000000 | index as u32).unwrap(), index as u64);
        let commands = &game.frame().commands;
        let mut pixels = vec![0u8; 1280 * 720 * 4];
        for (command, tile) in commands[..6]
            .iter()
            .zip(backgrounds["cases"][0]["tiles"].as_array().unwrap())
        {
            let Command::Image { image, .. } = command else {
                panic!("missing editor backdrop")
            };
            let left = tile["origin"][0].as_u64().unwrap() as usize;
            let top = tile["origin"][1].as_u64().unwrap() as usize;
            let [w, h] = image.size();
            for row in 0..h {
                pixels[((top + row) * 1280 + left) * 4..((top + row) * 1280 + left + w) * 4]
                    .copy_from_slice(&image.pixels()[row * w * 4..(row + 1) * w * 4]);
            }
        }
        for pixel in pixels.chunks_exact_mut(4) {
            for channel in &mut pixel[..3] {
                *channel = ((u16::from(*channel) * 102 + 127) / 255) as u8;
            }
        }
        let first=commands.iter().position(|command|matches!(command,Command::Image{x,image,..} if *x>=429.0 && *x<=430.0 && image.size()==[1,1])).expect("palette outer border");
        let last_grid=commands.iter().rposition(|command|matches!(command,Command::Image{image,..} if image.size()==[1,1] && image.pixels()==[80,80,100,255])).unwrap();
        assert!(first > last_grid, "palette follows grid and board");
        for (slot, command) in commands[first..first + 27].iter().enumerate() {
            let (x, y, width, height, image) = match command {
                Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                } if slot % 3 != 2 => (*x, *y, *width, *height, image),
                Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } if slot % 3 == 2 => (*x, *y, *width, *height, image),
                _ => panic!("palette command order changed at {slot}"),
            };
            let left = (x * 2.0) as usize;
            let top = ((y - 60.0) * 2.0) as usize;
            let w = (width * 2.0) as usize;
            let h = (height * 2.0) as usize;
            assert_eq!(image.size(), if slot % 3 == 2 { [15, 31] } else { [1, 1] });
            for row in 0..h {
                for col in 0..w {
                    let source = if slot % 3 == 2 {
                        (row * w + col) * 4
                    } else {
                        0
                    };
                    let dst = ((top + row) * 1280 + left + col) * 4;
                    let alpha = u32::from(image.pixels()[source + 3]);
                    for channel in 0..3 {
                        pixels[dst + channel] = ((u32::from(image.pixels()[source + channel])
                            * alpha
                            + u32::from(pixels[dst + channel]) * (255 - alpha)
                            + 127)
                            / 255) as u8;
                    }
                }
            }
        }
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        let actual: Vec<u8> = (18..52)
            .flat_map(|row| {
                pixels[(row * 1280 + 858) * 4..(row * 1280 + 1262) * 4]
                    .iter()
                    .copied()
            })
            .collect();
        assert_eq!(actual.len(), expected.len());
        let max = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            max <= 1,
            "case {index}, selected {} hovered {}, max pixel difference {max}",
            case["selected"],
            case["hovered"]
        );
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn editor_original_labels_bounded_overlap_and_toolbar_hover_reach_signed_profiles() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-editor-labels.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for state in 0..8u32 {
        let bg = state % 6;
        let hovered = if state < 7 { state } else { 255 };
        assert_eq!(
            game.step(0xc0000040 | (hovered << 8) | (bg << 16)).unwrap(),
            64
        );
        let commands = &game.frame().commands;
        let mut labels = vec![
            (0, 10, 10),
            (1, 760, 25),
            (2, 296, 620),
            (10 + bg, 1084, 570),
        ];
        for which in 0..7u32 {
            let rect = if which < 5 {
                [225 + which as i32 * 170, 660, 150, 40]
            } else if which == 5 {
                [1020, 610, 120, 35]
            } else {
                [1150, 610, 120, 35]
            };
            let width = fixture["cases"][which as usize + 3]["width"]
                .as_i64()
                .unwrap() as i32;
            labels.push((
                which + 3,
                rect[0] + (rect[2] - width) / 2,
                rect[1] + (rect[3] - 31) / 2,
            ));
            let fill = if hovered == which {
                [100, 100, 150, 200]
            } else {
                [60, 60, 100, 180]
            };
            assert!(commands.iter().any(|command|matches!(command,Command::Image{x,y,width,height,image} if (*x,*y,*width,*height)==(rect[0] as f32*0.5,60.0+rect[1] as f32*0.5,rect[2] as f32*0.5,rect[3] as f32*0.5) && image.size()==[1,1] && image.pixels()==fill)));
        }
        for (which, left, top) in labels {
            let case = &fixture["cases"][which as usize];
            let full_width = case["width"].as_u64().unwrap() as usize;
            let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
                &mut expected,
            )
            .unwrap();
            let mut column = 0;
            while column < full_width {
                let right = (column + 256).min(full_width);
                let source_left = column.saturating_sub(1);
                let source_right = (right + 1).min(full_width);
                let tile_width = source_right - source_left;
                let rect = [
                    (left as f32 + column as f32) * 0.5,
                    60.0 + top as f32 * 0.5,
                    (right - column) as f32 * 0.5,
                    15.5,
                ];
                let at=commands.iter().position(|command|matches!(command,Command::Clip{rect:Some(actual)} if *actual==rect)).expect("label clip missing");
                let Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = &commands[at + 1]
                else {
                    panic!("unfiltered label")
                };
                assert_eq!(
                    (*x, *y, *width, *height),
                    (
                        (left as f32 + source_left as f32) * 0.5,
                        60.0 + top as f32 * 0.5,
                        tile_width as f32 * 0.5,
                        15.5
                    )
                );
                assert_eq!(image.size(), [tile_width, 31]);
                for row in 0..31 {
                    assert_eq!(
                        &image.pixels()[row * tile_width * 4..(row + 1) * tile_width * 4],
                        &expected[(row * full_width + source_left) * 4
                            ..(row * full_width + source_right) * 4],
                        "{} row {row}",
                        case["text"]
                    );
                }
                assert_eq!(commands[at + 2], Command::Clip { rect: None });
                column = right;
            }
        }
        assert!(commands.len() < 1024);
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn dynamic_unicode_editor_names_rows_messages_and_viewport_clips_match_original_surfaces() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-dynamic-font.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let long = "a".repeat(200);
    let names = [
        "my_pattern",
        "cafe\u{301}",
        "Ελληνικά Привет 日本語 🎮 // “Δ",
        long.as_str(),
    ];
    for (which, name) in names.iter().enumerate() {
        assert_eq!(game.step(0xc0000041 | ((which as u32) << 8)).unwrap(), 65);
        let heading = format!(
            "Name: {name}{}",
            if which == 1 {
                "_ (ESC to finish)"
            } else {
                " (N to edit)"
            }
        );
        for (text, x, y, color, centered, row_height) in [
            (
                heading.as_str(),
                10,
                40,
                if which == 1 {
                    [100, 255, 100]
                } else {
                    [200, 200, 200]
                },
                false,
                -1,
            ),
            (*name, 370, 170, [255, 255, 255], false, 30),
            ("Saved: café", 640, 590, [100, 255, 100], true, -1),
        ] {
            let case = fixture["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["text"].as_str() == Some(text))
                .unwrap_or_else(|| panic!("missing original {text:?}"));
            let full_width = case["width"].as_i64().unwrap() as i32;
            let h = case["height"].as_i64().unwrap() as i32;
            let left = if centered { x - full_width / 2 } else { x };
            let top = if row_height >= 0 {
                y + (row_height - h) / 2
            } else {
                y
            };
            let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
                &mut expected,
            )
            .unwrap();
            for pixel in expected.chunks_exact_mut(4) {
                pixel[..3].copy_from_slice(&color);
            }
            let mut column = (-left).clamp(0, full_width);
            let right = (1280 - left).clamp(0, full_width);
            let commands = &game.frame().commands;
            while column < right {
                let core_right = (column + 256).min(right);
                let source_left = (column - 1).max(0);
                let source_right = (core_right + 1).min(full_width);
                let tile_width = source_right - source_left;
                let rect = [
                    (left + column) as f32 * 0.5,
                    60.0 + top as f32 * 0.5,
                    (core_right - column) as f32 * 0.5,
                    h as f32 * 0.5,
                ];
                let at=commands.iter().position(|command|matches!(command,Command::Clip{rect:Some(actual)} if *actual==rect)).unwrap_or_else(||panic!("missing clip for {text:?} column {column}"));
                let Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } = &commands[at + 1]
                else {
                    panic!("unfiltered dynamic label")
                };
                assert_eq!(
                    (*x, *y, *width, *height),
                    (
                        (left + source_left) as f32 * 0.5,
                        60.0 + top as f32 * 0.5,
                        tile_width as f32 * 0.5,
                        h as f32 * 0.5
                    )
                );
                assert_eq!(image.size(), [tile_width as usize, h as usize]);
                for row in 0..h as usize {
                    assert_eq!(
                        &image.pixels()
                            [row * tile_width as usize * 4..(row + 1) * tile_width as usize * 4],
                        &expected[(row * full_width as usize + source_left as usize) * 4
                            ..(row * full_width as usize + source_right as usize) * 4],
                        "{text:?} row {row}"
                    );
                }
                assert_eq!(commands[at + 2], Command::Clip { rect: None });
                column = core_right;
            }
        }
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_pattern_browser_complete_pixels_and_dense_board_budget_match() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-pattern-browser.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for case in reference["cases"].as_array().unwrap() {
        assert_eq!(
            game.step(
                0xc0000042
                    | ((case["count"].as_u64().unwrap() as u32) << 8)
                    | ((case["selected"].as_u64().unwrap() as u32) << 16)
            )
            .unwrap(),
            66
        );
        let mut actual = vec![0u8; 1280 * 720 * 4];
        for pixel in actual.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[120, 180, 240, 255]);
        }
        let mut clip = [0, 0, 1280, 720];
        for command in &game.frame().commands {
            if let Command::Clip { rect } = command {
                clip = rect
                    .map(|r| {
                        [
                            (r[0] * 2.0).round() as i32,
                            ((r[1] - 60.0) * 2.0).round() as i32,
                            ((r[0] + r[2]) * 2.0).round() as i32,
                            ((r[1] + r[3] - 60.0) * 2.0).round() as i32,
                        ]
                    })
                    .unwrap_or([0, 0, 1280, 720]);
                continue;
            }
            let (x, y, w, h, pixels, iw, ih) = match command {
                Command::Rect {
                    x,
                    y,
                    width,
                    height,
                    rgba,
                } => (*x, *y, *width, *height, rgba.to_be_bytes().to_vec(), 1, 1),
                Command::Image {
                    x,
                    y,
                    width,
                    height,
                    image,
                }
                | Command::LinearImage {
                    x,
                    y,
                    width,
                    height,
                    image,
                } => (
                    *x,
                    *y,
                    *width,
                    *height,
                    image.pixels().to_vec(),
                    image.size()[0],
                    image.size()[1],
                ),
                _ => panic!("unexpected browser command"),
            };
            let left = (x * 2.0).round() as i32;
            let top = ((y - 60.0) * 2.0).round() as i32;
            let width = (w * 2.0).round() as i32;
            let height = (h * 2.0).round() as i32;
            assert!(iw == 1 || (iw == width as usize && ih == height as usize));
            for row in top.max(0).max(clip[1])..(top + height).min(720).min(clip[3]) {
                for col in left.max(0).max(clip[0])..(left + width).min(1280).min(clip[2]) {
                    let src = if iw == 1 {
                        0
                    } else {
                        ((row - top) as usize * iw + (col - left) as usize) * 4
                    };
                    let dst = (row as usize * 1280 + col as usize) * 4;
                    let alpha = u32::from(pixels[src + 3]);
                    for c in 0..3 {
                        actual[dst + c] = ((u32::from(pixels[src + c]) * alpha
                            + u32::from(actual[dst + c]) * (255 - alpha)
                            + 127)
                            / 255) as u8;
                    }
                }
            }
        }
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        assert_eq!(actual.len(), expected.len());
        let delta = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            delta <= 2,
            "count {} selected {}, max {delta}",
            case["count"],
            case["selected"]
        );
    }
    game.step(0xc0090a42).unwrap();
    let isolated = game.frame().commands[1..].to_vec();
    assert_eq!(game.step(0xc0000043).unwrap(), 67);
    let full = &game.frame().commands;
    let first=full.iter().position(|c|matches!(c,Command::Image{x,y,width,height,image} if (*x,*y,*width,*height)==(0.0,60.0,640.0,360.0) && image.pixels()==[0,0,0,200])).unwrap();
    assert_eq!(
        &full[first..first + isolated.len()],
        isolated.as_slice(),
        "dense board silently dropped browser commands"
    );
    assert!(full.len() < 1024);
    let bytes: usize = full
        .iter()
        .map(|c| match c {
            Command::Image { image, .. }
            | Command::LinearImage { image, .. }
            | Command::StraightLinearImage { image, .. } => image.pixels().len(),
            _ => 0,
        })
        .sum();
    assert!(bytes <= 8 * 1024 * 1024);
}

fn canonical_menu_pixels(frame: &gamepkg::drawing::Frame) -> Vec<u8> {
    let mut output = vec![0u8; 1280 * 720 * 4];
    for pixel in output.chunks_exact_mut(4) {
        pixel.copy_from_slice(&frame.background.unwrap_or(0x10171dff).to_be_bytes());
    }
    let mut clip = [0, 0, 1280, 720];
    for command in &frame.commands {
        if let Command::Clip { rect } = command {
            clip = rect
                .map(|r| {
                    [
                        (r[0] * 2.0).round() as i32,
                        ((r[1] - 60.0) * 2.0).round() as i32,
                        ((r[0] + r[2]) * 2.0).round() as i32,
                        ((r[1] + r[3] - 60.0) * 2.0).round() as i32,
                    ]
                })
                .unwrap_or([0, 0, 1280, 720]);
            continue;
        }
        let (x, y, w, h, pixels, iw, ih) = match command {
            Command::Rect {
                x,
                y,
                width,
                height,
                rgba,
            } => (*x, *y, *width, *height, rgba.to_be_bytes().to_vec(), 1, 1),
            Command::Image {
                x,
                y,
                width,
                height,
                image,
            }
            | Command::LinearImage {
                x,
                y,
                width,
                height,
                image,
            }
            | Command::StraightLinearImage {
                x,
                y,
                width,
                height,
                image,
            } => (
                *x,
                *y,
                *width,
                *height,
                image.pixels().to_vec(),
                image.size()[0],
                image.size()[1],
            ),
            _ => panic!("unexpected menu command"),
        };
        let left = (x * 2.0).round() as i32;
        let top = ((y - 60.0) * 2.0).round() as i32;
        let width = (w * 2.0).round() as i32;
        let height = (h * 2.0).round() as i32;
        let rescaled = iw != width as usize || ih != height as usize;
        assert!(
            (iw == 1 && ih == 1)
                || !rescaled
                || matches!(
                    command,
                    Command::LinearImage { .. } | Command::StraightLinearImage { .. }
                )
        );
        for row in top.max(0).max(clip[1])..(top + height).min(720).min(clip[3]) {
            for col in left.max(0).max(clip[0])..(left + width).min(1280).min(clip[2]) {
                let dst = (row as usize * 1280 + col as usize) * 4;
                if rescaled && (iw > 1 || ih > 1) {
                    let sx = (((col - left) as f64 + 0.5) * iw as f64 / width as f64 - 0.5)
                        .clamp(0.0, iw as f64 - 1.0);
                    let sy = (((row - top) as f64 + 0.5) * ih as f64 / height as f64 - 0.5)
                        .clamp(0.0, ih as f64 - 1.0);
                    let x0 = sx.floor() as usize;
                    let y0 = sy.floor() as usize;
                    let fx = sx - x0 as f64;
                    let fy = sy - y0 as f64;
                    let mut sample = [0.0f64; 4];
                    for (y, wy) in [(y0, 1.0 - fy), ((y0 + 1).min(ih - 1), fy)] {
                        for (x, wx) in [(x0, 1.0 - fx), ((x0 + 1).min(iw - 1), fx)] {
                            let at = (y * iw + x) * 4;
                            let alpha = u32::from(pixels[at + 3]);
                            for c in 0..3 {
                                let value =
                                    if matches!(command, Command::StraightLinearImage { .. }) {
                                        u32::from(pixels[at + c])
                                    } else {
                                        (u32::from(pixels[at + c]) * alpha + 127) / 255
                                    };
                                sample[c] += f64::from(value) * wx * wy;
                            }
                            sample[3] += f64::from(alpha) * wx * wy;
                        }
                    }
                    for c in 0..3 {
                        let foreground = if matches!(command, Command::StraightLinearImage { .. }) {
                            sample[c] * sample[3] / 255.0
                        } else {
                            sample[c]
                        };
                        output[dst + c] = (foreground
                            + f64::from(output[dst + c]) * (255.0 - sample[3]) / 255.0)
                            .round() as u8;
                    }
                    continue;
                }
                let src = if iw == 1 && ih == 1 {
                    0
                } else {
                    ((row - top) as usize * iw + (col - left) as usize) * 4
                };
                let a = u32::from(pixels[src + 3]);
                for c in 0..3 {
                    output[dst + c] = ((u32::from(pixels[src + c]) * a
                        + u32::from(output[dst + c]) * (255 - a)
                        + 127)
                        / 255) as u8;
                }
            }
        }
    }
    output
}
#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn implemented_menu_controls_and_original_backdrop_match_captured_sdl_frames() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-menu-controls.json")).unwrap();
    let compressed = hex::decode(reference["base_rgba_zlib_hex"].as_str().unwrap()).unwrap();
    let mut base = Vec::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut base)
        .unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(0xc0000044 | ((index as u32) << 8)).unwrap(), 68);
        let actual = canonical_menu_pixels(game.frame());
        let compressed = hex::decode(case["xor_zlib_hex"].as_str().unwrap()).unwrap();
        let mut difference = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut difference)
            .unwrap();
        assert_eq!(base.len(), difference.len());
        let expected: Vec<_> = base.iter().zip(difference).map(|(a, b)| a ^ b).collect();
        let delta = actual
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        assert!(delta <= 2, "menu controls case {index}, delta {delta}");
        assert_eq!(
            game.frame()
                .commands
                .iter()
                .filter(|c| matches!(c,Command::LinearImage{image,..} if image.size()[0]>1 && image.size()[1]>32))
                .count(),
            6,
            "original backdrop missing"
        );
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_splash_pixels_and_all_fade_alphas_reach_real_runtimes() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-splash.json")).unwrap();
    let compressed = hex::decode(reference["full_rgba_zlib_hex"].as_str().unwrap()).unwrap();
    let mut full = Vec::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut full)
        .unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (timer, alpha) in reference["alphas"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            game.step(0xc0000045 | ((timer as u32) << 8)).unwrap(),
            alpha.as_u64().unwrap(),
            "splash timer {timer}"
        );
        assert_eq!(game.frame().background, Some(0xffffffff));
        assert_eq!(
            game.frame()
                .commands
                .iter()
                .filter(|c| matches!(c, Command::LinearImage { .. }))
                .count(),
            6
        );
    }
    for case in reference["cases"].as_array().unwrap() {
        let timer = case["timer"].as_u64().unwrap() as u32;
        game.step(0xc0000045 | (timer << 8)).unwrap();
        let actual = canonical_menu_pixels(game.frame());
        let compressed = hex::decode(case["xor_zlib_hex"].as_str().unwrap()).unwrap();
        let mut difference = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut difference)
            .unwrap();
        assert_eq!(difference.len(), full.len());
        assert_eq!(actual.len(), full.len());
        let delta = actual
            .iter()
            .zip(full.iter().zip(difference))
            .map(|(a, (b, d))| a.abs_diff(b ^ d))
            .max()
            .unwrap();
        assert!(delta <= 2, "splash timer {timer}, delta {delta}");
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_end_state_overlay_frames_and_dense_scene_suffix_are_preserved() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-end-screens.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(0xc0000046 | ((index as u32) << 8)).unwrap(), 70);
        let actual = canonical_menu_pixels(game.frame());
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        let delta = actual
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        assert!(delta <= 2, "end screen {index}: {case}, delta {delta}");
    }
    for (phase, mode, level, empty) in [(0, 3, 9, 0), (1, 4, 1, 0), (2, 4, 1, 1), (3, 5, 9, 0)] {
        let index = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .position(|c| {
                c["mode"] == mode
                    && c["level"] == level
                    && c["score"] == u32::MAX
                    && c["empty"] == empty
            })
            .unwrap();
        game.step(0xc0000046 | ((index as u32) << 8)).unwrap();
        let isolated = game.frame().commands[1..].to_vec();
        assert_eq!(game.step(0xc0000047 | (phase << 8)).unwrap(), 71);
        let full = &game.frame().commands;
        let alpha = if mode == 3 { 150 } else { 180 };
        let first=full.iter().rposition(|c|matches!(c,Command::Rect{x,y,width,height,rgba} if (*x,*y,*width,*height,*rgba)==(0.0,60.0,640.0,360.0,alpha))).unwrap();
        assert_eq!(
            &full[first..],
            isolated.as_slice(),
            "full scene silently dropped end-screen commands for phase {phase}"
        );
        assert!(full.len() < 1024);
        let bytes: usize = full
            .iter()
            .map(|c| match c {
                Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } => image.pixels().len(),
                _ => 0,
            })
            .sum();
        assert!(bytes <= 8 * 1024 * 1024);
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_full_hud_matches_sdl_frames_and_survives_full_scene_budgets() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-hud.json")).unwrap();
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            game.step(0xc0000048 | ((index as u32) << 8)).unwrap(),
            72,
            "HUD case {index}"
        );
        let actual = canonical_menu_pixels(game.frame());
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        let (at, delta) = actual
            .iter()
            .zip(expected)
            .enumerate()
            .map(|(at, (a, b))| (at, a.abs_diff(b)))
            .max_by_key(|(_, d)| *d)
            .unwrap();
        assert!(
            delta <= 2,
            "HUD case {index}, pixel {}, channel {}, delta {delta}",
            at / 4,
            at % 4
        );
    }
    for (phase, index) in [0u32, 3, 17, 19].into_iter().enumerate() {
        game.step(0xc0000048 | (index << 8)).unwrap();
        let isolated = game.frame().commands[1..].to_vec();
        assert_eq!(game.step(0xc0000049 | ((phase as u32) << 8)).unwrap(), 73);
        let full = &game.frame().commands;
        let first = full
            .iter()
            .position(|c| matches!(c,Command::LinearImage{x,y,..} if (*x,*y)==(5.0,65.0)))
            .unwrap()
            - 1;
        assert!(full[first..] == isolated,
            "dense full scene lost HUD commands for phase {phase}: {} versus {} commands; first mismatch {:?}",
            full.len()-first, isolated.len(), full[first..].iter().zip(&isolated).position(|(a,b)| a!=b));
        assert!(full.len() < 1024);
        let bytes: usize = full
            .iter()
            .map(|c| match c {
                Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } => image.pixels().len(),
                _ => 0,
            })
            .sum();
        assert!(bytes <= 8 * 1024 * 1024);
    }
    game.step(0xc0000048 | (28 << 8)).unwrap();
    let isolated = game.frame().commands[1..].to_vec();
    game.step(2147485696 | 32).unwrap();
    let full = &game.frame().commands;
    let first = full
        .iter()
        .position(|c| matches!(c,Command::LinearImage{x,y,..} if (*x,*y)==(5.0,65.0)))
        .unwrap()
        - 1;
    assert!(
        full[first..] == isolated,
        "200 bonus icons crowded out the original HUD: {} versus {} commands; first mismatch {:?}",
        full.len() - first,
        isolated.len(),
        full[first..]
            .iter()
            .zip(&isolated)
            .position(|(a, b)| a != b)
    );
}
#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn actual_production_fps_uses_host_time_and_renders_original_counter_glyphs() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-hud.json")).unwrap();
    for id in ["com.gamepkg.arkanoo", "com.gamepkg.arkanoo-files-component"] {
        let root = isolated_library();
        let mut game = Game::load(root.path(), id).unwrap();
        let mut times = Vec::new();
        for (index, input) in [4, 4, 0, 0, 0].into_iter().enumerate() {
            if index == 3 {
                std::thread::sleep(std::time::Duration::from_millis(1010));
            }
            let state = game.step(0xc000004b | (input << 8)).unwrap();
            let now = state >> 32;
            let fps = state & u64::from(u32::MAX);
            times.push(now);
            let expected = times.iter().filter(|&&t| now - t < 1000000).count() as u64;
            assert_eq!(fps, expected);
            if index == 0 {
                assert_eq!(now, 0);
                continue;
            }
            let label = &reference["labels"][format!("FPS: {fps}")];
            let w = label["width"].as_u64().unwrap() as usize;
            let h = label["height"].as_u64().unwrap() as usize;
            let compressed = hex::decode(label["rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut pixels = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut pixels)
                .unwrap();
            let counter = game
                .frame()
                .commands
                .iter()
                .find_map(|c| match c {
                    Command::LinearImage {
                        x,
                        y,
                        width,
                        height,
                        image,
                    } if (*x, *y, *width, *height)
                        == (
                            (1270 - w as i32) as f32 * 0.5,
                            60.0 + (710 - h as i32) as f32 * 0.5,
                            w as f32 * 0.5,
                            h as f32 * 0.5,
                        ) =>
                    {
                        Some(image)
                    }
                    _ => None,
                })
                .expect("real FPS counter missing");
            assert_eq!(counter.pixels(), pixels);
        }
    }
}

#[test]
#[ignore = "requires tests/arkanoo.py fixtures"]
fn original_cannon_and_blink_caption_match_unchanged_sdl_and_full_scene() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-cannon.json")).unwrap();
    assert_eq!(reference["cases"].as_array().unwrap().len(), 60);
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    for (index, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        assert_eq!(game.step(0xc000004c | ((index as u32) << 8)).unwrap(), 76);
        let commands = &game.frame().commands;
        let enabled = case["ammo"].as_u64().unwrap() > 0;
        let visible = (case["frame"].as_u64().unwrap() / 30).is_multiple_of(2);
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(
                    c,
                    Command::Rect {
                        rgba: 0x646464ff | 0x969696ff,
                        ..
                    }
                ))
                .count(),
            if enabled { 2 } else { 0 }
        );
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, Command::LinearImage { .. }))
                .count(),
            usize::from(enabled && visible)
        );
        let actual = canonical_menu_pixels(game.frame());
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        let (at, delta) = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .map(|(at, (a, b))| (at, a.abs_diff(*b)))
            .max_by_key(|(_, d)| *d)
            .unwrap();
        assert!(
            delta <= 2,
            "Cannon case {index}, pixel {}, channel {}, delta {delta}",
            at / 4,
            at % 4
        );
    }
    for (phase, index) in [1u32, 3, 5, 9].into_iter().enumerate() {
        game.step(0xc000004c | (index << 8)).unwrap();
        let isolated = game.frame().commands[1..].to_vec();
        game.step(0xc000004d | ((phase as u32) << 8)).unwrap();
        let commands = &game.frame().commands;
        let start = commands
            .iter()
            .position(|c| {
                matches!(
                    c,
                    Command::Rect {
                        rgba: 0x646464ff,
                        ..
                    }
                )
            })
            .unwrap();
        assert!(
            commands[start..start + isolated.len()] == isolated,
            "Dense scene lost original cannon/caption, phase {phase}"
        );
        let hearts = commands
            .iter()
            .filter(
                |c| matches!(c,Command::StraightLinearImage{image,..} if image.size()==[512,472]),
            )
            .count();
        assert_eq!(hearts, 3, "Cannon crowded out HUD hearts");
        let bytes: usize = commands
            .iter()
            .map(|c| match c {
                Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. } => image.pixels().len(),
                _ => 0,
            })
            .sum();
        assert!(bytes <= gamepkg::drawing::MAX_IMAGE_BYTES);
        assert!(commands.len() < gamepkg::drawing::MAX_COMMANDS);
    }
}

#[test]
#[ignore = "requires signed fixtures prepared by tests/arkanoo.py"]
fn signed_sound_requests_match_original_brick_and_rocket_callbacks() {
    let root = isolated_library();
    let mut game = Game::load(root.path(), "com.gamepkg.arkanoo-modes").unwrap();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-bricks.json")).unwrap();
    for (index, case) in reference["fixtures"].as_array().unwrap().iter().enumerate() {
        for (step, expected) in case["expected"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                game.step(0x00fa0000 + index as u32 * 4 + step as u32)
                    .unwrap(),
                expected["sound"].as_u64().unwrap(),
                "{} step {}",
                case["name"],
                step
            );
        }
    }
}

#[test]
#[ignore = "requires signed fixtures prepared by tests/arkanoo.py"]
fn signed_four_channel_mixer_preserves_tails_capacity_and_stereo_output() {
    let root = isolated_library();
    let target = std::env::var("GAMEPKG_ARKANOO_MIXER_NATIVE")
        .ok()
        .map(|v| v.parse::<bool>().unwrap());
    let mut game =
        Game::load_target(root.path(), "com.gamepkg.arkanoo-modes", &[], target).unwrap();
    for case in 0..6 {
        assert_eq!(game.step(0x00fb0000 | case).unwrap(), u64::from(case + 1));
        if case == 2 {
            assert!(game.audio_frame().is_empty());
            continue;
        }
        assert_eq!(game.audio_channels(), 2);
        assert_eq!(game.audio_frame().len(), 1600);
        if case >= 4 {
            assert_eq!(&game.audio_frame()[..4], &[8000, 8000, 7990, 7990]);
            assert_eq!(&game.audio_frame()[48..50], &[0, 0]);
            continue;
        }
        let expected = match case {
            0 => [4600, -2600, 4600, -2600],
            1 => [11800, -11050, 1800, -1050],
            _ => [1800, -1050, 1800, -1050],
        };
        assert_eq!(&game.audio_frame()[..4], &expected);
        if case != 0 {
            assert!(game.audio_frame()[60..].iter().all(|v| *v == 0));
        }
    }
}
