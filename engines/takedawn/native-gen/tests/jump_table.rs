use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::Decoder,
    dispatch::DispatchProgram,
    jump_table::{self, Table},
    program::Unit,
};
use std::collections::BTreeSet;
fn input(start: u32, negative: bool) -> (Vec<u8>, Table) {
    let mut b = vec![0xcc; 64];
    b[..3].copy_from_slice(&[0x83, 0xe1, 3]);
    let at = if negative {
        b[3..6].copy_from_slice(&[0x83, 0xe9, 4]);
        6
    } else {
        3
    };
    b[at..at + 3].copy_from_slice(&[0xff, 0x24, 0x8d]);
    b[at + 3..at + 7].copy_from_slice(&(start + if negative { 32 } else { 16 }).to_le_bytes());
    for i in 0..4usize {
        let pc = start + 32 + i as u32 * 6;
        b[16 + i * 4..20 + i * 4].copy_from_slice(&pc.to_le_bytes());
        b[32 + i * 6] = 0xb8;
        b[33 + i * 6..37 + i * 6].copy_from_slice(&(40 + i as u32).to_le_bytes());
        b[37 + i * 6] = 0xc3;
    }
    (
        b,
        Table {
            branch: start + at as u32,
            first_index: if negative { u32::MAX - 3 } else { 0 },
            entries: 4,
        },
    )
}
// Code ends at66; the eight LE pointers occupy the exact trailing32 bytes.
// This mirrors the reviewed extent shape without containing owned game bytes.
fn tail_input(start: u32) -> (Vec<u8>, Table) {
    let mut b = vec![0xcc; 98];
    b[..8].copy_from_slice(&[0x83, 0xf8, 7, 0x77, 55, 0xff, 0x24, 0x85]);
    b[8..12].copy_from_slice(&(start + 66).to_le_bytes());
    for i in 0..8usize {
        let pc = start + 12 + i as u32 * 6;
        b[12 + i * 6..18 + i * 6].copy_from_slice(&[0xb8, 100 + i as u8, 0, 0, 0, 0xc3]);
        b[66 + i * 4..70 + i * 4].copy_from_slice(&pc.to_le_bytes());
    }
    b[60..66].copy_from_slice(&[0xb8, 0xad, 0xde, 0, 0, 0xc3]);
    (
        b,
        Table {
            branch: start + 5,
            first_index: 0,
            entries: 8,
        },
    )
}
#[test]
fn trailing_eight_entry_table_requires_exact_code_and_data_identity() {
    use sha2::{Digest, Sha256};
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let (b, t) = tail_input(0x1000);
    let profile = serde_json::json!({"start":0x1000,"end":0x1062,"sha256":hex::encode(Sha256::digest(&b)),"tables":[{"branch":t.branch,"first_index":0,"entries":8}]});
    let p: jump_table::Function = serde_json::from_value(profile).unwrap();
    let d = p.decode(&decoder, &b, &budget).unwrap();
    assert_eq!(d.instructions().len(), 21);
    for i in 0..8 {
        assert!(d.instructions().iter().any(|v| v.address == 0x100c + i * 6));
    }
    assert!(d.instructions().iter().all(|v| v.end() <= 0x1042));
    assert!(jump_table::decode(&decoder, &b[..66], 0x1000, &[t], &budget).is_err());
    assert!(jump_table::decode(&decoder, &b[..97], 0x1000, &[t], &budget).is_err());
    assert!(
        jump_table::decode(&decoder, &b, 0x1000, &[Table { entries: 9, ..t }], &budget).is_err()
    );
    for target in [0x1042u32, 0x1043, 0x1061, 0x1062] {
        let mut bad = b.clone();
        bad[66..70].copy_from_slice(&target.to_le_bytes());
        assert!(jump_table::decode(&decoder, &bad, 0x1000, &[t], &budget).is_err());
    }
    for at in [0, 12, 60, 66, 97] {
        let mut bad = b.clone();
        bad[at] ^= 1;
        assert!(p.decode(&decoder, &bad, &budget).is_err());
    }
    DispatchProgram::build_controlled(
        &[Unit {
            decoded: &d,
            entry: 0x1000,
            end: 0x1062,
        }],
        &budget,
        1024 * 1024,
    )
    .unwrap();
}
#[test]
fn declared_tables_recover_hidden_targets_and_wrapped_indexes() {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    for negative in [false, true] {
        let (b, t) = input(0x1000, negative);
        let old = decoder
            .cfg(&b, 0x1000, &[0x1000], &BTreeSet::new(), &BTreeSet::new())
            .unwrap();
        assert!(!old.instructions().iter().any(|i| i.address == 0x1020));
        let d = jump_table::decode(&decoder, &b, 0x1000, &[t], &budget).unwrap();
        for n in 0..4 {
            assert!(d.instructions().iter().any(|i| i.address == 0x1020 + n * 6));
        }
        assert!(!d
            .instructions()
            .iter()
            .any(|i| (0x1010..0x1020).contains(&i.address)));
        let ordinary = DispatchProgram::build(
            &[Unit {
                decoded: &d,
                entry: 0x1000,
                end: 0x1040,
            }],
            &budget,
            1024 * 1024,
        )
        .unwrap();
        assert_eq!(ordinary.instruction_count(), if negative { 11 } else { 10 });
        DispatchProgram::build_controlled(
            &[Unit {
                decoded: &d,
                entry: 0x1000,
                end: 0x1040,
            }],
            &budget,
            1024 * 1024,
        )
        .unwrap();
    }
}
#[test]
fn nested_tables_and_function_identity_are_checked() {
    use sha2::{Digest, Sha256};
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let (mut b, outer) = input(0x1000, false);
    b.resize(128, 0xcc);
    b[16..20].copy_from_slice(&0x1050u32.to_le_bytes());
    b[80..83].copy_from_slice(&[0xff, 0x24, 0x85]);
    b[83..87].copy_from_slice(&0x1040u32.to_le_bytes());
    for n in 0..2usize {
        b[64 + n * 4..68 + n * 4].copy_from_slice(&(0x1060 + n as u32 * 6).to_le_bytes());
        b[96 + n * 6..102 + n * 6].copy_from_slice(&[0xb8, n as u8, 0, 0, 0, 0xc3]);
    }
    let inner = Table {
        branch: 0x1050,
        first_index: 0,
        entries: 2,
    };
    let profile = serde_json::json!({"start":0x1000,"end":0x1080,"sha256":hex::encode(Sha256::digest(&b)),"tables":[{"branch":outer.branch,"first_index":0,"entries":4},{"branch":inner.branch,"first_index":0,"entries":2}]});
    let p: jump_table::Function = serde_json::from_value(profile.clone()).unwrap();
    let decoded = p.decode(&decoder, &b, &budget).unwrap();
    for target in [0x1050, 0x1060, 0x1066] {
        assert!(decoded.instructions().iter().any(|i| i.address == target));
    }
    assert!(jump_table::decode(&decoder, &b, 0x1000, &[inner], &budget).is_err());
    b[100] ^= 1;
    assert!(p.decode(&decoder, &b, &budget).is_err());
    b[100] ^= 1;
    assert!(p.decode(&decoder, &b[..127], &budget).is_err());
    let mut invalid = profile;
    invalid["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<jump_table::Function>(invalid).is_err());
}
#[test]
fn streamed_profiles_match_builder_and_reject_unused_or_changed_inputs() {
    use sha2::{Digest, Sha256};
    use std::{cell::Cell, collections::BTreeMap};
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let (b, t) = input(0x1000, false);
    let profile = serde_json::json!({"start":0x1000,"end":0x1040,"sha256":hex::encode(Sha256::digest(&b)),"tables":[{"branch":t.branch,"first_index":0,"entries":4}]});
    let p = jump_table::Function::read(serde_json::to_vec(&profile).unwrap().as_slice(), &budget)
        .unwrap();
    let d = p.decode(&decoder, &b, &budget).unwrap();
    let built = DispatchProgram::build_controlled(
        &[Unit {
            decoded: &d,
            entry: 0x1000,
            end: 0x1040,
        }],
        &budget,
        1024 * 1024,
    )
    .unwrap();
    let mut expected = Vec::new();
    built.write_source(&mut expected).unwrap();
    let profiles = BTreeMap::from([(0x1000, p)]);
    let mut output = Vec::new();
    let summary = gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled_tables(
        &[(0x1000, 0x1040)],
        |_, _| Ok(&b),
        &decoder,
        &budget,
        1024 * 1024,
        &mut output,
        &profiles,
    )
    .unwrap();
    assert_eq!(output, expected);
    assert_eq!(summary.instruction_entries, 10);
    output.clear();
    assert!(
        gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled_tables(
            &[(0x1100, 0x1140)],
            |_, _| Ok(&b),
            &decoder,
            &budget,
            1024 * 1024,
            &mut output,
            &profiles
        )
        .is_err()
    );
    assert!(output.is_empty());
    let mut changed = b.clone();
    changed[63] ^= 1;
    let calls = Cell::new(0);
    assert!(
        gamepkg_takedawn_native_gen::dispatch::write_streamed_controlled_tables(
            &[(0x1000, 0x1040)],
            |_, _| {
                let n = calls.get();
                calls.set(n + 1);
                Ok(if n == 0 { &b } else { &changed })
            },
            &decoder,
            &budget,
            1024 * 1024,
            &mut output,
            &profiles
        )
        .is_err()
    );
    assert_eq!(calls.get(), 2);
    assert!(jump_table::Function::read(vec![b' '; 65537].as_slice(), &budget).is_err());
}
#[test]
fn tables_reject_forged_bounds_data_targets_and_unreached_branches() {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let (b, t) = input(0x1000, false);
    for entries in [0, 5, u32::MAX] {
        assert!(
            jump_table::decode(&decoder, &b, 0x1000, &[Table { entries, ..t }], &budget).is_err()
        );
    }
    for branch in [0, 0x1002, 0x103f, u32::MAX] {
        assert!(
            jump_table::decode(&decoder, &b, 0x1000, &[Table { branch, ..t }], &budget).is_err()
        );
    }
    assert!(jump_table::decode(&decoder, &b, 0x1000, &[t, t], &budget).is_err());
    for first_index in [1, u32::MAX, u32::MAX - 3] {
        assert!(
            jump_table::decode(&decoder, &b, 0x1000, &[Table { first_index, ..t }], &budget)
                .is_err()
        );
    }
    for target in [0, 0x1010, 0x1011, 0x101f, 0x1040, u32::MAX] {
        let mut bad = b.clone();
        bad[16..20].copy_from_slice(&target.to_le_bytes());
        assert!(jump_table::decode(&decoder, &bad, 0x1000, &[t], &budget).is_err());
    }
    for (at, v) in [(3, 0x64), (4, 0x14), (5, 0xa5), (5, 0x4d)] {
        let mut bad = b.clone();
        bad[at] = v;
        assert!(jump_table::decode(&decoder, &bad, 0x1000, &[t], &budget).is_err());
    }
    // A valid but unreachable second table cannot authorize arbitrary entries.
    let mut bad = b.clone();
    bad[56..59].copy_from_slice(&[0xff, 0x24, 0x8d]);
    bad[59..63].copy_from_slice(&0x1010u32.to_le_bytes());
    assert!(jump_table::decode(
        &decoder,
        &bad,
        0x1000,
        &[
            t,
            Table {
                branch: 0x1038,
                ..t
            }
        ],
        &budget
    )
    .is_err());
    // A pointer to undecodable bytes fails rather than silently dropping a target.
    let mut bad = b.clone();
    bad[16..20].copy_from_slice(&0x103fu32.to_le_bytes());
    bad[63] = 0x0f;
    assert!(jump_table::decode(&decoder, &bad, 0x1000, &[t], &budget).is_err());
}
fn fixture() -> String {
    let budget = Budget::new(32 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let input = [
        input(0x1000, false),
        input(0x1100, true),
        tail_input(0x1200),
    ];
    let decoded: Vec<_> = input
        .iter()
        .enumerate()
        .map(|(i, (b, t))| {
            jump_table::decode(&decoder, b, 0x1000 + i as u32 * 256, &[*t], &budget).unwrap()
        })
        .collect();
    let units: Vec<_> = decoded
        .iter()
        .enumerate()
        .map(|(i, d)| Unit {
            decoded: d,
            entry: 0x1000 + i as u32 * 256,
            end: 0x1000 + i as u32 * 256 + input[i].0.len() as u32,
        })
        .collect();
    let p = DispatchProgram::build(&units, &budget, 1024 * 1024).unwrap();
    let mut code = Vec::new();
    p.write_source(&mut code).unwrap();
    let init:String=input.iter().enumerate().map(|(i,(b,_))|format!("static const unsigned char code{i}[]={{{}}};memcpy(image+{},code{i},sizeof(code{i}));\n",b.iter().map(u8::to_string).collect::<Vec<_>>().join(","),i*256)).collect();
    include_str!("jump_table_fixture.c")
        .replace("/* PROGRAM */", &String::from_utf8(code).unwrap())
        .replace("/* INIT */", &init)
}
#[test]
fn export_jump_table_fixture() {
    if let Some(p) = std::env::var_os("GAMEPKG_JUMP_TABLE_FIXTURE") {
        std::fs::write(p, fixture()).unwrap();
    }
}
#[test]
#[cfg(target_os = "linux")]
fn generated_jumps_use_live_tables_and_preserve_fault_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = tempfile::tempdir().unwrap();
    let c = work.path().join("jump.c");
    let binary = work.path().join("jump");
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
    .arg(root.join("runtime"))
    .arg(c);
    for name in [
        "cpu",
        "interrupt",
        "dispatch",
        "memory",
        "fault",
        "x87",
        "table_store",
        "far_memory",
        "far_jump",
    ] {
        cc.arg(root.join(format!("runtime/gamepkg_x86_{name}.c")));
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
