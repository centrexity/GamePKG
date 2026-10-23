//! Maintainer exact-output differential and alternating A/B component benchmark.
//! No games, media, audio device, Python or Cargo dependencies are required.
#[allow(dead_code)]
#[path = "baseline.rs"]
mod baseline;
#[allow(dead_code)]
#[path = "../../src/audio_resample.rs"]
mod candidate;
use std::{hint::black_box, time::Instant};

trait Converter {
    fn new(source: u32, target: u32) -> Self;
    fn capacity(&self) -> usize;
    fn append(&mut self, input: &[[f32; 2]]);
    fn finish(&mut self);
    fn render(&mut self, output: &mut [[f32; 2]]) -> usize;
}
macro_rules! converter {
    ($t:ty) => {
        impl Converter for $t {
            fn new(source: u32, target: u32) -> Self {
                <$t>::new(source, target).unwrap()
            }
            fn capacity(&self) -> usize {
                self.input_capacity()
            }
            fn append(&mut self, input: &[[f32; 2]]) {
                self.append(input).unwrap()
            }
            fn finish(&mut self) {
                self.finish()
            }
            fn render(&mut self, output: &mut [[f32; 2]]) -> usize {
                self.render(output).unwrap()
            }
        }
    };
}
converter!(baseline::Resampler);
converter!(candidate::Resampler);
converter!(baseline::MixerResampler);
converter!(candidate::MixerResampler);

