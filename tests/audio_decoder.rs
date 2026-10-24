#[path = "common/music_fixture.rs"]
mod music_fixture;
use gamepkg::audio_decode::{Factory, Output};
use music_fixture::signed_music_package;
fn hostile(body: &str, output: u32) -> Vec<u8> {
    hostile_pages(body, output, 1)
}
fn hostile_pages(body: &str, output: u32, pages: u32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
      (memory(export "memory") {pages} 256)
      (func(export "decoder_reserve")(param i32)(result i32)i32.const 1)
      (func(export "decoder_open")(result i32)i32.const 0)
      (func(export "decoder_read")(param i32)(result i32){body})
      (func(export "decoder_output")(result i32)i32.const {output})
      (func(export "decoder_rate")(result i32)i32.const 48000)
      (func(export "decoder_channels")(result i32)i32.const 2)
      (func(export "decoder_work")(result i32)i32.const 0)
      (func(export "decoder_rewind")(result i32)i32.const 0))"#
    ))
    .unwrap()
}
#[test]
fn exact_length_reader_loads_directly_in_bounded_chunks() {
    struct Counting<'a> {
        input: &'a [u8],
        calls: usize,
        largest: usize,
    }
    impl std::io::Read for Counting<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            self.largest = self.largest.max(output.len());
            self.input.read(output)
        }
    }
    let check = "i32.const 1 i32.load8_u i32.const 90 i32.ne
        i32.const 65537 i32.load8_u i32.const 90 i32.ne i32.or
        i32.const 131073 i32.load8_u i32.const 90 i32.ne i32.or
        if (result i32) i32.const -1 else i32.const 0 end";
    let factory = Factory::new(&hostile_pages(check, 1, 3)).unwrap();
    let bytes = vec![90; 131073];
    let mut input = Counting {
        input: &bytes,
        calls: 0,
        largest: 0,
    };
    let mut stream = factory
        .open_reader(&mut input, bytes.len(), Output::Native, 3 * 65536)
        .unwrap();
    assert_eq!(input.calls, 4);
    assert_eq!(input.largest, 65536);
    assert!(input.input.is_empty());
    assert_eq!(stream.memory_limit_bytes(), 3 * 65536);
    assert_eq!(stream.read(&mut [123; 1600]).unwrap(), 0);
}
#[test]
fn rejected_reader_requests_never_read_and_io_failures_do_not_poison_the_factory() {
    struct NeverRead;
    impl std::io::Read for NeverRead {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("unexpected input read")
        }
    }
    let factory = Factory::new(&hostile("i32.const 0", 1)).unwrap();
    for (length, output, memory) in [
        (0, Output::Native, 65536),
        (8 * 1048576 + 1, Output::Native, 65536),
        (1, Output::Native, 0),
        (1, Output::Native, 65535),
        (1, Output::Native, 16 * 1048576 + 65536),
        (1, Output::Stereo(0), 65536),
        (1, Output::OriginalMixer(96000), 65536),
        (65536, Output::Native, 65536), // reserve pointer + length exceeds memory
    ] {
        assert!(factory
            .open_reader(&mut NeverRead, length, output, memory)
            .is_err());
    }
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected read failure"))
        }
    }
    assert!(factory
        .open_reader(&mut Broken, 1, Output::Native, 65536)
        .is_err());
    assert!(factory
        .open_reader(&mut &b""[..], 1, Output::Native, 65536)
        .is_err());
    assert!(factory
        .open_reader(&mut &b"ab"[..], 1, Output::Native, 65536)
        .is_err());
    assert!(factory
        .open_reader(&mut &b"a"[..], 1, Output::Native, 65536)
        .is_ok());
}
#[test]
fn smaller_per_stream_memory_limit_rejects_growth_without_publishing_audio() {
    let body = "i32.const 1 memory.grow i32.const -1 i32.eq
        if (result i32) i32.const -1 else i32.const 0 end";
    let factory = Factory::new(&hostile(body, 1)).unwrap();
    let mut limited = factory
        .open_reader(&mut &b"a"[..], 1, Output::Native, 65536)
        .unwrap();
    let mut output = [123; 1600];
    assert!(limited.read(&mut output).is_err());
    assert_eq!(output, [123; 1600]);
    assert_eq!(limited.memory_bytes(), 65536);
    assert!(limited.rewind().is_err());
    let mut larger = factory
        .open_reader(&mut &b"a"[..], 1, Output::Native, 131072)
        .unwrap();
    assert_eq!(larger.read(&mut output).unwrap(), 0);
    assert_eq!(larger.memory_bytes(), 131072);
}
#[test]
fn decoder_and_guest_reservations_share_one_ceiling_and_failures_release_their_charge() {
    struct NeverRead;
    impl std::io::Read for NeverRead {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("over-budget input was read")
        }
    }
    let factory = Factory::new(&hostile("i32.const 0", 1)).unwrap();
    let budget = gamepkg::memory_budget::Budget::new(16 * 1048576);
    let guest = budget.reserve(10 * 1048576).unwrap();
    let stream = factory
        .open_budgeted(&mut &b"a"[..], 1, Output::Native, &budget, 6 * 1048576)
        .unwrap();
    assert_eq!(budget.used(), budget.limit());
    assert!(factory
        .open_budgeted(&mut NeverRead, 1, Output::Native, &budget, 65536)
        .is_err());
    drop(stream);
    assert_eq!(budget.used(), guest.bytes());
    assert!(factory
        .open_budgeted(&mut &b""[..], 1, Output::Native, &budget, 6 * 1048576)
        .is_err());
    assert_eq!(budget.used(), guest.bytes());
    let replacement = factory
        .open_budgeted(&mut &b"a"[..], 1, Output::Native, &budget, 6 * 1048576)
        .unwrap();
    drop(replacement);
    drop(guest);
    assert_eq!(budget.used(), 0);
}
#[test]
fn decoder_companions_cannot_import_capabilities_or_have_unbounded_memory() {
    assert!(
        Factory::new(&wat::parse_str(r#"(module(import "env" "escape"(func)))"#).unwrap()).is_err()
    );
    assert!(
        Factory::new(&wat::parse_str(r#"(module(memory(export "memory") 1))"#).unwrap()).is_err()
    );
    assert!(
        Factory::new(&wat::parse_str(r#"(module(memory(export "memory") 257 257))"#).unwrap())
            .is_err()
    );
}
#[test]
fn invalid_decoder_results_never_publish_partial_pcm_and_stop_the_stream() {
    for (body, pointer) in [
        ("i32.const -1", 1),
        ("i32.const 1602", 1),
        ("i32.const 1", 1),
        ("i32.const 1600", 65535),
    ] {
        let factory = Factory::new(&hostile(body, pointer)).unwrap();
        let mut stream = factory.open(b"test").unwrap();
        let mut output = [123i16; 1600];
        assert!(stream.read(&mut output).is_err());
        assert_eq!(output, [123; 1600]);
        assert!(stream.read(&mut output).is_err());
        assert!(stream.rewind().is_err());
    }
}
#[test]
fn infinite_decode_is_interrupted_without_instruction_fuel() {
    let factory = Factory::new(&hostile("(loop $forever br $forever) i32.const 0", 1)).unwrap();
    let mut stream = factory.open(b"test").unwrap();
    let begin = std::time::Instant::now();
    assert!(stream.read(&mut [0i16; 1600]).is_err());
    assert!(begin.elapsed() < std::time::Duration::from_secs(1));
}
fn companion() -> Factory {
    let path =
        std::env::var_os("GAMEPKG_AUDIO_DECODER_WASM").expect("set GAMEPKG_AUDIO_DECODER_WASM");
    Factory::new(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
#[ignore = "requires companion, GAMEPKG_AUDIO_MONO_INPUT and GAMEPKG_SDL_RESAMPLE_REFERENCE"]
fn mono_original_mixer_matches_sdl_conversion_and_rewinds_with_equal_channels() {
    let factory = companion();
    let input = std::fs::read(std::env::var_os("GAMEPKG_AUDIO_MONO_INPUT").unwrap()).unwrap();
    let mut native = factory.open(&input).unwrap();
    assert_eq!(native.channels(), 1);
    let source_rate = native.sample_rate();
    if let Ok(expected) = std::env::var("GAMEPKG_AUDIO_MONO_RATE") {
        assert_eq!(
            source_rate,
            expected.parse::<u32>().unwrap(),
            "Mono source-rate metadata"
        );
    }
    let mut buffer = [0i16; 1600];
    let mut pcm = Vec::new();
    loop {
        let count = native.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        pcm.extend_from_slice(&buffer[..count]);
        assert!(pcm.len() <= 32768);
    }
    assert!(pcm.iter().any(|sample| *sample != 0));
    drop(native);
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        pcm.iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for rate in [44100, 48000] {
        let original =
            std::process::Command::new(std::env::var_os("GAMEPKG_SDL_RESAMPLE_REFERENCE").unwrap())
                .arg(file.path())
                .arg(source_rate.to_string())
                .arg(rate.to_string())
                .args(["stream", "1", "s16"])
                .output()
                .unwrap();
        assert!(
            original.status.success(),
            "{}",
            String::from_utf8_lossy(&original.stderr)
        );
        let mut stream = factory
            .open_original_mixer(&input, rate)
            .expect("Mono original-mixer conversion must be supported");
        assert_eq!(stream.channels(), 2);
        assert_eq!(stream.sample_rate(), rate);
        let mut expected = None;
        for pass in 0..2 {
            let mut actual = Vec::new();
            let mut step = 0;
            loop {
                let size = if pass == 0 {
                    [2, 6, 32, 1598, 1600][step % 5]
                } else {
                    1600
                };
                step += 1;
                let count = stream.read(&mut buffer[..size]).unwrap();
                if count == 0 {
                    break;
                }
                assert!(count.is_multiple_of(2));
                assert!(buffer[..count]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|pair| pair[0] == pair[1]));
                actual.extend(
                    buffer[..count]
                        .iter()
                        .flat_map(|sample| sample.to_le_bytes()),
                );
                assert!(actual.len() <= 262144);
                assert!(stream.memory_bytes() <= 6 * 1048576);
            }
            assert_eq!(
                actual, original.stdout,
                "Mono conversion differs from actual SDL at {rate} Hz, pass {pass}"
            );
            if let Some(previous) = &expected {
                assert_eq!(&actual, previous);
            } else {
                expected = Some(actual);
                stream.rewind().unwrap();
            }
        }
    }
}
fn open_music(factory: &Factory, input: &[u8]) -> anyhow::Result<gamepkg::audio_decode::Stream> {
    open_music_reader(factory, &mut std::io::Cursor::new(input), input.len())
}
fn open_music_reader<R: std::io::Read + ?Sized>(
    factory: &Factory,
    input: &mut R,
    length: usize,
) -> anyhow::Result<gamepkg::audio_decode::Stream> {
    let memory = std::env::var("GAMEPKG_AUDIO_MEMORY_BYTES")
        .map(|v| v.parse::<usize>().unwrap())
        .unwrap_or(16 * 1048576);
    assert!(
        factory.maximum_memory_bytes() <= memory,
        "companion declaration exceeds selected budget"
    );
    let output = if let Ok(rate) = std::env::var("GAMEPKG_AUDIO_OUTPUT_RATE") {
        if std::env::var_os("GAMEPKG_AUDIO_ORIGINAL_MIXER").is_some() {
            Output::OriginalMixer(rate.parse()?)
        } else {
            Output::Stereo(rate.parse()?)
        }
    } else {
        Output::Native
    };
    let budget = gamepkg::memory_budget::Budget::new(memory as u64);
    factory.open_budgeted(input, length, output, &budget, memory)
}

#[test]
#[ignore = "requires built companion and GAMEPKG_MUSIC_INPUT_DIR"]
fn original_tracks_stream_with_fixed_memory_chunk_invariance_and_rewind() {
    use sha2::{Digest, Sha256};
    let factory = companion();
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_INPUT_DIR").unwrap());
    let mut paths = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "mp3"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());
    let mut packaged =
        std::env::var_os("GAMEPKG_AUDIO_PACKAGE_READERS").map(|_| signed_music_package(&paths));
    for path in paths {
        let mut stream = if let Some((_, package)) = &mut packaged {
            let name = format!("music/{}", path.file_name().unwrap().to_str().unwrap());
            package
                .with_asset_reader(&name, 8 * 1048576, |input, length| {
                    open_music_reader(&factory, input, length)
                })
                .unwrap()
        } else {
            let mut input = std::fs::File::open(&path).unwrap();
            let length = usize::try_from(input.metadata().unwrap().len()).unwrap();
            open_music_reader(&factory, &mut input, length).unwrap()
        };
        let mut memory_peak = stream.memory_bytes();
        let mut samples = 0u64;
        let mut fnv = 2166136261u32;
        let mut witness: Option<(u64, Vec<i16>)> = None;
        let mut digest = Sha256::new();
        let mut audible = false;
        let mut stereo = false;
        let mut reference = std::env::var_os("GAMEPKG_AUDIO_REFERENCE_ROOT").map(|root| {
            std::fs::File::open(
                std::path::PathBuf::from(root)
                    .join(path.file_name().unwrap())
                    .with_extension("s16"),
            )
            .unwrap()
        });
        let reference_samples = reference.as_ref().map(|f| f.metadata().unwrap().len() / 2);
        let mut reference_at = 0u64;
        let mut different = 0u64;
        let mut maximum_delta = 0u32;
        let mut output = [0i16; 1600];
        loop {
            let n = stream.read(&mut output).unwrap();
            if n == 0 {
                break;
            }
            if let Some(file) = &mut reference {
                use std::io::Read;
                let count = (reference_samples.unwrap() - reference_at).min(n as u64) as usize;
                let mut original = vec![0u8; count * 2];
                file.read_exact(&mut original).unwrap();
                for (sample, bytes) in output[..count]
                    .iter()
                    .zip(original.as_chunks::<2>().0.iter())
                {
                    let delta = (i32::from(*sample)
                        - i32::from(i16::from_le_bytes([bytes[0], bytes[1]])))
                    .unsigned_abs();
                    different += u64::from(delta != 0);
                    maximum_delta = maximum_delta.max(delta);
                }
                reference_at += count as u64;
            }
            for sample in &output[..n] {
                let bytes = sample.to_le_bytes();
                digest.update(bytes);
                for byte in bytes {
                    fnv = (fnv ^ u32::from(byte)).wrapping_mul(16777619);
                }
            }
            if witness.is_none() && output[..n].iter().any(|v| i32::from(*v).abs() > 512) {
                witness = Some((samples, output[..n].to_vec()));
            }
            audible |= output[..n].iter().any(|v| *v != 0);
            stereo |= stream.channels() == 2
                && output[..n].as_chunks::<2>().0.iter().any(|p| p[0] != p[1]);
            samples += n as u64;
            assert!(samples < 1_000_000_000);
            memory_peak = memory_peak.max(stream.memory_bytes());
            assert!(memory_peak <= stream.memory_limit_bytes());
        }
        assert!(audible);
        assert!(stream.channels() == 1 || stereo);
        if let Some(expected) = reference_samples {
            assert_eq!(samples, expected, "SDL decoded length: {}", path.display());
            assert_eq!(
                reference_at, expected,
                "SDL reference was not fully compared"
            );
        }
        let first_hash = hex::encode(digest.finalize());
        stream.rewind().unwrap();
        let mut second_digest = Sha256::new();
        let mut second_count = 0u64;
        let chunks = [2usize, 6, 32, 1598, 1600];
        let mut next = 0;
        loop {
            let count = chunks[next % chunks.len()];
            next += 1;
            let n = stream.read(&mut output[..count]).unwrap();
            if n == 0 {
                break;
            }
            for sample in &output[..n] {
                second_digest.update(sample.to_le_bytes());
            }
            second_count += n as u64;
            memory_peak = memory_peak.max(stream.memory_bytes());
            assert!(memory_peak <= stream.memory_limit_bytes());
        }
        assert_eq!(second_count, samples);
        assert_eq!(hex::encode(second_digest.finalize()), first_hash);
        let (witness_offset, witness) = witness.expect("recording has no audible witness");
        println!(
            "MUSIC_JSON {}",
            serde_json::json!({"memory_budget":stream.memory_limit_bytes(),"memory_peak":memory_peak,"original_mixer":std::env::var_os("GAMEPKG_AUDIO_ORIGINAL_MIXER").is_some(),"output_rate":std::env::var("GAMEPKG_AUDIO_OUTPUT_RATE").ok().map(|v| v.parse::<u32>().unwrap()),"name":path.file_name().unwrap().to_string_lossy(),"rate":stream.sample_rate(),"channels":stream.channels(),"samples":samples,"sha256":first_hash,"fnv32":fnv,"sdl_samples":reference_samples,"sdl_different":different,"sdl_maximum_delta":maximum_delta,"witness_offset":witness_offset,"witness":witness})
        );
    }
}
#[test]
#[ignore = "requires built companion and GAMEPKG_MUSIC_INPUT_DIR"]
fn malformed_music_and_buffer_requests_fail_bounded_without_changing_output() {
    let factory = companion();
    if let Some(path) = std::env::var_os("GAMEPKG_AUDIO_MONO_INPUT") {
        let input = std::fs::read(path).unwrap();
        let mut native = factory.open(&input).unwrap();
        assert_eq!(native.channels(), 1);
        assert!(native.read(&mut [0; 1600]).unwrap() > 0);
        for rate in [44100, 48000] {
            for original_mixer in [false, true] {
                let mut converted = if original_mixer {
                    factory.open_original_mixer(&input, rate)
                } else {
                    factory.open_resampled(&input, rate)
                }
                .unwrap();
                assert_eq!(converted.channels(), 2);
                let mut output = [0; 1600];
                let mut heard = false;
                let mut samples = 0u64;
                let mut fnv = 2166136261u32;
                loop {
                    let count = converted.read(&mut output).unwrap();
                    if count == 0 {
                        break;
                    }
                    heard |= output[..count].iter().any(|sample| *sample != 0);
                    samples += count as u64;
                    for byte in output[..count]
                        .iter()
                        .flat_map(|sample| sample.to_le_bytes())
                    {
                        fnv = (fnv ^ u32::from(byte)).wrapping_mul(16777619);
                    }
                    assert!(output[..count]
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .all(|pair| pair[0] == pair[1]));
                }
                assert!(heard, "mono conversion must include audible PCM");
                println!(
                    "MONO_JSON {}",
                    serde_json::json!({"rate":rate,"original_mixer":original_mixer,"samples":samples,"fnv32":fnv})
                );
            }
        }
    }
    assert!(open_music(&factory, &[]).is_err());
    assert!(open_music(&factory, &vec![0; 8 * 1024 * 1024 + 1]).is_err());
    for bytes in [
        vec![0; 4096],
        vec![255; 4096],
        b"ID3\x04\0\0\x7f\x7f\x7f\x7f".to_vec(),
        vec![0; 8 * 1024 * 1024],
    ] {
        assert!(open_music(&factory, &bytes).is_err());
    }
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_MUSIC_INPUT_DIR").unwrap());
    let path = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|s| s == "mp3"))
        .unwrap();
    let bytes = std::fs::read(path).unwrap();
    let mut stream = open_music(&factory, &bytes).unwrap();
    let mut bad = [123i16; 1602];
    assert!(stream.read(&mut bad).is_err());
    assert!(bad.iter().all(|v| *v == 123));
    if stream.channels() == 2 {
        assert!(stream.read(&mut bad[..1]).is_err());
    }
    assert!(stream.read(&mut bad[..1600]).unwrap() > 0);
    for n in [1usize, 2, 3, 4, 8, 10, 31, 127, 511, 1023, 2047, 4095] {
        if let Ok(mut stream) = open_music(&factory, &bytes[..n.min(bytes.len())]) {
            for _ in 0..32 {
                if stream.read(&mut bad[..1600]).unwrap_or(0) == 0 {
                    break;
                }
            }
        }
    }
}
