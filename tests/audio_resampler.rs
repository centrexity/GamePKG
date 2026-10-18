use gamepkg::audio_resample::{MixerResampler, Resampler};

fn convert_mixer(input: &[[f32; 2]], source: u32, target: u32, small: bool) -> Vec<[f32; 2]> {
    let mut mixer = MixerResampler::new(source, target).unwrap();
    let mut output = [[0.0; 2]; 800];
    let mut result = Vec::new();
    let mut at = 0;
    let mut step = 0;
    loop {
        let size = if small {
            [1, 7, 31, 799, 2][step % 5]
        } else {
            800
        };
        let count = mixer.render(&mut output[..size]).unwrap();
        result.extend_from_slice(&output[..count]);
        if count == 0 {
            if at == input.len() {
                break;
            }
            let count = size.min(mixer.input_capacity()).min(input.len() - at);
            assert!(count > 0, "full queue cannot progress");
            mixer.append(&input[at..at + count]).unwrap();
            at += count;
            if at == input.len() {
                mixer.finish();
            }
        }
        step += 1;
    }
    result
}

#[test]
fn original_mixer_boundaries_are_independent_of_consumer_buffers() {
    let input = recording(4097);
    for source in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
        for target in [44100, 48000] {
            let a = convert_mixer(&input, source, target, false);
            assert_eq!(
                a,
                convert_mixer(&input, source, target, true),
                "{source} to {target}"
            );
            if source == target {
                assert_eq!(a, input);
            }
        }
    }
}

#[test]
#[ignore = "set GAMEPKG_SDL_RESAMPLE_REFERENCE to the compiled maintainer oracle"]
fn original_mixer_staging_and_flush_match_actual_sdl_stream_exactly() {
    let executable = std::env::var_os("GAMEPKG_SDL_RESAMPLE_REFERENCE").unwrap();
    for length in [
        1, 5, 511, 512, 513, 557, 558, 559, 1023, 1024, 1025, 1535, 1536, 2047, 2048, 4096, 8192,
    ] {
        let input = recording(length);
        let file = tempfile::NamedTempFile::new().unwrap();
        let bytes = input
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>();
        std::fs::write(file.path(), bytes).unwrap();
        for source in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
            for target in [44100, 48000] {
                let original = std::process::Command::new(&executable)
                    .arg(file.path())
                    .arg(source.to_string())
                    .arg(target.to_string())
                    .arg("stream")
                    .output()
                    .unwrap();
                assert!(original.status.success());
                let actual = convert_mixer(&input, source, target, true);
                assert_eq!(
                    actual.len() * 8,
                    original.stdout.len(),
                    "{length} frames, {source} to {target} length"
                );
                for (index, (sample, expected)) in actual
                    .iter()
                    .flatten()
                    .zip(original.stdout.chunks_exact(4))
                    .enumerate()
                {
                    assert_eq!(
                        sample.to_bits(),
                        u32::from_le_bytes(expected.try_into().unwrap()),
                        "{length} frames, {source} to {target}, sample {index}"
                    );
                }
            }
        }
    }
}

