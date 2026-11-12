use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{decode::Decoder, lower::instruction};
use sha2::{Digest, Sha256};
fn emit(bytes: &[u8]) -> anyhow::Result<String> {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024))?;
    let x = d.linear(bytes, 0x10000, &[])?;
    anyhow::ensure!(
        x.instructions().len() == 1 && usize::from(x.instructions()[0].size) == bytes.len(),
        "incomplete approximation {bytes:x?}"
    );
    instruction(&x.instructions()[0])
}
fn original_reference() -> String {
    let original = include_str!("../runtime/approx/upstream/sse_rcp.cc");
    let mut out = String::from(
        r#"#include <cstdint>
using Bit16u=uint16_t;using Bit16s=int16_t;using Bit32u=uint32_t;using float32=uint32_t;
static_assert((-1>>1)==-1,"original reference needs arithmetic right shift");
enum softfloat_class_t {softfloat_zero,softfloat_denormal,softfloat_positive_inf,softfloat_negative_inf,softfloat_SNaN,softfloat_QNaN,softfloat_normalized};
static softfloat_class_t f32_class(uint32_t v){uint32_t e=v&0x7f800000,f=v&0x7fffff;if(!e)return f?softfloat_denormal:softfloat_zero;if(e!=0x7f800000)return softfloat_normalized;if(f)return v&0x400000?softfloat_QNaN:softfloat_SNaN;return v>>31?softfloat_negative_inf:softfloat_positive_inf;}
static bool f32_sign(uint32_t v){return v>>31;}static uint32_t f32_fraction(uint32_t v){return v&0x7fffff;}static int16_t f32_exp(uint32_t v){return (v>>23)&255;}
static uint32_t packFloat32(bool s,int e,uint32_t f){return ((uint32_t)s<<31)+((uint32_t)e<<23)+f;}
static uint32_t convert_to_QNaN(uint32_t v){return v|0x400000;}
#define FLOAT32_EXP_BIAS 127
#define float32_default_nan 0xffc00000u
"#,
    );
    for name in ["rcp_table[", "rsqrt_table0[", "rsqrt_table1["] {
        let start = original.find(name).unwrap();
        let begin = original[..start].rfind("static ").unwrap();
        let end = start + original[start..].find("\n};").unwrap() + 3;
        out.push_str(&original[begin..end]);
        out.push('\n');
    }
    for name in ["approximate_rcp", "approximate_rsqrt"] {
        let start = original
            .find(&format!("float32 {name}(float32 op)"))
            .unwrap();
        let body = start + original[start..].find('{').unwrap();
        let mut depth = 0;
        let mut end = None;
        for (offset, b) in original.as_bytes()[body..].iter().enumerate() {
            if *b == b'{' {
                depth += 1;
            } else if *b == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(body + offset + 1);
                    break;
                }
            }
        }
        out.push_str(&original[start..end.unwrap()]);
        out.push('\n');
    }
    out.push_str("extern \"C\" uint32_t original_approx(uint32_t v,uint32_t kind){return kind?approximate_rsqrt(v):approximate_rcp(v);}\n");
    out
}
fn fixture() -> String {
    let mut c = String::from("#include <stdint.h>\n");
    if let Some(h) = std::env::var_os("GAMEPKG_TAKEDAWN_RUNTIME_HEADER") {
        let h = h.to_str().unwrap();
        assert!(!h.contains(['\"', '\n', '\r', '\\']));
        c.push_str(&format!("#include \"{h}\"\n"));
    } else {
        c.push_str("typedef struct {uint32_t u[4];} RecompXmm;\n");
    }
    c.push_str(include_str!("approx_fixture.c"));
    let mut w = String::new();
    let mut names = vec![];
    let mut forms = vec![];
    for kind in 0..2 {
        for lanes in [1, 4] {
            for mode in 0..5 {
                let mut b = vec![];
                if lanes == 1 {
                    b.push(0xf3);
                }
                if mode == 2 {
                    b.push(0x64);
                }
                if mode == 3 {
                    b.push(0x67);
                }
                b.extend_from_slice(&[
                    0x0f,
                    if kind == 0 { 0x53 } else { 0x52 },
                    match mode {
                        0 => 0xc1,
                        4 => 0xc0,
                        _ => 0x00,
                    },
                ]);
                let code = emit(&b).unwrap();
                let name = format!("instruction_{}", names.len());
                w.push_str(&format!(
                    "static void {name}(void){{uint32_t ebp=g_ebp;(void)ebp;{code}++completed;}}\n"
                ));
                names.push(name);
                forms.push(format!("{{{kind},{lanes},{mode}}}"));
            }
        }
    }
    c.replace("/* WRAPPERS */",&format!("{w}\nstatic void (*instructions[])(void)={{{}}};\nstatic const unsigned forms[][3]={{{}}};",names.join(","),forms.join(",")))
}
#[test]
fn pinned_approximation_sources_and_typed_rejections() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime/approx");
    let lock: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("upstream-lock.json")).unwrap()).unwrap();
    assert_eq!(lock["revision"], "b0c88410501f48bbe1a966cab1c0592fb504915a");
    assert_eq!(lock["files"].as_array().unwrap().len(), 2);
    for f in lock["files"].as_array().unwrap() {
        assert_eq!(
            hex::encode(Sha256::digest(
                std::fs::read(root.join(f["path"].as_str().unwrap())).unwrap()
            )),
            f["sha256"].as_str().unwrap()
        );
    }
    for b in [
        &[0x66, 0x0f, 0x52, 0xc1][..],
        &[0xf2, 0x0f, 0x53, 0xc1],
        &[0xf3, 0xf3, 0x0f, 0x52, 0xc1],
        &[0x65, 0xf3, 0x0f, 0x52, 0x00],
    ] {
        assert!(emit(b).is_err());
    }
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    let x = d.linear(&[0xf3, 0x0f, 0x52, 0xc1], 0x10000, &[]).unwrap();
    let mut bad = x.instructions()[0].clone();
    bad.mnemonic = "rcpss".into();
    assert!(instruction(&bad).is_err());
    for index in 0..2 {
        let mut bad = x.instructions()[0].clone();
        bad.operands[index].expr = Some("bad".into());
        assert!(instruction(&bad).is_err());
    }
}
#[test]
fn export_approximation_fixtures() {
    let c = fixture();
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_APPROX_FIXTURE") {
        std::fs::write(p, c).unwrap();
    }
    if let Some(p) = std::env::var_os("GAMEPKG_TAKEDAWN_APPROX_REFERENCE") {
        std::fs::write(p, original_reference()).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
fn approximation_profile_accuracy_state_and_emission() {
    let tmp = tempfile::tempdir().unwrap();
    let r = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("runtime");
    let source = tmp.path().join("approx.c");
    let reference = tmp.path().join("original.cc");
    let object = tmp.path().join("original.o");
    let binary = tmp.path().join("approx");
    std::fs::write(&source, fixture()).unwrap();
    std::fs::write(&reference, original_reference()).unwrap();
    let out = std::process::Command::new("c++")
        .args(["-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-c"])
        .arg(reference)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
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
            "-DGAMEPKG_APPROX_ORACLE=1",
        ])
        .arg("-I")
        .arg(&r)
        .arg(source)
        .arg(object)
        .arg(r.join("gamepkg_x86_approx.c"))
        .arg(r.join("approx/gamepkg_approx.c"))
        .arg(r.join("gamepkg_x86_fault.c"))
        .arg("-lgmp")
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
fn decoder_retains_valid_approximation_prefix_orders() {
    let d = Decoder::new(&Budget::new(16 * 1024 * 1024)).unwrap();
    for opcode in [0x52, 0x53] {
        let canonical = d
            .linear(&[0x64, 0x67, 0xf3, 0x0f, opcode, 0x00], 0x12340, &[])
            .unwrap();
        assert_eq!(canonical.instructions().len(), 1);
        for prefixes in [
            [0x64, 0x67, 0xf3],
            [0x64, 0xf3, 0x67],
            [0x67, 0x64, 0xf3],
            [0x67, 0xf3, 0x64],
            [0xf3, 0x64, 0x67],
            [0xf3, 0x67, 0x64],
        ] {
            let mut bytes = prefixes.to_vec();
            bytes.extend_from_slice(&[0x0f, opcode, 0x00]);
            let x = d.linear(&bytes, 0x12340, &[]).unwrap();
            assert_eq!(x.instructions().len(), 1, "{bytes:x?}");
            let i = &x.instructions()[0];
            assert_eq!(i.address, 0x12340);
            assert_eq!(i.bytes_hex, hex::encode(&bytes));
            assert_eq!(i.size, 6);
            assert_eq!(i.operands, canonical.instructions()[0].operands);
            assert_eq!(i.mnemonic, canonical.instructions()[0].mnemonic);
            assert!(instruction(i).is_ok());
        }
    }
}
