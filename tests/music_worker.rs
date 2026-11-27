#[path = "common/music_fixture.rs"]
mod music_fixture;
use gamepkg::{
    audio_decode::Factory,
    memory_budget::Budget,
    music::{Service, Status, HOST_BYTES},
};
fn decoder_factory() -> Factory {
    if std::env::var_os("GAMEPKG_MUSIC_USE_BUNDLED").is_some() {
        Factory::bundled().unwrap()
    } else {
        let wasm = std::fs::read(std::env::var_os("GAMEPKG_AUDIO_DECODER_WASM").unwrap()).unwrap();
        if let Some(millis) = std::env::var_os("GAMEPKG_AUDIO_QUALIFICATION_DEADLINE_MS") {
            let millis = millis.to_str().unwrap().parse::<u64>().unwrap();
            Factory::new_with_operation_deadline(&wasm, std::time::Duration::from_millis(millis))
                .unwrap()
        } else {
            Factory::new(&wasm).unwrap()
        }
    }
}
#[test]
#[ignore = "requires mono fixture, decoder and GAMEPKG_SDL_RESAMPLE_REFERENCE"]
fn mono_signed_package_worker_controls_and_full_output_match_sdl() {
    let path = std::path::PathBuf::from(std::env::var_os("GAMEPKG_AUDIO_MONO_INPUT").unwrap());
    let factory = decoder_factory();
    assert!(factory.maximum_memory_bytes() <= 6 * 1048576);
    let mut native = factory.open(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(native.channels(), 1);
    let source_rate = native.sample_rate();
    let mut decoded = Vec::new();
    let mut block = [0i16; 1600];
    loop {
        let count = native.read(&mut block).unwrap();
        if count == 0 {
            break;
        }
        decoded.extend(
            block[..count]
                .iter()
                .flat_map(|sample| sample.to_le_bytes()),
        );
        assert!(decoded.len() <= 65536);
    }
    drop(native);
    let temporary = tempfile::tempdir().unwrap();
    let pcm = temporary.path().join("source.s16");
    std::fs::write(&pcm, decoded).unwrap();
    let oracle = std::env::var_os("GAMEPKG_SDL_RESAMPLE_REFERENCE").unwrap();
    let original = std::process::Command::new(&oracle)
        .arg(&pcm)
        .arg(source_rate.to_string())
        .args(["44100", "stream", "1", "s16"])
        .output()
        .unwrap();
    assert!(original.status.success());
    assert!(original
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .any(|sample| *sample != [0, 0]));
    let mixer = temporary.path().join("mixer.s16");
    std::fs::write(&mixer, &original.stdout).unwrap();
    let (_package_root, package) = music_fixture::signed_music_package(std::slice::from_ref(&path));
    let name = format!("music/{}", path.file_name().unwrap().to_str().unwrap());
    for rate in [44100, 48000] {
        let expected = if rate == 44100 {
            original.stdout.clone()
        } else {
            let converted = std::process::Command::new(&oracle)
                .arg(&mixer)
                .args(["44100", "48000", "cvt", "2", "s16"])
                .output()
                .unwrap();
            assert!(converted.status.success());
            converted.stdout
        };
        let budget = Budget::new(16 * 1048576);
        let guest = budget
            .reserve(budget.limit() - 6 * 1048576 - HOST_BYTES)
            .unwrap();
        assert!(Service::new(
            factory.clone(),
            &package,
            false,
            budget.clone(),
            6 * 1048576
        )
        .is_err());
        assert_eq!(budget.used(), guest.bytes());
        let service = if rate == 44100 {
            Service::new_original_rate(factory.clone(), &package, true, budget.clone(), 6 * 1048576)
        } else {
            Service::new(factory.clone(), &package, true, budget.clone(), 6 * 1048576)
        }
        .unwrap();
        let mut service = gamepkg::music::controls::Transaction::new(service);
        let output = service.output();
        service.begin().unwrap();
        service.play(&name).unwrap();
        service.finish(false).unwrap();
        assert_eq!(output.status(), Status::Stopped);
        for gain in [128, 0, 128] {
            service.begin().unwrap();
            service.play(&name).unwrap();
            service.volume(gain).unwrap();
            service.pause(true).unwrap();
            service.finish(true).unwrap();
            let mut sentinel = [[123, -456]; 1];
            assert_eq!(output.read(&mut sentinel), 0);
            assert_eq!(sentinel, [[123, -456]; 1]);
            service.begin().unwrap();
            service.pause(false).unwrap();
            service.finish(true).unwrap();
            let started = std::time::Instant::now();
            let mut actual = Vec::new();
            let mut block = [[0i16; 2]; 800];
            let mut step = 0;
            loop {
                let count = output.read(&mut block[..[1, 113, 7, 799, 800][step % 5]]);
                step += 1;
                actual.extend(
                    block[..count]
                        .iter()
                        .flatten()
                        .flat_map(|sample| sample.to_le_bytes()),
                );
                assert!(actual.len() <= expected.len());
                assert!(budget.used() <= budget.limit());
                assert_ne!(
                    output.status(),
                    Status::Failed,
                    "{}",
                    service.error().unwrap_or_default()
                );
                if output.status() == Status::Ended {
                    break;
                }
                assert!(
                    started.elapsed() < std::time::Duration::from_secs(10),
                    "Mono output worker stalled"
                );
                if count == 0 {
                    std::thread::sleep(std::time::Duration::from_micros(100));
                }
            }
            assert_eq!(actual.len(), expected.len());
            assert_eq!(output.played_frames() * 4, actual.len() as u64);
            if gain == 0 {
                assert!(actual.iter().all(|byte| *byte == 0));
            } else {
                assert_eq!(actual, expected, "Original mono worker PCM at {rate} Hz");
            }
            service.begin().unwrap();
            service.stop().unwrap();
            service.finish(true).unwrap();
            assert_eq!(output.status(), Status::Stopped);
            assert_eq!(output.read(&mut sentinel), 0);
            assert_eq!(sentinel, [[123, -456]; 1]);
        }
        drop(service);
        assert_eq!(budget.used(), guest.bytes() + HOST_BYTES);
        drop(output);
        assert_eq!(budget.used(), guest.bytes());
        drop(guest);
        assert_eq!(budget.used(), 0);
    }
}
#[cfg(all(feature = "audio", target_os = "linux"))]
#[test]
#[ignore = "requires generated mono input, decoder and private ALSA null configuration"]
fn mono_actual_device_callback_controls_both_worker_rates_without_game_ticks() {
    assert!(std::env::var_os("ALSA_CONFIG_PATH").is_some());
    let path = std::path::PathBuf::from(std::env::var_os("GAMEPKG_AUDIO_MONO_INPUT").unwrap());
    let factory = decoder_factory();
    assert!(factory.maximum_memory_bytes() <= 6 * 1048576);
    let input = factory.open(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(input.channels(), 1);
    if let Ok(rate) = std::env::var("GAMEPKG_AUDIO_MONO_RATE") {
        assert_eq!(input.sample_rate(), rate.parse::<u32>().unwrap());
    }
    drop(input);
    let (_temporary, package) = music_fixture::signed_music_package(std::slice::from_ref(&path));
    let name = format!("music/{}", path.file_name().unwrap().to_str().unwrap());
    for rate in [44100, 48000] {
        let budget = Budget::new(16 * 1048576);
        let guest = budget.reserve(9 * 1048576).unwrap();
        let service = if rate == 44100 {
            Service::new_original_rate(factory.clone(), &package, true, budget.clone(), 6 * 1048576)
        } else {
            Service::new(factory.clone(), &package, true, budget.clone(), 6 * 1048576)
        }
        .unwrap();
        let output = service.output();
        for master in [0, 100] {
            let player = gamepkg::audio::Player::new(master).unwrap();
            if rate == 44100 {
                assert!(!player.music_source(Some(output.clone())));
                let playback =
                    gamepkg::audio_mix_stream::Playback::new(output.clone(), &budget).unwrap();
                assert!(player.original_source(Some(playback)));
            } else {
                assert!(player.music_source(Some(output.clone())));
            }
            service
                .batch(&[
                    gamepkg::music::Command::Play(&name),
                    gamepkg::music::Command::Pause(true),
                ])
                .unwrap();
            let start = std::time::Instant::now();
            while output.played_frames() != 0 {
                assert!(start.elapsed() < std::time::Duration::from_secs(5));
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            assert_eq!(output.status(), Status::Paused);
            std::thread::sleep(std::time::Duration::from_millis(10));
            assert_eq!(output.played_frames(), 0);
            service.pause(false).unwrap();
            let start = std::time::Instant::now();
            while output.played_frames() < 4800 || (master != 0 && player.non_silent_frames() == 0)
            {
                assert_ne!(output.status(), Status::Failed, "{:?}", service.error());
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(5),
                    "Mono callback did not advance"
                );
                assert!(budget.used() <= budget.limit());
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            if master == 0 {
                assert_eq!(player.non_silent_frames(), 0);
            }
            service.pause(true).unwrap();
            assert_eq!(output.status(), Status::Paused);
            let paused = output.played_frames();
            std::thread::sleep(std::time::Duration::from_millis(10));
            assert_eq!(output.played_frames(), paused);
            service.pause(false).unwrap();
            let start = std::time::Instant::now();
            while output.played_frames() == paused {
                assert_ne!(output.status(), Status::Failed, "{:?}", service.error());
                assert!(
                    start.elapsed() < std::time::Duration::from_secs(5),
                    "Mono callback did not resume"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            service.stop().unwrap();
            assert_eq!(output.status(), Status::Stopped);
            if rate == 44100 {
                assert!(player.original_source(None));
            } else {
                assert!(player.music_source(None));
            }
            assert!(player.error().is_none());
            drop(player);
        }
        drop(service);
        assert_eq!(budget.used(), guest.bytes() + HOST_BYTES);
        drop(output);
        assert_eq!(budget.used(), guest.bytes());
        drop(guest);
        assert_eq!(budget.used(), 0);
    }
}
#[cfg(feature = "audio")]
#[test]
#[ignore = "requires the private ALSA null configuration from tools/check_music_worker.py --null-device"]
fn actual_device_callback_drives_music_while_the_game_thread_is_idle() {
    assert!(std::env::var_os("ALSA_CONFIG_PATH").is_some());
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_INPUT_DIR").unwrap());
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|extension| extension == "mp3"))
        .collect::<Vec<_>>();
    paths.sort();
    let paths = &paths[..1];
    let (_temporary, package) = music_fixture::signed_music_package(paths);
    let factory = decoder_factory();
    let budget = Budget::new(16 * 1048576);
    let guest = budget
        .reserve(budget.limit() - 6 * 1048576 - HOST_BYTES)
        .unwrap();
    let service = Service::new(factory, &package, true, budget.clone(), 6 * 1048576).unwrap();
    let output = service.output();
    let muted = gamepkg::audio::Player::new(0).unwrap();
    assert!(muted.music_source(Some(output.clone())));
    service
        .play(&format!(
            "music/{}",
            paths[0].file_name().unwrap().to_str().unwrap()
        ))
        .unwrap();
    let start = std::time::Instant::now();
    while output.played_frames() < 4800 {
        assert_ne!(
            output.status(),
            Status::Failed,
            "{}",
            service.error().unwrap_or_default()
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(muted.non_silent_frames(), 0);
    assert!(muted.error().is_none());
    assert!(muted.music_source(None));
    drop(muted);
    let player = gamepkg::audio::Player::new(100).unwrap();
    assert!(player.music_source(Some(output.clone())));
    let start = std::time::Instant::now();
    while player.non_silent_frames() == 0 {
        assert_ne!(
            output.status(),
            Status::Failed,
            "{}",
            service.error().unwrap_or_default()
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    service.pause(true).unwrap();
    let paused = output.played_frames();
    std::thread::sleep(std::time::Duration::from_millis(10));
    assert_eq!(output.played_frames(), paused);
    service.pause(false).unwrap();
    let start = std::time::Instant::now();
    while output.played_frames() == paused {
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(player.error().is_none());
    assert!(player.music_source(None));
    drop(player);
    drop(service);
    assert_eq!(budget.used(), guest.bytes() + HOST_BYTES);
    drop(output);
    assert_eq!(budget.used(), guest.bytes());
    drop(guest);
    assert_eq!(budget.used(), 0);
    println!("MUSIC_CALLBACK_OK");
}
#[cfg(feature = "audio")]
#[test]
#[ignore = "requires original music, decoder and private ALSA null configuration from tools/check_music_worker.py --null-device"]
fn combined_original_rate_music_and_effects_reach_the_device_without_game_ticks() {
    assert!(std::env::var_os("ALSA_CONFIG_PATH").is_some());
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_INPUT_DIR").unwrap());
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mp3"))
        .collect::<Vec<_>>();
    paths.sort();
    let effect_root = tempfile::tempdir().unwrap();
    let effect_path = effect_root.path().join("effect.wav");
    let mut wav = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\0\0\0\0".to_vec();
    wav[4..8].copy_from_slice(&64036u32.to_le_bytes());
    wav[40..44].copy_from_slice(&64000u32.to_le_bytes());
    for _ in 0..32000 {
        wav.extend_from_slice(&20000i16.to_le_bytes());
    }
    std::fs::write(&effect_path, wav).unwrap();
    let (_temporary, package) =
        music_fixture::signed_music_package(&[paths[0].clone(), effect_path]);
    let factory = decoder_factory();
    let budget = Budget::new(16 * 1048576);
    let guest = budget.reserve(9 * 1048576).unwrap();
    let service =
        Service::new_original_rate(factory, &package, true, budget.clone(), 6 * 1048576).unwrap();
    let output = service.output();
    assert_eq!(output.sample_rate(), 44100);
    let player = gamepkg::audio::Player::new(100).unwrap();
    assert!(
        !player.music_source(Some(output.clone())),
        "wrong-rate attachment must fail"
    );
    let playback = gamepkg::audio_mix_stream::Playback::new(output.clone(), &budget).unwrap();
    assert!(player.original_source(Some(playback)));
    assert!(
        !player.submit_stereo(&[123, -456]),
        "premixed audio must not be silently dropped"
    );
    service
        .play(&format!(
            "music/{}",
            paths[0].file_name().unwrap().to_str().unwrap()
        ))
        .unwrap();
    let start = std::time::Instant::now();
    while output.played_frames() < 4800 || player.non_silent_frames() == 0 {
        assert_ne!(output.status(), Status::Failed, "{:?}", service.error());
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Effects have their own output-clock lifetime even with music stopped.
    service.stop().unwrap();
    use gamepkg::audio::EffectCommand;
    assert!(!player.effect_volume(129));
    let effects = budget.partition(128 * 1024).unwrap();
    let mut transaction =
        gamepkg::audio_effects::Service::new(package, true, effects.clone()).unwrap();
    transaction.begin().unwrap();
    transaction.volume(0).unwrap();
    transaction.pause(true).unwrap();
    transaction.play("music/effect.wav").unwrap();
    assert!(transaction.finish(true, |commands| player.effect_batch(commands)));
    drop(effects); // Pending/playing clips must retain the parent's cache charge.
    std::thread::sleep(std::time::Duration::from_millis(20));
    let silent = player.non_silent_frames();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(player.non_silent_frames(), silent);
    transaction.begin().unwrap();
    transaction.volume(128).unwrap();
    transaction.pause(false).unwrap();
    transaction.play("music/effect.wav").unwrap();
    assert!(!transaction.finish(false, |_| panic!("failed tick reached the audio device")));
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(player.non_silent_frames(), silent);
    drop(transaction); // The output retains PCM after the cache closes.
                       // A malformed batch must not partially stop the retained voice. Resuming
                       // it below proves the Stop was never applied to the real callback.
    assert!(!player.effect_batch(&[EffectCommand::Stop, EffectCommand::Volume(129)]));
    assert!(player.effect_batch(&[EffectCommand::Volume(128), EffectCommand::Pause(false)]));
    let start = std::time::Instant::now();
    while player.non_silent_frames() == silent {
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(player.error().is_none());
    assert!(player.effect_stop());
    assert!(player.original_source(None));
    assert!(!player.effect_volume(128));
    drop(player);
    drop(service);
    drop(output);
    assert_eq!(budget.used(), guest.bytes());
    drop(guest);
    assert_eq!(budget.used(), 0);
    println!("ORIGINAL_MIX_CALLBACK_OK");
    println!("EFFECT_TRANSACTION_CALLBACK_OK");
}
