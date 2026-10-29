use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, dispatch::DispatchProgram, program::Unit};

#[test]
fn interrupt_instructions_require_controlled_cpu_generation() {
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    for bytes in [[0xfa, 0xc3], [0xfb, 0xc3]] {
        let decoded = decoder.linear(&bytes, 0x10000, &[]).unwrap();
        assert!(
            gamepkg_takedawn_native_gen::lower::instruction(&decoded.instructions()[0]).is_err()
        );
        let units = [Unit {
            decoded: &decoded,
            entry: 0x10000,
            end: 0x10002,
        }];
        assert!(DispatchProgram::build(&units, &budget, 1024 * 1024).is_err());
        let program = DispatchProgram::build_controlled(&units, &budget, 1024 * 1024).unwrap();
        let mut source = vec![];
        program.write_source(&mut source).unwrap();
        let mut streamed = vec![];
        gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled(
            &[(0x10000, 0x10002)],
            |_, _| Ok(&bytes[..]),
            &decoder,
            &budget,
            1024 * 1024,
            &mut streamed,
        )
        .unwrap();
        assert_eq!(streamed, source);
        let text = std::str::from_utf8(&source).unwrap();
        let op = u8::from(bytes[0] == 0xfb);
        assert!(text.contains(&format!(
            "if(gamepkg_x86_dispatch_interrupt({op}u,ebp))return;\n"
        )));
        assert!(!text.contains(&format!(
            "gamepkg_x86_dispatch_interrupt({op}u,ebp))return;\nif(gamepkg_x86_dispatch_retire"
        )));
    }
}

#[test]
fn controlled_interrupts_reject_prefixes_and_forged_metadata() {
    use gamepkg_takedawn_native_gen::{
        decode::Operand, lower::dispatch::controlled_interrupt_instruction,
    };
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    for bytes in [
        &[0x66, 0xfa][..],
        &[0x64, 0xfb],
        &[0xf3, 0xfa],
        &[0xf0, 0xfb],
    ] {
        if let Ok(decoded) = decoder.linear(bytes, 0x10000, &[]) {
            assert!(decoded
                .instructions()
                .iter()
                .all(|i| controlled_interrupt_instruction(i).is_err()));
        }
    }
    let decoded = decoder.linear(&[0xfa], 0x10000, &[]).unwrap();
    for field in 0..9 {
        let mut bad = decoded.instructions()[0].clone();
        match field {
            0 => bad.bytes_hex = "fb".into(),
            1 => bad.size = 2,
            2 => bad.op_str = "escape()".into(),
            3 => bad.operands.push(Operand::default()),
            4 => bad.call_target = Some(0x10000),
            5 => bad.jump_target = Some(0x10000),
            6 => bad.memory_refs.push(0),
            7 => bad.imm_values.push(0),
            _ => bad.mnemonic = "sti".into(),
        }
        assert!(controlled_interrupt_instruction(&bad).is_err());
    }
}

#[cfg(target_os = "linux")]
fn run_fixture(fixture: &str, extra: &[&str]) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("interrupt-state");
    let mut build = std::process::Command::new("cc");
    build
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
        .arg(root.join("runtime"))
        .arg(root.join("tests").join(fixture))
        .arg(root.join("runtime/gamepkg_x86_interrupt.c"));
    for source in extra {
        build.arg(root.join("runtime").join(source));
    }
    let build = build.arg("-o").arg(&exe).output().unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = std::process::Command::new(exe).output().unwrap();
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn interrupt_state_privilege_virtualization_and_retirement() {
    run_fixture("interrupt_state.c", &[]);
}

#[cfg(target_os = "linux")]
#[test]
fn shared_cpu_ownership_interrupt_exclusion_and_blocking() {
    run_fixture("cpu_domain.c", &["gamepkg_x86_cpu.c"]);
}

#[cfg(target_os = "linux")]
#[test]
fn irq_flag_plan_rejects_unsupported_or_aliased_state() {
    run_fixture("portable_irq_flags.c", &[]);
}
