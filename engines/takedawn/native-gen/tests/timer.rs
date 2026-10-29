use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::dispatch::controlled_block};
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let mut program = String::new();
    let mut entries = Vec::new();
    for (pc, code) in [
        (
            0x400000u32,
            vec![
                0x8b, 0x44, 0x24, 8, 0x8b, 0x4c, 0x24, 4, 0x8b, 0x54, 0x24, 12, 0x8b, 0x5c, 0x24,
                16, 0x89, 0x10, 0x89, 0x58, 4, 0x89, 0x48, 8, 0xc2, 16, 0,
            ],
        ),
        (0x400100, vec![0x8b, 2, 0xc2, 16, 0]),
        (0x600000, vec![0x90, 0xc2, 16, 0]),
    ] {
        let d = decoder.linear(&code, pc, &[]).unwrap();
        let b = controlled_block(&d, pc, pc + code.len() as u32, &budget).unwrap();
        program.push_str(b.source());
        entries.push((pc, format!("{{{pc}u,gamepkg_cpu_block_{pc:08x}}},\n")));
    }
    // A real generated DPC rearms its own timer through a checked stdcall4 HLE.
    let pc = 0x400200u32;
    let mut code = vec![
        0x8b, 0x44, 0x24, 4, 0x50, 0x6a, 0xff, 0x6a, 0xf6, 0x68, 0, 0x22, 0, 0, 0xe8,
    ];
    code.extend(
        0x700100u32
            .wrapping_sub(pc + code.len() as u32 + 4)
            .to_le_bytes(),
    );
    code.extend([0xc2, 16, 0]);
    let d = decoder.linear(&code, pc, &[]).unwrap();
    let b = controlled_block(&d, pc, pc + code.len() as u32, &budget).unwrap();
    program.push_str(b.source());
    for n in [0, 4, 5, 7, 9, 14, 19] {
        entries.push((
            pc + n,
            format!("{{{}u,gamepkg_cpu_block_{pc:08x}}},\n", pc + n),
        ));
    }
    entries.sort_unstable_by_key(|(pc, _)| *pc);
    let entries: String = entries.into_iter().map(|(_, entry)| entry).collect();
    // Independent widened-integer clock oracle. C targets replay concrete
    // expected states without relying on host signed64 overflow or int128.
    let mut state = 0x31a4_7d29_b630_5c81u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut cases = String::new();
    for i in 0..1024u32 {
        let i0 = if i % 4 == 0 {
            u64::MAX
        } else {
            100u64.saturating_add(random() % 1_000_000)
        };
        let s0 = random();
        let raw_due = match i % 8 {
            0 => 1u64 << 63,
            1 => u64::MAX,
            2 => 0,
            3 => 1,
            4 => i64::MAX as u64,
            5 => 0u64.wrapping_sub(216_000_000_000),
            _ => random(),
        };
        let repeat = [0u32, 1, 2, i32::MAX as u32][(i / 8 % 4) as usize];
        let delta = if i % 3 == 0 {
            u64::MAX
        } else {
            random() % 50_000_000
        };
        let i1 = i0.saturating_add(delta);
        let s1 = random();
        let signed_due = i128::from(raw_due as i64);
        let elapsed = i128::from(i1) - i128::from(i0);
        let ticks = i128::from(repeat) * 10_000;
        let immediate = signed_due >= 0 && u64::try_from(signed_due).unwrap() <= s0;
        let expired = if immediate {
            repeat != 0 && elapsed >= ticks
        } else if signed_due < 0 {
            elapsed >= -signed_due
        } else {
            i128::from(s1) >= signed_due
        };
        let armed = repeat != 0 || (!immediate && !expired);
        let mut next = 0;
        if expired && repeat != 0 {
            let excess = if immediate {
                elapsed - ticks
            } else if signed_due < 0 {
                elapsed + signed_due
            } else {
                i128::from(s1) - signed_due
            };
            next = ((i128::from(i1) + ticks - excess % ticks) & i128::from(u64::MAX)) as u64;
        }
        cases.push_str(&format!("{{UINT64_C({i0}),UINT64_C({s0}),UINT64_C({raw_due}),UINT64_C({i1}),UINT64_C({s1}),UINT64_C({next}),{repeat}u,{}u,{}u,{}u}},\n", u32::from(expired), u32::from(armed), u32::from(immediate || expired)));
    }
    include_str!("timer_fixture.c")
        .replace("/* PROGRAM */", &program)
        .replace("/* ENTRIES */", &entries)
        .replace("/* CLOCK_CASES */", &cases)
}
#[test]
fn export_timer_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_TIMER_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn timers_and_deferred_callbacks_preserve_faults_time_and_priority() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = root.join("runtime");
    let work = tempfile::tempdir().unwrap();
    let c = work.path().join("timer.c");
    let binary = work.path().join("timer");
    std::fs::write(&c, fixture()).unwrap();
    let mut cc = std::process::Command::new("cc");
    cc.args([
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
    .arg(c);
    for n in [
        "timer",
        "dpc_queue",
        "irql",
        "cpu",
        "cpu_code",
        "interrupt",
        "cpu_dispatch",
        "dispatch",
        "far_memory",
        "far_jump",
        "memory",
        "fault",
        "table_store",
        "x87",
    ] {
        cc.arg(runtime.join(format!("gamepkg_x86_{n}.c")));
    }
    let o = cc.arg("-o").arg(&binary).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = std::process::Command::new(binary).output().unwrap();
    assert!(
        o.status.success(),
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    print!("{}", String::from_utf8_lossy(&o.stdout));
}
