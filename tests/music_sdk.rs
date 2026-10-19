use gamepkg::{
    music::{Output, Status},
    runtime::Game,
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
fn drain(output: &Output, reference: &[u8]) -> Vec<u8> {
    let start = Instant::now();
    let mut pcm = Vec::new();
    loop {
        let mut block = [[0; 2]; 800];
        let count = output.read(&mut block);
        for frame in &block[..count] {
            for sample in frame {
                pcm.extend(sample.to_le_bytes());
            }
        }
        assert_ne!(output.status(), Status::Failed);
        if output.status() == Status::Ended {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        std::thread::yield_now();
    }
    assert_eq!(pcm.len(), reference.len());
    assert!(!pcm.is_empty());
    let mut audible = false;
    for (actual, expected) in pcm
        .as_chunks::<2>()
        .0
        .iter()
        .zip(reference.as_chunks::<2>().0.iter())
    {
        let actual = i16::from_le_bytes(*actual);
        let expected = i16::from_le_bytes(*expected) / 2;
        // Same original-mixer decoder comparison bound as the complete music corpus.
        assert!((i32::from(actual) - i32::from(expected)).unsigned_abs() <= 2);
        audible |= actual != 0;
    }
    assert!(audible);
    pcm
}
#[test]
#[ignore = "requires signed SDK fixtures from tools/check_music_sdk.py"]
fn signed_music_bindings_commit_and_rollback_across_desktop_backends() {
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_SDK_LIBRARY").unwrap());
    let reference =
        std::fs::read(std::env::var_os("GAMEPKG_MUSIC_SDK_REFERENCE").unwrap()).unwrap();
    let native = std::env::var_os("GAMEPKG_MUSIC_SDK_NATIVE").is_some();
    let mut first = None;
    for suffix in ["core", "component"] {
        for label in ["off", "on", "solo"] {
            let enabled = label != "off";
            let expected = if label == "on" {
                2057
            } else if enabled {
                1033
            } else {
                874
            };
            let id = format!("com.gamepkg.music-{suffix}-{}", label);
            let selected = Some(native && suffix == "core");
            if enabled {
                let config = std::path::PathBuf::from(std::env::var_os("GAMEPKG_CONFIG").unwrap());
                let original = std::fs::read_to_string(&config).unwrap();
                std::fs::write(&config, format!("{original}\nmemory_ceiling_mib=7\n")).unwrap();
                let refused = Game::load_target(&root, &id, &[], selected);
                std::fs::write(&config, original).unwrap();
                assert!(
                    refused.is_err(),
                    "audio reservations bypassed player memory ceiling: {id}"
                );
            }
            let mut game = Game::load_target(&root, &id, &[], selected).unwrap();
            let delivered = Arc::new(Mutex::new(0));
            let target = delivered.clone();
            game.attach_effect_output(Arc::new(move |_| {
                *target.lock().unwrap() += 1;
                true
            }));
            assert_eq!(game.step(0).unwrap(), expected);
            if enabled {
                let output = game.music_output().unwrap();
                let pcm = drain(&output, &reference);
                if let Some(first) = &first {
                    assert_eq!(&pcm, first);
                } else {
                    first = Some(pcm);
                }
                assert_eq!(game.step(3).unwrap(), 1);
                assert_eq!(game.step(5).unwrap(), 1);
                let start = Instant::now();
                while output.status() != Status::Failed {
                    assert!(start.elapsed() < Duration::from_secs(5));
                    std::thread::yield_now();
                }
                assert_eq!(game.step(4).unwrap(), 1);
                assert_eq!(game.step(0).unwrap(), expected);
                assert_eq!(drain(&output, &reference), *first.as_ref().unwrap());
                let played = output.played_frames();
                *delivered.lock().unwrap() = 0;
                assert!(game.step(1).is_err());
                assert_eq!(*delivered.lock().unwrap(), 0);
                assert_eq!(output.status(), Status::Ended);
                assert_eq!(output.played_frames(), played);
            } else {
                assert!(game.music_output().is_none());
            }
            drop(game);
            if enabled {
                for failure in ["save", "history", "output", "controls"] {
                    let mut game = Game::load_target(&root, &id, &[], selected).unwrap();
                    let emitted = Arc::new(Mutex::new(0));
                    let target = emitted.clone();
                    game.attach_effect_output(Arc::new(move |_| {
                        *target.lock().unwrap() += 1;
                        true
                    }));
                    assert_eq!(game.step(0).unwrap(), expected);
                    *emitted.lock().unwrap() = 0;
                    let output = game.music_output().unwrap();
                    assert_eq!(drain(&output, &reference), *first.as_ref().unwrap());
                    let played = output.played_frames();
                    let directory = match failure {
                        "save" => Some("saves"),
                        "history" => Some("progress"),
                        _ => None,
                    };
                    if let Some(directory) = directory {
                        std::fs::rename(
                            root.join(directory),
                            root.join(format!("old-{directory}")),
                        )
                        .unwrap();
                        std::fs::write(root.join(directory), b"blocked").unwrap();
                    } else if failure == "output" {
                        game.attach_effect_output(Arc::new(|_| false));
                    }
                    assert!(
                        game.step(if failure == "controls" { 6 } else { 2 })
                            .is_err(),
                        "{id}: {failure}"
                    );
                    assert_eq!(
                        *emitted.lock().unwrap(),
                        0,
                        "rejected music batch emitted effects: {id}: {failure}"
                    );
                    assert_eq!(output.status(), Status::Ended, "{id}: {failure}");
                    assert_eq!(output.played_frames(), played);
                    drop(game);
                    if let Some(directory) = directory {
                        std::fs::remove_file(root.join(directory)).unwrap();
                        std::fs::rename(
                            root.join(format!("old-{directory}")),
                            root.join(directory),
                        )
                        .unwrap();
                    }
                }
            }
            #[cfg(feature = "audio")]
            if enabled {
                let mut game = Game::load_target(&root, &id, &[], selected).unwrap();
                let audio = gamepkg::audio::Player::new(100).unwrap();
                assert!(audio.original_source(game.effect_playback().unwrap()));
                game.attach_effect_output(audio.effect_publisher());
                let output = game.music_output().unwrap();
                assert_eq!(game.step(0).unwrap(), expected);
                let start = Instant::now();
                while output.status() != Status::Ended {
                    assert_ne!(output.status(), Status::Failed);
                    assert!(start.elapsed() < Duration::from_secs(5));
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(output.played_frames() > 4096);
                assert!(audio.non_silent_frames() > 4096);
                std::thread::sleep(Duration::from_millis(20));
                let audible = audio.non_silent_frames();
                assert!(game.step(1).is_err());
                std::thread::sleep(Duration::from_millis(20));
                assert_eq!(
                    audio.non_silent_frames(),
                    audible,
                    "failed tick replayed device audio: {id}"
                );
                assert!(audio.error().is_none());
                println!("Signed music/effects device callback {id}: independent output clock, complete playback, no trap replay");
            }
            println!("Signed music SDK {id}, native={}: opt-in, full PCM, completion, trap/save/history/output rollback", native && suffix == "core");
        }
    }
}
