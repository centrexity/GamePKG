use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, dispatch::DispatchProgram, program::Unit};
use sha2::Digest;
fn critical_program() -> Vec<u8> {
    let mut bytes = Vec::new();
    for import in [
        0xf0100008u32,
        0xf0100000,
        0xf0100000,
        0xf0100004,
        0xf0100004,
    ] {
        bytes.push(0x68); // push guest lock VA
        bytes.extend_from_slice(&0x100040u32.to_le_bytes());
        bytes.push(0xb9); // mov ecx, trusted import PC; call ecx
        bytes.extend_from_slice(&import.to_le_bytes());
        bytes.extend_from_slice(&[0xff, 0xd1]);
    }
    bytes.extend_from_slice(&[0x46, 0xc3]);
    bytes
}
fn units() -> Vec<(u32, Vec<u8>)> {
    vec![
        (
            0x10000,
            vec![0x49, 0x74, 0x05, 0xe8, 0xf8, 0xff, 0xff, 0xff, 0xc3],
        ),
        (
            0x11000,
            vec![0x6a, 7, 0x6a, 5, 0xe8, 0xf7, 0, 0, 0, 0x83, 0xc0, 1, 0xc3],
        ),
        (
            0x11100,
            vec![0x8b, 0x44, 0x24, 4, 0x03, 0x44, 0x24, 8, 0xc2, 8, 0],
        ),
        (0x11200, vec![0xff, 0x14, 0x24, 0x83, 0xc4, 4, 0xc3]),
        (0x11210, vec![0x8b, 0x04, 0x24, 0xc3]),
        (0x11300, vec![0xff, 0xd0, 0xc3]),
        (0x11310, vec![0xb8, 0x34, 0x12, 0, 0, 0xc3]),
        (
            0x11400,
            vec![
                0xe8, 0x1b, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0xc3, 0xb8, 2, 0, 0, 0, 0xc3,
            ],
        ),
        (0x11420, vec![0xc7, 0x04, 0x24, 0x0b, 0x14, 1, 0, 0xc3]),
        (0x11500, vec![0x43, 0xf7, 0xf1, 0x46, 0xc3]),
        (0x11600, vec![0xc3]),
        (0x11610, vec![0x66, 0xc3]),
        (0x11620, vec![0xc2, 0xff, 0x7f]),
        (0x11700, vec![0xe8, 0x0b, 0xfc, 0xff, 0xff, 0xc3]),
        (0x11800, vec![0xff, 0x10, 0xc3]),
        (0x11810, vec![0xff, 0xe0]),
        (0x11a00, vec![0xb8, 0xb8, 0x2a, 0, 0, 0xc3]),
        (0x11a01, vec![0xb8, 0x2a, 0, 0, 0xc3, 0xc3]),
        (0x11b00, vec![0x74, 1, 0xb8, 0xb8, 0x2a, 0, 0, 0xc3, 0xc3]),
        (0x11c00, vec![0x43, 0xcc, 0x46, 0xc3]),
        (0x11d00, critical_program()),
        (0x11e00, vec![0xf3, 0xa4, 0x43, 0xc3]),
        (0x11e10, vec![0xf3, 0xa6, 0xc3]),
        (
            0x11900,
            vec![0xbd, 0x78, 0x56, 0x34, 0x12, 0x8b, 0x00, 0xc3],
        ),
    ]
}
fn fixture() -> String {
    let input = units();
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let decoded: Vec<_> = input
        .iter()
        .map(|(a, b)| {
            let seeds = if *a == 0x11400 {
                vec![*a, *a + 11]
            } else {
                vec![*a]
            };
            decoder
                .cfg(
                    b,
                    *a,
                    &seeds,
                    &std::collections::BTreeSet::new(),
                    &std::collections::BTreeSet::new(),
                )
                .unwrap()
        })
        .collect();
    let units: Vec<_> = input
        .iter()
        .zip(&decoded)
        .map(|((a, b), d)| Unit {
            decoded: d,
            entry: *a,
            end: *a + b.len() as u32,
        })
        .collect();
    let program = DispatchProgram::build(&units, &budget, 8 * 1024 * 1024).unwrap();
    let mut source = vec![];
    program.write_source(&mut source).unwrap();
    assert_eq!(source.len() as u64, program.source_bytes());
    assert_eq!(program.function_count(), input.len());
    assert!(program.instruction_count() > 30);
    let mut c = String::from("#define _GNU_SOURCE 1\n#include <stdint.h>\n");
    if let Some(h) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let h = h.to_str().unwrap();
        assert!(!h.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{h}\"\n"));
    }
    c.push_str(include_str!("dispatch_fixture.c"));
    let controlled = DispatchProgram::build_controlled(&units, &budget, 8 * 1024 * 1024).unwrap();
    let mut controlled_source = vec![];
    controlled.write_source(&mut controlled_source).unwrap();
    assert_eq!(controlled_source.len() as u64, controlled.source_bytes());
    let mut privileged_source = String::new();
    let mut privileged_map = String::new();
    let privileged_input = vec![
        (0x12000, vec![0xfa, 0x43, 0xfb, 0x42, 0x46, 0xc3]),
        (0x12010, vec![0xfb, 0xfa, 0x43, 0xc3]),
        (0x12020, vec![0xfb, 0xfb, 0x43, 0xc3]),
        (0x12030, vec![0xfb, 0xf3, 0xa4, 0x43, 0xc3]),
        (0x12040, vec![0xfa, 0xc3]),
        (0x12050, vec![0xfb, 0xc3]),
        (0x12060, vec![0xfb, 0xf7, 0xf1, 0xc3]),
        (0x12080, vec![0xfa, 0xfb, 0xc3]),
        (
            0x12090,
            vec![
                0xfa, 0xe8, 0x6a, 0xff, 0xfe, 0xff, 0x43, 0xfb, 0x42, 0x46, 0xc3,
            ],
        ),
    ];
    for (pc, bytes) in privileged_input {
        let decoded = decoder.linear(&bytes, pc, &[]).unwrap();
        let block = gamepkg_takedawn_native_gen::lower::dispatch::controlled_block(
            &decoded,
            pc,
            pc + bytes.len() as u32,
            &budget,
        )
        .unwrap();
        privileged_source.push_str(block.source());
        for &address in block.addresses() {
            privileged_map.push_str(&format!(
                "{{0x{address:08x}u,gamepkg_cpu_block_{pc:08x}}},\n"
            ));
        }
    }
    privileged_source.push_str(&format!(
        "static const GamePkgX86DispatchEntry privileged_entries[]={{{privileged_map}}};\n"
    ));
    let programs = format!(
        "{}\n{}\n{}",
        std::str::from_utf8(&source).unwrap(),
        std::str::from_utf8(&controlled_source).unwrap(),
        privileged_source
    );
    c = c.replace("/* PROGRAM */", &programs);
    c = c.replace(
        "/* CONTROLLED_CASES */",
        &include_str!("controlled_dispatch_fixture.c")
            .replace("/* CPU_EVENT_CASES */", include_str!("cpu_event_fixture.c")),
    );
    c = c.replace(
        "/* PRIVILEGED_CASES */",
        include_str!("privileged_dispatch_fixture.c"),
    );
    let mut cpu = String::new();
    for (a, b) in input {
        cpu.push_str(&format!("{{static const unsigned char bytes[]={{{}}};memcpy(code+0x{:x},bytes,sizeof(bytes));}}\n",b.iter().map(|v|format!("0x{v:02x}")).collect::<Vec<_>>().join(","),a-0x10000));
    }
    c.replace("/* CPU_BYTES */", &cpu)
}
#[test]
fn exports_dispatch_fixture() {
    let c = fixture();
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_DISPATCH_FIXTURE") {
        std::fs::write(p, c).unwrap();
    }
}
#[test]
fn rejects_unsupported_blocks_and_unbounded_maps() {
    let b = Budget::new(16 * 1024 * 1024);
    let d = Decoder::new(&b).unwrap();
    let bad = d.linear(&[0x0f, 0xa2, 0xc3], 0x10000, &[]).unwrap();
    let u = [Unit {
        decoded: &bad,
        entry: 0x10000,
        end: 0x10003,
    }];
    let baseline = b.used();
    assert!(DispatchProgram::build(&u, &b, 100000).is_err());
    assert_eq!(b.used(), baseline);
    let good = d.linear(&[0xc3], 0x10000, &[]).unwrap();
    let u = [Unit {
        decoded: &good,
        entry: 0x10000,
        end: 0x10001,
    }];
    assert!(DispatchProgram::build(&u, &b, 1).is_err());
    assert!(DispatchProgram::build(&[], &b, 10000).is_err());
    let mut i = d
        .linear(&[0xff, 0xd0], 0x10000, &[])
        .unwrap()
        .instructions()[0]
        .clone();
    i.operands[0].expr = Some("unsafe".into());
    assert!(gamepkg_takedawn_native_gen::lower::instruction(&i).is_err());
}
#[test]
fn breakpoint_requires_exact_qualified_encoding_and_metadata() {
    let b = Budget::new(16 * 1024 * 1024);
    let d = Decoder::new(&b).unwrap();
    let decoded = d.linear(&[0xcc], 0x10000, &[]).unwrap();
    let good = &decoded.instructions()[0];
    let emit = gamepkg_takedawn_native_gen::lower::instruction;
    assert!(emit(good).unwrap().contains("0x00010001u,ebp"));
    for bytes in ["cd03", "66cc", "f0cc", "90", "cccc", "", "zz"] {
        let mut bad = good.clone();
        bad.bytes_hex = bytes.into();
        assert!(emit(&bad).is_err());
    }
    let mut bad = good.clone();
    bad.size = 2;
    assert!(emit(&bad).is_err());
    bad = good.clone();
    bad.address = u32::MAX;
    assert!(emit(&bad).is_err());
    bad = good.clone();
    bad.call_target = Some(0x10000);
    assert!(emit(&bad).is_err());
    bad = good.clone();
    bad.jump_target = Some(0x10000);
    assert!(emit(&bad).is_err());
    bad = good.clone();
    bad.operands.push(Default::default());
    assert!(emit(&bad).is_err());
    for bytes in [&[0xcd, 3][..], &[0x66, 0xcc], &[0xf3, 0xcc]] {
        let decoded = d.linear(bytes, 0x10000, &[]).unwrap();
        assert!(emit(&decoded.instructions()[0]).is_err());
    }
}
#[cfg(target_os = "linux")]
#[test]
fn generated_dispatch_returns_resume_faults_without_native_recursion() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("dispatch.c");
    let binary = temp.path().join("dispatch");
    std::fs::write(&source, fixture()).unwrap();
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let out = std::process::Command::new("cc")
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&r)
        .arg(source)
        .arg(r.join("gamepkg_x86_dispatch.c"))
        .arg(r.join("gamepkg_x86_memory.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
        .arg(r.join("gamepkg_x86_cpu.c"))
        .arg(r.join("gamepkg_x86_interrupt.c"))
        .arg(r.join("gamepkg_x86_cpu_dispatch.c"))
        .arg(r.join("gamepkg_x86_table_store.c"))
        .arg(r.join("gamepkg_x86_far_memory.c"))
        .arg(r.join("gamepkg_x86_far_jump.c"))
        .arg(r.join("gamepkg_x86_x87.c"))
        .arg(r.join("gamepkg_x86_string.c"))
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = std::process::Command::new(binary).output().unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    println!("{}", String::from_utf8_lossy(&out.stdout));
}
#[test]
fn streamed_dispatch_matches_builder_and_rejects_unstable_inputs() {
    use gamepkg_takedawn_native_gen::dispatch::write_streamed;
    use std::collections::BTreeSet;
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let input: Vec<(u32, Vec<u8>)> = vec![
        (0x10000, vec![0x90, 0xc3]),
        (0x10001, vec![0xc3]),
        (0x11000, vec![0xe8, 0xfb, 1, 0, 0, 0xc3]),
        (0x11200, vec![0xc3]),
    ];
    let decoded: Vec<_> = input
        .iter()
        .map(|(a, b)| {
            decoder
                .cfg(b, *a, &[*a], &BTreeSet::new(), &BTreeSet::new())
                .unwrap()
        })
        .collect();
    let units: Vec<_> = input
        .iter()
        .zip(&decoded)
        .map(|((a, b), d)| Unit {
            decoded: d,
            entry: *a,
            end: *a + b.len() as u32,
        })
        .collect();
    let program = DispatchProgram::build(&units, &budget, 1000000).unwrap();
    let mut expected = vec![];
    program.write_source(&mut expected).unwrap();
    let ranges: Vec<_> = input
        .iter()
        .rev()
        .map(|(a, b)| (*a, *a + b.len() as u32))
        .collect();
    let load = |a: u32, _| -> anyhow::Result<&[u8]> {
        Ok(input.iter().find(|(at, _)| *at == a).unwrap().1.as_slice())
    };
    let before = budget.used();
    let mut actual = vec![];
    let summary = write_streamed(&ranges, load, &decoder, &budget, 1000000, &mut actual).unwrap();
    assert_eq!(actual, expected);
    let cpu_program = DispatchProgram::build_controlled(&units, &budget, 1000000).unwrap();
    let mut cpu_expected = vec![];
    cpu_program.write_source(&mut cpu_expected).unwrap();
    let mut cpu_actual = vec![];
    let cpu_summary = gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled(
        &ranges,
        load,
        &decoder,
        &budget,
        1000000,
        &mut cpu_actual,
    )
    .unwrap();
    assert_eq!(cpu_expected, cpu_actual);
    assert_eq!(cpu_summary.map_sha256, summary.map_sha256);
    assert_eq!(cpu_summary.bytes, cpu_program.source_bytes());
    drop(cpu_program);
    assert_eq!(summary.functions, 4);
    assert_eq!(summary.instruction_entries, 5);
    assert_eq!(summary.shared_tail_entries, 1);
    assert_eq!(summary.bytes, actual.len() as u64);
    assert_eq!(budget.used(), before);
    let mut expected_map = sha2::Sha256::new();
    for (address, owner) in [
        (0x10000u32, 0x10000u32),
        (0x10001, 0x10000),
        (0x11000, 0x11000),
        (0x11005, 0x11000),
        (0x11200, 0x11200),
    ] {
        expected_map.update(address.to_le_bytes());
        expected_map.update(owner.to_le_bytes());
    }
    assert_eq!(summary.map_sha256, hex::encode(expected_map.finalize()));
    assert_eq!(
        summary.source_sha256,
        hex::encode(sha2::Sha256::digest(&actual))
    );
    let mut rejected = vec![];
    assert!(
        write_streamed(
            &ranges,
            load,
            &decoder,
            &budget,
            summary.bytes - 1,
            &mut rejected
        )
        .is_err()
            && rejected.is_empty()
    );
    assert!(
        write_streamed(
            &ranges,
            |_, _| Ok(&[][..]),
            &decoder,
            &budget,
            1000000,
            &mut rejected
        )
        .is_err()
            && rejected.is_empty()
    );
    assert!(
        write_streamed(
            &ranges,
            load,
            &decoder,
            &Budget::new(1),
            1000000,
            &mut rejected
        )
        .is_err()
            && rejected.is_empty()
    );
    let calls = std::cell::Cell::new(0usize);
    let changed = [0x90, 0x90];
    assert!(write_streamed(
        &ranges,
        |a, e| {
            calls.set(calls.get() + 1);
            if calls.get() == 5 {
                Ok(&changed[..])
            } else {
                load(a, e)
            }
        },
        &decoder,
        &budget,
        1000000,
        &mut rejected
    )
    .is_err());
    assert!(!rejected.is_empty()); // Caller must stage a failed second pass.
    struct Failing;
    impl std::io::Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("fixture writer failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(write_streamed(&ranges, load, &decoder, &budget, 1000000, &mut Failing).is_err());
    let outer = [0xb8, 0x90, 0x90, 0x90, 0x90, 0xc3];
    let conflicting = [0x91, 0x90, 0x90, 0x90, 0xc3];
    let mut rejected = vec![];
    assert!(
        write_streamed(
            &[(0x10000, 0x10006), (0x10001, 0x10006)],
            |a, _| if a == 0x10000 {
                Ok(&outer[..])
            } else {
                Ok(&conflicting[..])
            },
            &decoder,
            &budget,
            1000000,
            &mut rejected
        )
        .is_err()
            && rejected.is_empty()
    );
    assert_eq!(budget.used(), before);
}
#[test]
fn streaming_dispatch_bounds_live_bodies_and_grows_map_chunks() {
    use gamepkg_takedawn_native_gen::dispatch::write_streamed;
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let baseline = budget.used();
    let peak = std::cell::Cell::new(baseline);
    struct Sink<'a> {
        budget: &'a Budget,
        peak: &'a std::cell::Cell<u64>,
        bytes: u64,
    }
    impl std::io::Write for Sink<'_> {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.peak.set(self.peak.get().max(self.budget.used()));
            self.bytes += b.len() as u64;
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = [0x90u8; 32];
    bytes[31] = 0xc3;
    let ranges: Vec<_> = (0..600u32)
        .map(|n| (0x10000 + n * 32, 0x10020 + n * 32))
        .collect();
    let mut out = Sink {
        budget: &budget,
        peak: &peak,
        bytes: 0,
    };
    let summary = write_streamed(
        &ranges,
        |_, _| {
            peak.set(peak.get().max(budget.used()));
            Ok(&bytes[..])
        },
        &decoder,
        &budget,
        64 * 1024 * 1024,
        &mut out,
    )
    .unwrap();
    assert_eq!(summary.functions, 600);
    assert_eq!(summary.instruction_entries, 19200);
    assert_eq!(summary.bytes, out.bytes);
    assert!(peak.get() - baseline < 4 * 1024 * 1024);
    assert_eq!(budget.used(), baseline);
}
#[test]
fn compatible_crossing_ranges_and_long_validation_windows() {
    use gamepkg_takedawn_native_gen::dispatch::write_streamed;
    use std::collections::BTreeSet;
    let budget = Budget::new(192 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut small = [0x90u8; 96];
    small[63] = 0xc3;
    small[95] = 0xc3;
    let ranges = [(0x10000, 0x10040), (0x10020, 0x10060)];
    let decoded: Vec<_> = ranges
        .iter()
        .map(|&(a, e)| {
            decoder
                .cfg(
                    &small[(a - 0x10000) as usize..(e - 0x10000) as usize],
                    a,
                    &[a],
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                )
                .unwrap()
        })
        .collect();
    let units: Vec<_> = ranges
        .iter()
        .zip(&decoded)
        .map(|(&(a, e), d)| Unit {
            decoded: d,
            entry: a,
            end: e,
        })
        .collect();
    let program = DispatchProgram::build(&units, &budget, 1000000).unwrap();
    let mut expected = vec![];
    program.write_source(&mut expected).unwrap();
    let mut actual = vec![];
    let summary = write_streamed(
        &ranges,
        |a, e| Ok(&small[(a - 0x10000) as usize..(e - 0x10000) as usize]),
        &decoder,
        &budget,
        1000000,
        &mut actual,
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(summary.overlapping_units, 1);
    assert_eq!(summary.shared_tail_entries, 0);
    drop(program);
    drop(decoded);
    let mut bytes = vec![0x90; 40960];
    for i in [16383, 24575, 32767, 40959] {
        bytes[i] = 0xc3;
    }
    let ranges = [
        (0x10000, 0x14000),
        (0x12000, 0x16000),
        (0x14000, 0x18000),
        (0x16000, 0x1a000),
    ];
    let before = budget.used();
    let summary = write_streamed(
        &ranges,
        |a, e| Ok(&bytes[(a - 0x10000) as usize..(e - 0x10000) as usize]),
        &decoder,
        &budget,
        64 * 1024 * 1024,
        &mut std::io::sink(),
    )
    .unwrap();
    assert_eq!(summary.overlapping_units, 3);
    assert_eq!(summary.instruction_entries, 32768);
    assert_eq!(budget.used(), before);
    bytes[16384] = 0xb8;
    let mut inconsistent = bytes.clone();
    inconsistent[16385] = 0x91;
    let conflicts = [
        (0x10000, 0x14000),
        (0x12000, 0x16000),
        (0x14000, 0x18000),
        (0x14001, 0x18000),
    ];
    let mut out = vec![];
    assert!(
        write_streamed(
            &conflicts,
            |a, e| if a == 0x14001 {
                Ok(&inconsistent[(a - 0x10000) as usize..(e - 0x10000) as usize])
            } else {
                Ok(&bytes[(a - 0x10000) as usize..(e - 0x10000) as usize])
            },
            &decoder,
            &budget,
            64 * 1024 * 1024,
            &mut out
        )
        .is_err()
            && out.is_empty()
    );
    assert_eq!(budget.used(), before);
}
#[test]
fn exact_pc_validation_rejects_different_instruction_data() {
    use gamepkg_takedawn_native_gen::dispatch::write_streamed;
    let b = Budget::new(16 * 1024 * 1024);
    let d = Decoder::new(&b).unwrap();
    let first = [0xe9, 0, 0, 0, 0, 0xc3];
    let second = [0x90, 0xc3];
    let mut out = vec![];
    let error = write_streamed(
        &[(0x10000, 0x10006), (0x10005, 0x10007)],
        |a, _| {
            if a == 0x10000 {
                Ok(&first[..])
            } else {
                Ok(&second[..])
            }
        },
        &d,
        &b,
        1000000,
        &mut out,
    )
    .err()
    .expect("conflicting same-PC models must fail");
    assert!(error.to_string().contains("conflicting same-PC"));
    assert!(out.is_empty());
}
