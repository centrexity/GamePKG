use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    dispatch::{write_streamed_controlled, DispatchProgram},
    lower::{dispatch::controlled_far_instruction, instruction},
    program::Unit,
};
fn pointer(selector: u16, offset: u32, small: bool) -> Vec<u8> {
    let mut b = Vec::new();
    if small {
        b.push(0x66);
    }
    b.push(0xea);
    b.extend_from_slice(&offset.to_le_bytes()[..if small { 2 } else { 4 }]);
    b.extend(selector.to_le_bytes());
    b
}
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut programs = Vec::new();
    let mut rows = String::new();
    for selector in [0u16, 8, 16, 20, 0xfff8] {
        for small in [false, true] {
            let pc = 0x10000 + programs.len() as u32 * 0x40;
            let target = if small { 0x2000 } else { 0x20000 };
            let mut b = pointer(selector, target, small);
            rows.push_str(&format!(
                "{{{pc}u,{}u,{selector}u,{target}u,{}u}},\n",
                b.len(),
                u32::from(small)
            ));
            b.extend([0x43, 0xc3]); // Must not fall through after the far JMP.
            programs.push((pc, b));
        }
    }
    for target in [0x2000, 0x20000] {
        programs.push((target, vec![0xb8, 0x44, 0x33, 0x22, 0x11, 0xff, 0xe1]));
    }
    programs.sort_by_key(|(pc, _)| *pc);
    let decoded = programs
        .iter()
        .map(|(pc, b)| decoder.linear(b, *pc, &[]).unwrap())
        .collect::<Vec<_>>();
    let units = programs
        .iter()
        .zip(&decoded)
        .map(|((pc, b), decoded)| Unit {
            decoded,
            entry: *pc,
            end: *pc + b.len() as u32,
        })
        .collect::<Vec<_>>();
    let mut built = Vec::new();
    DispatchProgram::build_controlled(&units, &budget, 8 * 1024 * 1024)
        .unwrap()
        .write_source(&mut built)
        .unwrap();
    let extents = units.iter().map(|u| (u.entry, u.end)).collect::<Vec<_>>();
    let mut streamed = Vec::new();
    write_streamed_controlled(
        &extents,
        |pc, _| {
            Ok(programs
                .iter()
                .find(|(p, _)| *p == pc)
                .unwrap()
                .1
                .as_slice())
        },
        &decoder,
        &budget,
        8 * 1024 * 1024,
        &mut streamed,
    )
    .unwrap();
    assert_eq!(built, streamed);
    include_str!("far_dispatch_fixture.c")
        .replace("/* PROGRAM */", std::str::from_utf8(&built).unwrap())
        .replace("/* ROWS */", &rows)
}
#[test]
fn export_far_dispatch_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_FAR_DISPATCH_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
fn far_jumps_reject_forged_encoding_metadata_and_ordinary_admission() {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    for small in [false, true] {
        for selector in [0u16, 1, 8, 9, 10, 16, 0xffff] {
            for offset in [0u32, 1, 9, 10, 0xffff, 0xffffffff] {
                let b = pointer(selector, offset, small);
                let d = decoder.linear(&b, 0x10000, &[]).unwrap();
                let i = &d.instructions()[0];
                assert!(controlled_far_instruction(i).is_ok());
                assert!(instruction(i).is_err());
                let units = [Unit {
                    decoded: &d,
                    entry: 0x10000,
                    end: 0x10000 + b.len() as u32,
                }];
                assert!(DispatchProgram::build(&units, &budget, 1024 * 1024).is_err());
            }
        }
    }
    let b = pointer(16, 0x20000, false);
    let d = decoder.linear(&b, 0x10000, &[]).unwrap();
    let original = &d.instructions()[0];
    for mutated in 0..18 {
        let mut i = original.clone();
        match mutated {
            0 => i.mnemonic = "jmp".into(),
            1 => i.size += 1,
            2 => i.bytes_hex.push_str("90"),
            3 => i.bytes_hex = format!("66{}", i.bytes_hex),
            4 => i.call_target = Some(0x20000),
            5 => i.jump_target = Some(0x20000),
            6 => i.memory_refs.push(0x1000),
            7 => i.imm_values.reverse(),
            8 => i.imm_values.push(0),
            9 => i.operands[0].expr = Some("escape()".into()),
            10 => i.operands[1].expr_src = Some("escape()".into()),
            11 => i.operands[0].mem_base = Some("eax".into()),
            12 => i.operands[1].imm = Some(0x1234),
            13 => i.operands[0].kind = "reg",
            14 => i.op_str = "escape()".into(),
            15 => i.bytes_hex = format!("f0{}", i.bytes_hex),
            16 => i.bytes_hex = format!("67{}", i.bytes_hex),
            _ => i.bytes_hex = format!("6464{}", i.bytes_hex),
        }
        assert!(
            controlled_far_instruction(&i).is_err(),
            "mutation {mutated}"
        );
    }
    for b in [vec![0xff, 0x2e], vec![0x66, 0xff, 0x2e]] {
        let d = decoder.linear(&b, 0x10000, &[]).unwrap();
        assert!(controlled_far_instruction(&d.instructions()[0]).is_err());
    }
}
#[test]
#[cfg(target_os = "linux")]
fn generated_far_transfers_commit_and_fetch_with_scoped_cpu_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("far.c");
    let binary = work.path().join("far");
    std::fs::write(&source, fixture()).unwrap();
    let mut build = std::process::Command::new("cc");
    build
        .args([
            "-std=c11",
            "-O3",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pthread",
            "-fsanitize=address,undefined",
            "-fno-sanitize-recover=all",
        ])
        .arg("-I")
        .arg(&runtime)
        .arg(&source);
    for name in [
        "dispatch",
        "cpu",
        "interrupt",
        "cpu_dispatch",
        "table_store",
        "far_memory",
        "far_jump",
        "memory",
        "fault",
        "x87",
    ] {
        build.arg(runtime.join(format!("gamepkg_x86_{name}.c")));
    }
    let result = build.arg("-o").arg(&binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(binary).output().unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    print!("{}", String::from_utf8_lossy(&result.stdout));
}
