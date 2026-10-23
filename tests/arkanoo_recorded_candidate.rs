//! Private full-port migration test. No recordings or package fixtures are committed.
use gamepkg::{input_events::Event, music::Status, runtime::Game};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn event(game: &mut Game, kind: u32, x: f32, y: f32, code: u32) {
    game.input_event(Event { kind, x, y, code }).unwrap();
    game.step(0)
        .unwrap_or_else(|error| panic!("input kind={kind} x={x} y={y} code={code}: {error:#}"));
}
fn assert_audible_music(output: &gamepkg::music::Output) {
    let start = Instant::now();
    let mut audible = false;
    while !audible {
        let mut pcm = [[0; 2]; 800];
        let count = output.read(&mut pcm);
        audible = pcm[..count].iter().flatten().any(|sample| *sample != 0);
        assert_ne!(output.status(), Status::Failed);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Original music never became audible"
        );
        std::thread::yield_now();
    }
}
#[test]
#[ignore = "requires a private recorded candidate with an unusable packaged playlist"]
fn unusable_packaged_playlist_keeps_original_effects_on_the_host() {
    let root = PathBuf::from(std::env::var_os("GAMEPKG_RECORDED_PORT_ROOT").unwrap());
    let native = std::env::var_os("GAMEPKG_RECORDED_PORT_NATIVE").is_some();
    for _ in 0..2 {
        let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
        let controls = Arc::new(Mutex::new(Vec::new()));
        let delivered = controls.clone();
        game.attach_effect_output(Arc::new(move |batch| {
            delivered.lock().unwrap().extend_from_slice(batch);
            true
        }));
        let mut playback = game.effect_playback().unwrap().unwrap();
        game.step(0).unwrap();
        let output = game.music_output().unwrap();
        assert_eq!(output.status(), Status::Stopped);
        event(&mut game, 5, 0.0, 0.0, 40);
        event(&mut game, 2, 320.0, 185.0, 1);
        event(&mut game, 5, 0.0, 0.0, 44);
        for _ in 0..480 {
            if controls
                .lock()
                .unwrap()
                .iter()
                .any(|command| matches!(command, gamepkg::audio_voices::EffectCommand::Play(_)))
            {
                break;
            }
            game.step(0).unwrap();
        }
        let mut plays = 0;
        for command in controls.lock().unwrap().drain(..) {
            match command {
                gamepkg::audio_voices::EffectCommand::Play(clip) => {
                    assert!(playback.effects().play(clip).is_some());
                    plays += 1;
                }
                gamepkg::audio_voices::EffectCommand::Volume(gain) => {
                    assert!(playback.effects().volume(gain))
                }
                gamepkg::audio_voices::EffectCommand::Pause(paused) => {
                    playback.effects().pause(paused)
                }
                gamepkg::audio_voices::EffectCommand::Stop => playback.effects().stop(),
            }
        }
        assert!(
            plays > 0,
            "Unusable music disabled recorded collision effects"
        );
        let mut audible = false;
        for _ in 0..4096 {
            audible |= playback.frame().unwrap().iter().any(|sample| *sample != 0);
        }
        assert!(
            audible,
            "Original effect became silent after music playlist failure"
        );
        assert!(
            game.audio_frame().is_empty(),
            "Unexpected guest PCM fallback"
        );
        assert_eq!(output.status(), Status::Stopped);
        game.finish().unwrap();
    }
}
#[test]
#[ignore = "requires full private candidate from prepare_arkanoo_recorded_port.py"]
fn complete_port_music_controls_save_and_restart_use_real_host_output() {
    let root = PathBuf::from(std::env::var_os("GAMEPKG_RECORDED_PORT_ROOT").unwrap());
    let native = std::env::var_os("GAMEPKG_RECORDED_PORT_NATIVE").is_some();
    let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
    let controls = Arc::new(Mutex::new(Vec::new()));
    let delivered = controls.clone();
    game.attach_effect_output(Arc::new(move |batch| {
        delivered.lock().unwrap().extend_from_slice(batch);
        true
    }));
    let mut playback = game.effect_playback().unwrap().unwrap();
    game.step(0).unwrap();
    let output = game.music_output().unwrap();
    assert_audible_music(&output);
    event(&mut game, 5, 0.0, 0.0, 40);
    event(&mut game, 2, 320.0, 285.0, 1);
    event(&mut game, 2, 320.0, 180.0, 1);
    assert_eq!(output.status(), Status::Paused);
    let played = output.played_frames();
    assert_eq!(output.read(&mut [[0; 2]; 800]), 0);
    assert_eq!(output.played_frames(), played);
    event(&mut game, 2, 320.0, 200.0, 1);
    event(&mut game, 1, 320.0, 200.0, 1);
    event(&mut game, 1, 295.0, 200.0, 1);
    event(&mut game, 3, 295.0, 200.0, 1);
    assert_eq!(output.status(), Status::Paused);
    event(&mut game, 2, 320.0, 450.0, 1);
    event(&mut game, 2, 320.0, 185.0, 1);
    event(&mut game, 5, 0.0, 0.0, 44);
    for _ in 0..480 {
        if controls
            .lock()
            .unwrap()
            .iter()
            .any(|control| matches!(control, gamepkg::audio_voices::EffectCommand::Play(_)))
        {
            break;
        }
        game.step(0).unwrap();
    }
    let mut plays = 0;
    for control in controls.lock().unwrap().drain(..) {
        match control {
            gamepkg::audio_voices::EffectCommand::Play(clip) => {
                assert!(playback.effects().play(clip).is_some());
                plays += 1;
            }
            gamepkg::audio_voices::EffectCommand::Volume(gain) => {
                assert!(playback.effects().volume(gain));
            }
            gamepkg::audio_voices::EffectCommand::Pause(paused) => playback.effects().pause(paused),
            gamepkg::audio_voices::EffectCommand::Stop => playback.effects().stop(),
        }
    }
    assert!(
        plays > 0,
        "Launched gameplay did not commit a collision effect"
    );
    let mut effect_audible = false;
    for _ in 0..4096 {
        effect_audible |= playback.frame().unwrap().iter().any(|sample| *sample != 0);
    }
    assert!(
        effect_audible,
        "Original collision effect was silent while music was paused"
    );
    assert!(
        game.audio_frame().is_empty(),
        "Recorded port submitted guest PCM"
    );
    drop(playback);
    game.finish().unwrap();
    drop(game);
    let save = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = std::fs::read(&save).unwrap();
    assert_eq!(&bytes[..4], b"ARK4");
    let flags = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!((flags >> 10) & 255, 32);
    assert_ne!(flags & (1 << 18), 0);
    let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
    game.attach_effect_output(Arc::new(|_| true));
    game.step(0).unwrap();
    let output = game.music_output().unwrap();
    assert_eq!(output.status(), Status::Paused);
    event(&mut game, 5, 0.0, 0.0, 40);
    event(&mut game, 2, 320.0, 285.0, 1);
    event(&mut game, 2, 320.0, 180.0, 1);
    assert!(matches!(output.status(), Status::Loading | Status::Playing));
    game.finish().unwrap();
    drop(game);
    // Exercise real guest decoding and re-encoding at the unchanged archive
    // ceiling while both recorded-audio shares remain enabled.
    let mut maximum = b"ARK4".to_vec();
    for word in [42u32, 9, 0, 1, 1048576 - 224] {
        maximum.extend_from_slice(&word.to_le_bytes());
    }
    if std::env::var_os("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").is_some() {
        let mut name = vec![b'A'];
        let late =
            std::env::var("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").as_deref() == Ok("late-composing");
        if late {
            name[0] = b'S';
        }
        let sample = if std::env::var("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").as_deref()
            == Ok("mixed")
        {
            "\u{fe0e}\u{fe0f}"
        } else if std::env::var("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").as_deref()
            == Ok("mixed-missing")
        {
            "\u{327}\u{328}"
        } else if std::env::var("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").as_deref()
            == Ok("mixed-class-missing")
        {
            "\u{327}\u{315}"
        } else if std::env::var("GAMEPKG_RECORDED_PORT_SELECTOR_RUN").as_deref() == Ok("missing") {
            "\u{327}"
        } else {
            "\u{fe0f}"
        };
        if late {
            name.extend_from_slice(&"\u{315}".as_bytes().repeat((1048576 - 228) / 2));
            name.extend_from_slice("\u{327}".as_bytes());
        } else {
            name.extend_from_slice(&sample.as_bytes().repeat((1048576 - 226) / sample.len()));
        }
        name.resize(1048576 - 224, b'x');
        assert!(std::str::from_utf8(&name).is_ok());
        maximum.extend_from_slice(&name);
    } else if let Ok(sample) = std::env::var("GAMEPKG_RECORDED_PORT_UNICODE") {
        assert!(!sample.is_empty());
        let mut name = sample.as_bytes().repeat((1048576 - 225) / sample.len());
        name.resize(1048576 - 224, b'x');
        assert!(std::str::from_utf8(&name).is_ok());
        maximum.extend_from_slice(&name);
    } else {
        maximum.resize(1048576 - 200, b'x');
    }
    maximum.resize(1048576, 6);
    std::fs::write(&save, &maximum).unwrap();
    let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
    game.attach_effect_output(Arc::new(|_| true));
    game.step(0).unwrap();
    event(&mut game, 5, 0.0, 0.0, 40);
    event(&mut game, 2, 320.0, 285.0, 1);
    event(&mut game, 2, 320.0, 180.0, 1);
    game.finish().unwrap();
    drop(game);
    let saved = std::fs::read(&save).unwrap();
    assert_eq!(
        saved.len(),
        1048576,
        "Maximum archive was discarded or truncated"
    );
    assert_eq!(
        &saved[16..],
        &maximum[16..],
        "Maximum pattern changed on settings save"
    );
    let flags = u32::from_le_bytes(saved[12..16].try_into().unwrap());
    assert_ne!(
        flags & (1 << 18),
        0,
        "Maximum archive did not commit the music toggle"
    );
    if std::env::var_os("GAMEPKG_RECORDED_PORT_LONG_NAMES").is_some() {
        let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
        game.attach_effect_output(Arc::new(|_| true));
        game.step(0).unwrap();
        event(&mut game, 5, 0.0, 0.0, 40);
        event(&mut game, 9, 0.0, 0.0, 101);
        event(&mut game, 9, 0.0, 0.0, 108);
        event(&mut game, 9, 0.0, 0.0, 13);
        if std::env::var_os("GAMEPKG_RECORDED_PORT_TYPING").is_some() {
            event(&mut game, 9, 0.0, 0.0, 110);
            event(&mut game, 9, 0.0, 0.0, 8);
            event(&mut game, 7, 0.0, 0.0, 121);
            event(&mut game, 9, 0.0, 0.0, 8);
            event(&mut game, 7, 0.0, 0.0, 120);
            event(&mut game, 9, 0.0, 0.0, 27);
        }
        event(&mut game, 9, 0.0, 0.0, 115);
        game.finish().unwrap();
        drop(game);
        assert_eq!(
            std::fs::read(&save).unwrap(),
            saved,
            "Long active name changed in the editor"
        );
    }
    let count = (1048576 - 20) / 208;
    let extra = 1048576 - 20 - count * 208;
    let mut collection = b"ARK4".to_vec();
    for word in [42u32, 9, 0, count as u32] {
        collection.extend_from_slice(&word.to_le_bytes());
    }
    for index in 0..count {
        let mut name = format!("{index:04}");
        if index + 1 == count {
            name.push_str(&"x".repeat(extra));
        }
        collection.extend_from_slice(&(name.len() as u32).to_le_bytes());
        collection.extend_from_slice(name.as_bytes());
        collection.extend_from_slice(&[6; 200]);
    }
    assert_eq!(collection.len(), 1048576);
    std::fs::write(&save, &collection).unwrap();
    let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
    game.attach_effect_output(Arc::new(|_| true));
    game.step(0).expect("Maximum collection startup");
    event(&mut game, 5, 0.0, 0.0, 40);
    event(&mut game, 2, 320.0, 285.0, 1);
    event(&mut game, 2, 320.0, 180.0, 1);
    game.finish().unwrap();
    drop(game);
    let saved = std::fs::read(&save).unwrap();
    assert_eq!(
        &saved[16..],
        &collection[16..],
        "Maximum collection changed on settings save"
    );
    assert_ne!(
        u32::from_le_bytes(saved[12..16].try_into().unwrap()) & (1 << 18),
        0
    );
    let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
    game.attach_effect_output(Arc::new(|_| true));
    game.step(0).unwrap();
    event(&mut game, 5, 0.0, 0.0, 40);
    event(&mut game, 9, 0.0, 0.0, 101); // Editor.
    event(&mut game, 9, 0.0, 0.0, 108); // Pattern browser.
    event(&mut game, 9, 0.0, 0.0, 13); // Load the first pattern.
    event(&mut game, 2, 20.0, 100.0, 2); // Erase its first cell.
    event(&mut game, 3, 20.0, 100.0, 2);
    event(&mut game, 9, 0.0, 0.0, 115); // Store over the existing pattern.
    game.finish().unwrap();
    drop(game);
    let edited = std::fs::read(&save).unwrap();
    let mut expected = saved;
    expected[28] = 255;
    assert_eq!(
        edited, expected,
        "Full collection editor save changed unrelated bytes or failed to store"
    );
    for (magic, flags) in [
        (b"ARK1", 2u32),
        (b"ARK1", u32::MAX),
        (b"ARK2", 2),
        (b"ARK3", 2048),
        (b"ARK4", 129 << 10),
        (b"ARK4", 1 << 19),
    ] {
        let mut damaged = magic.to_vec();
        for word in [17u32, 7, flags] {
            damaged.extend(word.to_le_bytes());
        }
        if magic != b"ARK1" {
            damaged.extend(0u32.to_le_bytes());
        }
        std::fs::write(&save, &damaged).unwrap();
        let mut game = Game::load_target(&root, "com.gamepkg.arkanoo", &[], Some(native)).unwrap();
        game.attach_effect_output(Arc::new(|_| true));
        game.step(0).unwrap();
        let output = game.music_output().unwrap();
        assert_audible_music(&output);
        event(&mut game, 5, 0.0, 0.0, 40);
        event(&mut game, 2, 320.0, 285.0, 1);
        assert!(
            game.frame().commands.iter().any(
                |command| matches!(command,gamepkg::drawing::Command::Image{y,..} if *y==330.0)
            ),
            "Corrupt-save warning missing for {magic:?}, {flags:#x}"
        );
        event(&mut game, 2, 320.0, 180.0, 1);
        assert_eq!(output.status(), Status::Paused);
        event(&mut game, 2, 320.0, 245.0, 1);
        event(&mut game, 1, 295.0, 245.0, 1);
        event(&mut game, 3, 295.0, 245.0, 1);
        event(&mut game, 2, 320.0, 180.0, 1);
        assert_audible_music(&output);
        game.finish().unwrap();
        drop(game);
        assert_eq!(
            std::fs::read(&save).unwrap(),
            damaged,
            "Corrupt {magic:?} flags {flags:#x} were overwritten"
        );
    }
    println!("All six corrupt-save cases preserve original bytes and retain real music/mute/resume; native={native}");
    println!("Full private port: audible original music, mute clock freeze, slider, ARK4 save and restart/resume passed; native={native}");
}
