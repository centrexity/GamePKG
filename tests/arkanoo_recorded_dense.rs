//! Source-state probes retain production rendering and real host audio services.
use gamepkg::{audio_voices::EffectCommand, drawing::Command, music::Status, runtime::Game};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires private recipes from check_arkanoo_recorded_dense.py"]
fn full_recorded_audio_shares_preserve_dense_scene_commands_and_pixels() {
    let root = PathBuf::from(std::env::var_os("GAMEPKG_RECORDED_DENSE_ROOT").unwrap());
    let native = std::env::var("GAMEPKG_RECORDED_DENSE_NATIVE").unwrap() == "1";
    let mut baseline = Game::load_target(
        &root,
        "com.gamepkg.arkanoo-dense-baseline",
        &[],
        Some(native),
    )
    .unwrap();
    let mut recorded = Game::load_target(
        &root,
        "com.gamepkg.arkanoo-dense-recorded",
        &[],
        Some(native),
    )
    .unwrap();
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let sink = delivered.clone();
    recorded.attach_effect_output(Arc::new(move |batch| {
        sink.lock().unwrap().extend_from_slice(batch);
        true
    }));
    baseline.step(0).unwrap();
    recorded.step(0).unwrap();
    let output = recorded.music_output().unwrap();
    let start = Instant::now();
    loop {
        let mut pcm = [[0; 2]; 800];
        let count = output.read(&mut pcm);
        if pcm[..count].iter().flatten().any(|sample| *sample != 0) {
            break;
        }
        assert_ne!(output.status(), Status::Failed);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "Original music did not become audible"
        );
        std::thread::yield_now();
    }
    let mut playback = recorded.effect_playback().unwrap().unwrap();
    let patterns = if std::env::var_os("GAMEPKG_RECORDED_DENSE_FULL_SAVE").is_some() {
        5041u64
    } else {
        0
    };
    for level in 1..=6 {
        assert_eq!(
            output.status(),
            Status::Playing,
            "music must remain active before dense scene {level}"
        );
        let expected = (patterns << 32) | 3000;
        assert_eq!(baseline.step(0x9f000000 + level).unwrap(), expected);
        assert_eq!(
            recorded.step(0x9f000000 + level).unwrap(),
            expected,
            "dense recorded scene {level}"
        );
        let a = baseline.frame();
        let b = recorded.frame();
        assert_eq!(a.background, b.background);
        assert_eq!(a.commands.len(), b.commands.len());
        if let Some(index) = a.commands.iter().zip(&b.commands).position(|(a, b)| a != b) {
            panic!("Scene {level} command {index} changed geometry, order, colors or image bytes");
        }
        assert!(b.commands.len() < gamepkg::drawing::MAX_COMMANDS);
        let image_bytes: usize = b
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Image { image, .. }
                | Command::LinearImage { image, .. }
                | Command::StraightLinearImage { image, .. }
                | Command::ShaderImage { image, .. } => Some(image.pixels().len()),
                _ => None,
            })
            .sum();
        assert!(image_bytes <= gamepkg::drawing::MAX_IMAGE_BYTES);
        let mut plays = 0;
        for command in delivered.lock().unwrap().drain(..) {
            match command {
                EffectCommand::Play(clip) => {
                    assert!(playback.effects().play(clip).is_some());
                    plays += 1;
                }
                EffectCommand::Volume(gain) => {
                    assert!(playback.effects().volume(gain));
                }
                EffectCommand::Pause(paused) => playback.effects().pause(paused),
                EffectCommand::Stop => playback.effects().stop(),
            }
        }
        assert!(plays > 0, "dense scene did not deliver its original effect");
        let mut audible = false;
        let start = Instant::now();
        while playback.effects().active() > 0 {
            audible |= playback.frame().unwrap().iter().any(|sample| *sample != 0);
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Effect did not finish through the real playback consumer"
            );
        }
        assert!(audible);
        assert_eq!(
            output.status(),
            Status::Playing,
            "music stopped during dense scene {level}"
        );
        assert!(recorded.audio_frame().is_empty());
        println!("Scene {level}: 200 blocks/3000 particles, {} exact draw commands, {image_bytes} image bytes, live recorded music/effect; native={native}",b.commands.len());
    }
}
