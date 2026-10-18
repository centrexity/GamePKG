use gamepkg::audio_decode::Factory;
fn hostile(body: &str, output: u32) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module
      (memory(export "memory") 1 256)
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
fn open_music(factory: &Factory, input: &[u8]) -> anyhow::Result<gamepkg::audio_decode::Stream> {
    if let Ok(rate) = std::env::var("GAMEPKG_AUDIO_OUTPUT_RATE") {
        factory.open_resampled(input, rate.parse()?)
    } else {
        factory.open(input)
    }
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
    for path in paths {
        let input = std::fs::read(&path).unwrap();
        let mut stream = open_music(&factory, &input).unwrap();
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
                for (sample, bytes) in output[..count].iter().zip(original.chunks_exact(2)) {
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
            stereo |= stream.channels() == 2 && output[..n].chunks_exact(2).any(|p| p[0] != p[1]);
            samples += n as u64;
            assert!(samples < 1_000_000_000);
            assert!(stream.memory_bytes() <= 16 * 1024 * 1024);
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
        }
        assert_eq!(second_count, samples);
        assert_eq!(hex::encode(second_digest.finalize()), first_hash);
        let (witness_offset, witness) = witness.expect("recording has no audible witness");
        println!(
            "MUSIC_JSON {}",
            serde_json::json!({"output_rate":std::env::var("GAMEPKG_AUDIO_OUTPUT_RATE").ok().map(|v| v.parse::<u32>().unwrap()),"name":path.file_name().unwrap().to_string_lossy(),"rate":stream.sample_rate(),"channels":stream.channels(),"samples":samples,"sha256":first_hash,"fnv32":fnv,"sdl_samples":reference_samples,"sdl_different":different,"sdl_maximum_delta":maximum_delta,"witness_offset":witness_offset,"witness":witness})
        );
    }
}
#[test]
#[ignore = "requires built companion and GAMEPKG_MUSIC_INPUT_DIR"]
fn malformed_music_and_buffer_requests_fail_bounded_without_changing_output() {
    let factory = companion();
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
