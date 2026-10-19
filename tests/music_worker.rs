#[path = "common/music_fixture.rs"]
mod music_fixture;
use gamepkg::{
    audio_decode::Factory,
    memory_budget::Budget,
    music::{Service, Status, HOST_BYTES},
};
use std::{
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
fn decoder_factory() -> Factory {
    if std::env::var_os("GAMEPKG_MUSIC_USE_BUNDLED").is_some() {
        Factory::bundled().unwrap()
    } else {
        Factory::new(
            &std::fs::read(std::env::var_os("GAMEPKG_AUDIO_DECODER_WASM").unwrap()).unwrap(),
        )
        .unwrap()
    }
}
struct Drawing {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<u64>>,
}
impl Drop for Drawing {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            assert!(thread.join().unwrap() > 0);
        }
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
#[test]
#[ignore = "requires built decoder, original music and SDL references from tools/check_music_worker.py"]
fn original_tracks_play_from_signed_package_on_the_output_clock_without_game_ticks() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_INPUT_DIR").unwrap());
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|extension| extension == "mp3"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-music-worker-reference.json")).unwrap();
    let cases = golden["cases"].as_array().unwrap();
    assert_eq!(
        paths.len(),
        cases.len(),
        "the complete pinned original corpus is required"
    );
    let factory = decoder_factory();
    assert!(factory.maximum_memory_bytes() <= 6 * 1048576);
    let (_temporary, package) = music_fixture::signed_music_package(&paths);
    let budget = Budget::new(16 * 1048576);
    let guest = budget
        .reserve(budget.limit() - 6 * 1048576 - HOST_BYTES)
        .unwrap();
    let mut service = gamepkg::music::controls::Transaction::new(
        Service::new(factory, &package, true, budget.clone(), 6 * 1048576).unwrap(),
    );
    let output = service.output();
    let mut rendering = package.try_clone_verified().unwrap();
    let expected_hello = package.entries["assets/hello.txt"];
    let expected = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal/assets/hello.txt"),
    )
    .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut count = 0;
        while !flag.load(Ordering::Relaxed) {
            assert_eq!(
                rendering
                    .asset_chunk("hello.txt", 0, expected_hello as usize)
                    .unwrap(),
                expected
            );
            count += 1;
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        count
    });
    let drawing = Drawing {
        stop,
        thread: Some(thread),
    };
    drop(package);
    for path in paths {
        let name = path.file_name().unwrap().to_str().unwrap();
        let expected = cases
            .iter()
            .find(|case| case["name"].as_str() == Some(name))
            .expect("unknown music corpus entry");
        let mut input = std::fs::File::open(&path).unwrap();
        let mut input_hash = Sha256::new();
        let mut input_block = [0u8; 65536];
        loop {
            let count = input.read(&mut input_block).unwrap();
            if count == 0 {
                break;
            }
            input_hash.update(&input_block[..count]);
        }
        assert_eq!(
            hex::encode(input_hash.finalize()),
            expected["input_sha256"].as_str().unwrap()
        );
        let reference =
            std::path::PathBuf::from(std::env::var_os("GAMEPKG_AUDIO_REFERENCE_ROOT").unwrap())
                .join(name)
                .with_extension("s16");
        let mut reference = std::fs::File::open(reference).unwrap();
        let expected_samples = reference.metadata().unwrap().len() / 2;
        let previous = output.status();
        service.begin().unwrap();
        service.play("music/not-published.mp3").unwrap();
        service.volume(0).unwrap();
        service.finish(false).unwrap();
        assert_eq!(output.status(), previous);
        service.begin().unwrap();
        service.stop().unwrap();
        service.play(&format!("music/{name}")).unwrap();
        service.volume(128).unwrap();
        service.pause(true).unwrap();
        service.finish(true).unwrap();
        let mut sentinel = [[123, -456]; 1];
        assert_eq!(output.read(&mut sentinel), 0);
        assert_eq!(sentinel, [[123, -456]; 1]);
        service.begin().unwrap();
        service.pause(false).unwrap();
        service.finish(true).unwrap();
        let mut samples = 0u64;
        let mut hash = Sha256::new();
        let mut different = 0u64;
        let mut maximum_delta = 0u32;
        let mut audible = false;
        let mut stereo = false;
        let mut block = [[0i16; 2]; 800];
        let mut original = [0u8; 3200];
        let mut step = 0;
        let start = std::time::Instant::now();
        loop {
            let count = output.read(&mut block[..[1, 113, 7, 799, 800][step % 5]]);
            step += 1;
            if count > 0 {
                reference.read_exact(&mut original[..count * 4]).unwrap();
                for (sample, bytes) in block[..count]
                    .iter()
                    .flatten()
                    .zip(original.as_chunks::<2>().0.iter())
                {
                    hash.update(sample.to_le_bytes());
                    let expected = i16::from_le_bytes([bytes[0], bytes[1]]);
                    let delta = (i32::from(*sample) - i32::from(expected)).unsigned_abs();
                    different += u64::from(delta != 0);
                    maximum_delta = maximum_delta.max(delta);
                    audible |= *sample != 0;
                }
                stereo |= block[..count].iter().any(|frame| frame[0] != frame[1]);
                samples += count as u64 * 2;
            }
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
                start.elapsed() < std::time::Duration::from_secs(10),
                "worker stalled on {name}"
            );
            if count == 0 {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }
        assert_eq!(
            samples, expected_samples,
            "complete output-clock length: {name}"
        );
        assert_eq!(output.played_frames() * 2, samples);
        assert!(audible && stereo);
        assert_eq!(reference.read(&mut [0]).unwrap(), 0);
        let actual_hash = hex::encode(hash.finalize());
        assert_eq!(samples, expected["samples"].as_u64().unwrap());
        assert_eq!(
            actual_hash,
            expected["sha256"].as_str().unwrap(),
            "complete output PCM: {name}"
        );
        println!(
            "MUSIC_WORKER_JSON {}",
            serde_json::json!({"name":name,"samples":samples,
                "sha256":actual_hash,"sdl_different":different,"sdl_maximum_delta":maximum_delta})
        );
    }
    drop(drawing);
    drop(service);
    assert_eq!(budget.used(), guest.bytes() + HOST_BYTES);
    drop(output);
    assert_eq!(budget.used(), guest.bytes());
    drop(guest);
    assert_eq!(budget.used(), 0);
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
