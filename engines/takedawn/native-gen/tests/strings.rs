use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
fn forms() -> Vec<(u32, u32, u32, u32, u32, Vec<u8>)> {
    let mut out = vec![];
    for (op, byte) in [(0, 0xa4u8), (1, 0xaa), (2, 0xac), (3, 0xa6), (4, 0xae)] {
        for w in 0..3 {
            for a16 in 0..2 {
                for repeat in 0..3 {
                    for fs in 0..2 {
                        if op < 3 && repeat == 2 {
                            continue;
                        }
                        let mut b = vec![];
                        if fs == 1 {
                            b.push(0x64);
                        }
                        if a16 == 1 {
                            b.push(0x67);
                        }
                        if w == 1 {
                            b.push(0x66);
                        }
                        if repeat != 0 {
                            b.push(if repeat == 1 { 0xf3 } else { 0xf2 });
                        }
                        b.push(byte + u8::from(w != 0));
                        out.push((op, 1 << w, a16, repeat, fs, b));
                    }
                }
            }
        }
    }
    out
}
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete string"
    );
    instruction(&x.instructions()[0])
}
#[test]
fn reject_string_prefix_and_implicit_operand_tampering() {
    for b in [
        &[0xf0, 0xa4][..],
        &[0xf2, 0xa4],
        &[0x65, 0xa4],
        &[0xf3, 0xf2, 0xa4],
        &[0x66, 0x66, 0xa5],
        &[0xf2, 0x0f, 0x10, 0xc1],
    ] {
        assert!(emit(b).is_err());
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xf3, 0xa4], 0x10000, &[]).unwrap();
    for i in 0..5 {
        let mut bad = x.instructions()[0].clone();
        match i {
            0 => bad.operands[0].mem_seg = None,
            1 => bad.operands[1].mem_base = Some("eax".into()),
            2 => bad.operands[0].mem_size = 4,
            3 => bad.operands[1].expr = Some("escape()".into()),
            _ => bad.mnemonic = "rep stosb".into(),
        }
        assert!(instruction(&bad).is_err());
    }
}
#[test]
fn export_string_variants() {
    let mut cases = String::new();
    for (op, bytes, a16, repeat, fs, b) in forms() {
        let _ = emit(&b).unwrap();
        let key = (((op * 3
            + match bytes {
                1 => 0,
                2 => 1,
                _ => 2,
            })
            * 2
            + a16)
            * 3
            + repeat)
            * 2
            + fs;
        let text = b
            .iter()
            .map(|v| format!("0x{v:02x}"))
            .collect::<Vec<_>>()
            .join(",");
        cases.push_str(&format!("case {key}: EXECUTE(\".byte {text}\");break;\n"));
    }
    assert_eq!(cases.lines().count(), 144);
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_STRING_VARIANTS") {
        std::fs::write(p, cases).unwrap();
    }
}

fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n");
    if let Some(h) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let h = h.to_str().unwrap();
        assert!(!h.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{h}\"\n"));
    }
    c.push_str(&include_str!("string_fault_fixture.c").replace(
        "/* CHECKPOINT_FAULT_CASES */",
        include_str!("string_checkpoint_faults.c"),
    ));
    let mut wrappers = String::new();
    let mut names = vec![];
    let mut list = vec![];
    for (op, bytes, a16, repeat, fs, b) in forms() {
        let code = emit(&b).unwrap();
        let name = format!("instruction_{}", names.len());
        wrappers.push_str(&format!(
            "static void {name}(void){{uint32_t ebp=0xabcdef12;{code}++completed;}}\n"
        ));
        names.push(name);
        list.push(format!("{{{op},{bytes},{a16},{repeat},{fs}}}"));
    }
    c.replace("/* WRAPPERS */",&format!("{wrappers}\nstatic void (*instructions[])(void)={{{}}};\nstatic const unsigned forms[][5]={{{}}};",names.join(","),list.join(",")))
}
#[test]
fn export_string_fault_fixture() {
    let c = fixture();
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_STRING_FIXTURE") {
        std::fs::write(p, c).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn emitted_strings_preserve_restart_state_and_partial_progress() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("strings.c");
    let binary = d.path().join("strings");
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
        .arg(r.join("gamepkg_x86_string.c"))
        .arg(r.join("gamepkg_x86_memory.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
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
    print!("{}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn all_string_forms_use_controlled_checkpoints_in_builder_and_stream() {
    use gamepkg_takedawn_native_gen::dispatch::{write_streamed_controlled, DispatchProgram};
    use gamepkg_takedawn_native_gen::program::Unit;
    let budget = Budget::new(16 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let all_input: Vec<_> = forms()
        .into_iter()
        .enumerate()
        .map(|(i, (op, width, a16, repeat, fs, mut bytes))| {
            bytes.push(0xc3);
            (0x10000 + i as u32 * 16, bytes, (op, width, a16, repeat, fs))
        })
        .collect();
    for input in all_input.chunks(16) {
        let baseline = budget.used();
        let decoded: Vec<_> = input
            .iter()
            .map(|(pc, bytes, _)| decoder.linear(bytes, *pc, &[]).unwrap())
            .collect();
        let units: Vec<_> = input
            .iter()
            .zip(&decoded)
            .map(|((pc, bytes, _), decoded)| Unit {
                decoded,
                entry: *pc,
                end: *pc + bytes.len() as u32,
            })
            .collect();
        let program = DispatchProgram::build_controlled(&units, &budget, 2 * 1024 * 1024).unwrap();
        let mut expected = vec![];
        program.write_source(&mut expected).unwrap();
        let source = std::str::from_utf8(&expected).unwrap();
        assert_eq!(
            source.matches("if(gamepkg_x86_string_controlled(").count(),
            input.len()
        );
        for (pc, _, (op, width, a16, repeat, fs)) in input {
            assert!(source.contains(&format!("if(gamepkg_x86_string_controlled({op}u,{width}u,{a16}u,{repeat}u,{fs}u,0x{pc:08x}u,ebp,gamepkg_x86_dispatch_rep_checkpoint))return;")));
        }
        let ranges: Vec<_> = units
            .iter()
            .rev()
            .map(|unit| (unit.entry, unit.end))
            .collect();
        let mut streamed = vec![];
        let summary = write_streamed_controlled(
            &ranges,
            |pc, _| {
                Ok(input
                    .iter()
                    .find(|(at, _, _)| *at == pc)
                    .unwrap()
                    .1
                    .as_slice())
            },
            &decoder,
            &budget,
            2 * 1024 * 1024,
            &mut streamed,
        )
        .unwrap();
        assert_eq!(streamed, expected);
        assert_eq!(summary.functions, input.len());
        assert_eq!(summary.instruction_entries, input.len() * 2);
        assert_eq!(summary.bytes, program.source_bytes());
        drop(program);
        drop(units);
        drop(decoded);
        assert_eq!(budget.used(), baseline);
    }
}