fn input(length: usize, extreme: bool) -> Vec<[f32; 2]> {
    let mut random = 0x12345678u32;
    (0..length)
        .map(|index| {
            std::array::from_fn(|channel| {
                random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                if extreme {
                    let edge = [
                        0u32, 0x80000000, 1, 0x80000001, 0x00800000, 0x7f7fffff, 0xff7fffff,
                        0x3f800000, 0xbf800000,
                    ];
                    f32::from_bits(if index < edge.len() {
                        edge[(index + channel) % edge.len()]
                    } else {
                        random & 0xff7fffff
                    })
                } else {
                    f32::from((random >> 16) as i16) / 32768.0
                }
            })
        })
        .collect()
}
fn drain<T: Converter>(
    input: &[[f32; 2]],
    source: u32,
    target: u32,
    chunk: usize,
    mut sink: impl FnMut(&[[f32; 2]]),
) -> usize {
    let mut converter = T::new(source, target);
    let mut buffer = [[0.0; 2]; 800];
    let mut at = 0;
    let mut frames = 0;
    let mut step = 0;
    if input.is_empty() {
        converter.finish();
    }
    loop {
        let n = converter.render(&mut buffer[..chunk]);
        sink(&buffer[..n]);
        frames += n;
        if n == 0 {
            if at == input.len() {
                break;
            }
            // Deliberately split puts independently from output buffer sizes.
            let size = [1, 799, 7, 512, 800, 31][step % 6];
            let n = size.min(converter.capacity()).min(input.len() - at);
            assert!(n > 0, "stalled conversion");
            converter.append(&input[at..at + n]);
            at += n;
            step += 1;
            if at == input.len() {
                converter.finish();
            }
        }
    }
    frames
}
fn bits<T: Converter>(input: &[[f32; 2]], source: u32, target: u32, chunk: usize) -> Vec<[u32; 2]> {
    let mut output = Vec::new();
    drain::<T>(input, source, target, chunk, |frames| {
        output.extend(frames.iter().map(|frame| frame.map(f32::to_bits)))
    });
    output
}
fn verify_pair<A: Converter, B: Converter>(family: &str, checksum: &mut u64) -> (usize, usize) {
    let mut cases = 0;
    let mut frames = 0;
    for source in [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000] {
        for target in [44100, 48000] {
            for chunk in [1, 7, 64, 800] {
                for extreme in [false, true] {
                    for length in [0, 1, 511, 512, 513, 1023, 1024, 1025, 4097] {
                        let input = input(length, extreme);
                        let before = bits::<A>(&input, source, target, chunk);
                        let after = bits::<B>(&input, source, target, chunk);
                        assert_eq!(before,after,"{family}: {source}->{target}, chunk={chunk}, length={length}, extreme={extreme}");
                        for frame in &after {
                            for sample in frame {
                                for byte in sample.to_le_bytes() {
                                    *checksum =
                                        (*checksum ^ u64::from(byte)).wrapping_mul(1099511628211);
                                }
                            }
                        }
                        cases += 1;
                        frames += after.len();
                    }
                }
            }
        }
    }
    (cases, frames)
}
fn time<T: Converter>(input: &[[f32; 2]], source: u32, target: u32, chunk: usize) -> (u128, usize) {
    let start = Instant::now();
    let frames = drain::<T>(black_box(input), source, target, chunk, |frames| {
        black_box(frames);
    });
    (start.elapsed().as_nanos(), frames)
}
fn bench_pair<A: Converter, B: Converter>(family: &str, rounds: usize) {
    for (source, target) in [
        (44100, 48000),
        (22050, 48000),
        (48000, 44100),
        (48000, 48000),
    ] {
        let input = input(source as usize, false);
        for chunk in [1, 64, 800] {
            // Both paths are warmed before timed samples.
            let expected = time::<A>(&input, source, target, chunk).1;
            assert_eq!(expected, time::<B>(&input, source, target, chunk).1);
            let mut a = Vec::new();
            let mut b = Vec::new();
            for round in 0..rounds {
                let (before, after) = if round % 2 == 0 {
                    (
                        time::<A>(&input, source, target, chunk),
                        time::<B>(&input, source, target, chunk),
                    )
                } else {
                    let after = time::<B>(&input, source, target, chunk);
                    (time::<A>(&input, source, target, chunk), after)
                };
                assert_eq!(before.1, expected);
                assert_eq!(after.1, expected);
                a.push(before.0);
                b.push(after.0);
            }
            a.sort_unstable();
            b.sort_unstable();
            let median = rounds / 2;
            println!(
                r#"{{"kind":"timing","family":"{family}","source":{source},"target":{target},"chunk":{chunk},"rounds":{rounds},"frames":{expected},"baseline_min_ns":{},"baseline_median_ns":{},"baseline_max_ns":{},"candidate_min_ns":{},"candidate_median_ns":{},"candidate_max_ns":{},"speedup":{:.6}}}"#,
                a[0],
                a[median],
                a[rounds - 1],
                b[0],
                b[median],
                b[rounds - 1],
                a[median] as f64 / b[median] as f64
            );
        }
    }
}
pub fn verify() -> (usize, usize, u64) {
    assert_eq!(
        std::mem::size_of::<baseline::Resampler>(),
        std::mem::size_of::<candidate::Resampler>()
    );
    assert_eq!(
        std::mem::size_of::<baseline::MixerResampler>(),
        std::mem::size_of::<candidate::MixerResampler>()
    );
    let mut checksum = 14695981039346656037u64;
    let a = verify_pair::<baseline::Resampler, candidate::Resampler>("global", &mut checksum);
    let b =
        verify_pair::<baseline::MixerResampler, candidate::MixerResampler>("mixer", &mut checksum);
    (a.0 + b.0, a.1 + b.1, checksum)
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let verify_only = args.as_slice() == ["--verify-only"];
    let rounds = if args.is_empty() || verify_only {
        9
    } else {
        assert!(
            args.len() == 2 && args[0] == "--rounds",
            "usage: benchmark [--verify-only|--rounds ODD_COUNT]"
        );
        let n: usize = args[1].parse().unwrap();
        assert!((3..=31).contains(&n) && n % 2 == 1);
        n
    };
    let (cases, frames, checksum) = verify();
    println!(
        r#"{{"kind":"verification","cases":{},"frames":{},"fnv64":"{checksum:016x}","baseline_bytes":{},"candidate_bytes":{},"baseline_mixer_bytes":{},"candidate_mixer_bytes":{}}}"#,
        cases,
        frames,
        std::mem::size_of::<baseline::Resampler>(),
        std::mem::size_of::<candidate::Resampler>(),
        std::mem::size_of::<baseline::MixerResampler>(),
        std::mem::size_of::<candidate::MixerResampler>()
    );
    if !verify_only {
        bench_pair::<baseline::Resampler, candidate::Resampler>("global", rounds);
        bench_pair::<baseline::MixerResampler, candidate::MixerResampler>("mixer", rounds);
    }
}
