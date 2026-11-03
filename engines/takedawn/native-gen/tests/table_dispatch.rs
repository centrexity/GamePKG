use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    dispatch::{write_streamed_controlled, DispatchProgram},
    lower::{dispatch::controlled_block, instruction},
    program::Unit,
};
fn programs() -> Vec<(u32, Vec<u8>)> {
    let mut result = Vec::new();
    for op in 0..2u8 {
        for small in 0..2 {
            for mode in 0..8 {
                let mut b = Vec::new();
                if small != 0 {
                    b.push(0x66);
                }
                match mode {
                    1 => b.push(0x64),
                    2 => b.push(0x67),
                    7 => b.extend([0x64, 0x67]),
                    _ => {}
                }
                b.extend([0x0f, 0x01]);
                b.extend(match mode {
                    0 | 1 => vec![op << 3 | 6],
                    2 | 7 => vec![op << 3],
                    3 => vec![0x84 | (op << 3), 0xfe, 0x20, 0, 0, 0],
                    4 => vec![0x45 | (op << 3), 0],
                    5 => vec![0x44 | (op << 3), 0x24, 6],
                    _ => vec![0x05 | (op << 3), 3, 0x10, 0, 0],
                });
                b.extend([0x43, 0xc3]);
                result.push((0x10000 + result.len() as u32 * 0x20, b));
            }
        }
    }
    result
}
fn fixture() -> String {
    let budget = Budget::new(16 * 1024 * 1024);
    let d = Decoder::new(&budget).unwrap();
    let programs = programs();
    let decoded = programs
        .iter()
        .map(|(pc, b)| d.linear(b, *pc, &[]).unwrap())
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
    let mut generated = Vec::new();
    DispatchProgram::build_controlled(&units, &budget, 16 * 1024 * 1024)
        .unwrap()
        .write_source(&mut generated)
        .unwrap();
    let extents = units.iter().map(|u| (u.entry, u.end)).collect::<Vec<_>>();
    let mut streamed = Vec::new();
    write_streamed_controlled(
        &extents,
        |entry, _| {
            Ok(programs
                .iter()
                .find(|(pc, _)| *pc == entry)
                .unwrap()
                .1
                .as_slice())
        },
        &d,
        &budget,
        16 * 1024 * 1024,
        &mut streamed,
    )
    .unwrap();
    assert_eq!(generated, streamed);
    let mut rows = String::new();
    for (n, ((pc, _), decoded)) in programs.iter().zip(&decoded).enumerate() {
        assert!(instruction(&decoded.instructions()[0]).is_err());
        rows.push_str(&format!(
            "{{{pc}u,{}u,{}u,{}u,{}u}},\n",
            decoded.instructions()[0].size,
            n / 16,
            (n / 8) % 2,
            n % 8
        ));
    }
    include_str!("table_dispatch_fixture.c")
        .replace("/* PROGRAM */", std::str::from_utf8(&generated).unwrap())
        .replace("/* ROWS */", &rows)
}
#[test]
fn export_table_dispatch_fixture() {
    if let Some(path) = std::env::var_os("GAMEPKG_TABLE_DISPATCH_FIXTURE") {
        std::fs::write(path, fixture()).unwrap();
    }
}
#[test]
fn controlled_tables_reject_forged_operands_and_ordinary_admission() {
    let budget = Budget::new(16 * 1024 * 1024);
    let d = Decoder::new(&budget).unwrap();
    let decoded = d.linear(&[0x0f, 0x01, 6], 0x10000, &[]).unwrap();
    let original = &decoded.instructions()[0];
    assert!(instruction(original).is_err());
    assert!(
        gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(original)
            .is_ok()
    );
    for width in [1, 2, 4, 8, 10, 16] {
        let mut bad = original.clone();
        bad.operands[0].mem_size = width;
        assert!(
            gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad)
                .is_err()
        );
    }
    for bytes in [
        "0f010e",
        "0f0116",
        "0f01c0",
        "f00f0106",
        "f30f0106",
        "650f0106",
        "64640f0106",
        "66660f0106",
        "67670f0106",
        "0f010690",
        "0f0144",
        "0f0186",
    ] {
        let mut bad = original.clone();
        bad.bytes_hex = bytes.into();
        bad.size = (bytes.len() / 2) as u8;
        assert!(
            gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad)
                .is_err()
        );
    }
    for segment in ["cs", "ds", "es", "ss", "gs", "fs"] {
        let mut bad = original.clone();
        bad.operands[0].mem_seg = Some(segment.into());
        assert!(
            gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad)
                .is_err()
        );
    }
    let mut bad = original.clone();
    bad.operands[0].expr_src = Some("escape()".into());
    assert!(
        gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad).is_err()
    );
    let mut bad = original.clone();
    bad.jump_target = Some(0x1234);
    assert!(
        gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad).is_err()
    );
    let mut bad = original.clone();
    bad.imm_values.push(9);
    assert!(
        gamepkg_takedawn_native_gen::lower::dispatch::controlled_table_instruction(&bad).is_err()
    );
    assert!(controlled_block(&decoded, 0x10000, 0x10003, &budget).is_ok());
}
#[test]
#[cfg(target_os = "linux")]
fn generated_tables_require_scoped_cpu_provider_and_correct_retirement() {
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("table.c");
    let binary = work.path().join("table");
    std::fs::write(&source, fixture()).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
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
        "table_store",
        "far_memory",
        "far_jump",
        "x87",
        "cpu_dispatch",
        "cpu",
        "interrupt",
        "fault",
        "memory",
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