fn recording(count: usize) -> Vec<[f32; 2]> {
    let mut state = 0x9e3779b9u32;
    (0..count)
        .map(|i| {
            let mut frame = [0.0; 2];
            for sample in &mut frame {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *sample = (state as i16) as f32 / 32768.0;
            }
            if i < 8 {
                frame = [
                    if i == 0 { 1.0 } else { 0.0 },
                    if i == 7 { -1.0 } else { 0.0 },
                ];
            }
            frame
        })
        .collect()
}
fn convert(input: &[[f32; 2]], source: u32, target: u32, small: bool) -> Vec<[f32; 2]> {
    let mut resampler = Resampler::new(source, target).unwrap();
    let mut result = Vec::new();
    let mut output = [[0.0; 2]; 800];
    let sizes = if small { [1, 7, 31, 799, 2] } else { [800; 5] };
    let mut at = 0;
    let mut step = 0;
    while at < input.len() {
        let end = (at + sizes[step % sizes.len()]).min(input.len());
        resampler.append(&input[at..end]).unwrap();
        at = end;
        loop {
            let count = resampler
                .render(&mut output[..sizes[(step + 1) % sizes.len()]])
                .unwrap();
            result.extend_from_slice(&output[..count]);
            if count == 0 {
                break;
            }
        }
        step += 1;
    }
    resampler.finish();
    loop {
        let count = resampler.render(&mut output).unwrap();
        result.extend_from_slice(&output[..count]);
        if count == 0 {
            break;
        }
    }
    result
}
#[test]
fn streaming_phase_edges_channels_and_duration_do_not_depend_on_buffer_sizes() {
    let input = recording(44117);
    for source in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
        for target in [44100, 48000] {
            let a = convert(&input, source, target, false);
            let b = convert(&input, source, target, true);
            assert_eq!(a.len(), input.len() * target as usize / source as usize);
            assert_eq!(a, b, "{source} to {target}");
            if source == target {
                assert_eq!(a, input);
            }
        }
    }
}
#[test]
fn short_and_empty_streams_finish_without_extra_frames() {
    for n in 0..12 {
        let input = recording(n);
        for (source, target) in [(44100, 48000), (48000, 44100)] {
            assert_eq!(
                convert(&input, source, target, true).len(),
                n * target as usize / source as usize
            );
        }
    }
}
#[test]
fn invalid_buffers_and_backpressure_reject_without_corrupting_queued_audio() {
    assert!(Resampler::new(0, 48000).is_err());
    assert!(Resampler::new(44100, 96000).is_err());
    let mut r = Resampler::new(48000, 48000).unwrap();
    let input = recording(800);
    r.append(&input).unwrap();
    assert!(r.append(&[]).is_err());
    assert!(r.append(&[[0.0; 2]; 801]).is_err());
    assert!(r.append(&[[f32::NAN, 0.0]]).is_err());
    assert!(r.append(&[[0.0, f32::INFINITY]]).is_err());
    r.append(&input).unwrap();
    assert!(r.append(&input).is_err());
    let mut output = [[123.0; 2]; 801];
    assert!(r.render(&mut output).is_err());
    assert_eq!(output, [[123.0; 2]; 801]);
    assert!(r.render(&mut []).is_err());
    assert_eq!(r.render(&mut output[..800]).unwrap(), 800);
    assert_eq!(&output[..800], &input);
    r.finish();
    assert!(r.append(&input).is_err());
    assert_eq!(r.render(&mut output[..800]).unwrap(), 800);
    assert_eq!(&output[..800], &input);
    assert_eq!(r.render(&mut output[..800]).unwrap(), 0);
}
#[test]
#[ignore = "set GAMEPKG_SDL_RESAMPLE_REFERENCE to the compiled maintainer oracle"]
fn complete_stream_matches_independent_sdl_float_conversion_exactly() {
    let executable = std::env::var_os("GAMEPKG_SDL_RESAMPLE_REFERENCE").unwrap();
    let input = recording(4096);
    let file = tempfile::NamedTempFile::new().unwrap();
    let mut bytes = Vec::new();
    for sample in input.iter().flatten() {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(file.path(), bytes).unwrap();
    for source in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
        for target in [44100, 48000] {
            let original = std::process::Command::new(&executable)
                .arg(file.path())
                .arg(source.to_string())
                .arg(target.to_string())
                .output()
                .unwrap();
            assert!(
                original.status.success(),
                "{}",
                String::from_utf8_lossy(&original.stderr)
            );
            let actual = convert(&input, source, target, true);
            assert_eq!(
                actual.len() * 8,
                original.stdout.len(),
                "{source} to {target} length"
            );
            for (index, (sample, expected)) in actual
                .iter()
                .flatten()
                .zip(original.stdout.chunks_exact(4))
                .enumerate()
            {
                assert_eq!(
                    sample.to_bits(),
                    u32::from_le_bytes(expected.try_into().unwrap()),
                    "{source} to {target} sample {index}"
                );
            }
        }
    }
}
