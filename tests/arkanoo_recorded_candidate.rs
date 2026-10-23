//! Private full-port migration test. No recordings or package fixtures are committed.
use gamepkg::{input_events::Event, music::Status, runtime::Game};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn event(game: &mut Game, kind: u32, x: f32, y: f32, code: u32) {
    game.input_event(Event { kind, x, y, code }).unwrap();
    game.step(0).unwrap();
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
    let bytes = std::fs::read(save).unwrap();
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
    println!("Full private port: audible original music, mute clock freeze, slider, ARK4 save and restart/resume passed; native={native}");
}
