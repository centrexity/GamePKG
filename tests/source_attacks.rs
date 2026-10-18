#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::archive::Package;
#[cfg(unix)]
use std::fs;

#[test]
fn concealed_filesystem_calls_macros_aliases_and_callbacks_fail_even_when_signed() {
    let snippets = [
        "pub fn x() { std /* disguise */ :: fs :: read(\"secret\"); }",
        "pub fn x() { ::std::r#fs::read(\"secret\"); }",
        "use std::{fs::{self, read}};",
        "use std::fs::read as allowed; pub fn x() { allowed(\"secret\"); }",
        "use std::fs as r#safe;",
        "use std::fs::{read as _, write as _};",
        "use gamepkg as api;",
        "use gamepkg::{self as api};",
        "use gamepkg::*;",
        "use gamepkg::asset_read as read;",
        "type Fs = std::fs::File;",
        "type Alias = u32;",
        "struct X; impl X { type Alias = u32; }",
        "extern crate std as allowed;",
        "extern crate libc;",
        "pub fn x() { std::process::Command::new(\"sh\"); }",
        "pub fn x() { std::env::var(\"HOME\"); }",
        "pub fn x() { core::include_bytes!(\"/etc/passwd\"); }",
        "pub fn x() { include_str!(concat!(\"../\", \"secret\")); }",
        "macro_rules! call { ($a:ident) => { $a!(\"secret\") }; }",
        "macro_rules! escape { () => { std::fs::read(\"secret\") }; }",
        "pub fn x() { panic!(\"hidden macro\"); }",
        "pub fn x() { let _ = core::assert!(true); }",
        "pub fn x() { let _ = env!(\"HOME\"); }",
        "#[cfg(any())] pub fn x() { std::fs::read(\"secret\"); }",
        "#[cfg_attr(any(), path=\"../../secret.rs\")] mod hidden;",
        "#[path=\"../../secret.rs\"] mod hidden;",
        r##"#[path=r"C:\secret.rs"] mod hidden;"##,
        "mod a { #[path=\"../../../secret.rs\"] mod hidden; }",
        "#[allow(unused)] pub fn x() {}",
        "#[unsafe(no_mangle)] pub fn x() {}",
        "pub unsafe fn x() {}",
        "pub fn x() { unsafe { core::ptr::read(0 as *const u32); } }",
        "unsafe extern \"C\" { fn open(); }",
        "pub extern \"C\" fn x() {}",
        "pub fn x() { let f = gamepkg::asset_read; f(\"secret\",0,&mut [0;8]); }",
        "pub fn x() { (gamepkg::asset_read)(\"secret\",0,&mut [0;8]); }",
        "pub fn x() { { gamepkg::asset_read }(\"secret\",0,&mut [0;8]); }",
        "pub fn x() { let table = [gamepkg::asset_read]; table[0](\"secret\",0,&mut [0;8]); }",
        "pub fn x() { let (f,) = (gamepkg::asset_read,); f(\"secret\",0,&mut [0;8]); }",
        "pub fn x() { let f = || 1; f(); }",
        "pub fn x(f: fn()) { f(); }",
        "pub fn x(f: &dyn Fn()) { f(); }",
        "pub fn x(f: impl Fn()) { f(); }",
        "pub fn x<F: Fn()>(f: F) { f(); }",
        "trait Hidden { fn read(&self); }",
        "pub fn helper() {} pub fn x() { let callback = helper; }",
        "pub fn x() { Some(1).map(gamepkg::save_size); }",
        "pub fn x() { let mut f = None; f = Some(gamepkg::save_size); f.unwrap()(); }",
        "pub fn x() { let f = &gamepkg::save_size; (*f)(); }",
        "pub fn x() { let f = gamepkg::save_size as fn() -> Option<u32>; f(); }",
        "struct Holder { f: fn() } pub fn x(h: Holder) { (h.f)(); }",
        "mod hidden { pub fn read() { std::fs::read(\"secret\"); } }",
        "const HIDDEN: () = { unsafe {} };",
    ];
    for snippet in snippets {
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/minimal");
        // Keep a valid entry point so a compilation error cannot explain rejection.
        files.insert(
            "source/src/lib.rs".into(),
            format!("pub fn game_tick(_:u32,_:u64)->u64 {{0}}\n{snippet}").into_bytes(),
        );
        let source = temp.path().join("source");
        helper::write_fixture(&files, &source);
        let output = temp.path().join("rejected.gamepkg");
        let error = gamepkg::pack::pack(&source, &output, &helper::SEED).unwrap_err();
        assert!(
            error.to_string().contains("forbidden construct")
                && error.to_string().contains("source/src/lib.rs"),
            "wrong rejection for {snippet}: {error:#}"
        );
        assert!(!output.exists() && !output.with_extension("gamepkg.partial").exists());
        let signed = helper::sign(&files, temp.path());
        let error =
            gamepkg::verify::verify_embedded(&mut Package::open(&signed).unwrap()).unwrap_err();
        assert!(
            error.to_string().contains("forbidden construct")
                && error.to_string().contains("source/src/lib.rs"),
            "signed source accepted or wrong rejection: {snippet}: {error:#}"
        );
    }
}

#[test]
fn cargo_configuration_and_unused_source_cannot_hide_outside_the_scanned_tree() {
    for (name, data, reason) in [
        (
            "source/.cargo/config.toml",
            "[build]\nrustflags=['--cfg=escape']",
            "only Rust source files",
        ),
        (
            "source/.cargo/config",
            "[source.crates-io]\nreplace-with='evil'",
            "only Rust source files",
        ),
        ("source/build.rs", "fn main() {}", "only Rust source files"),
        (
            "source/src/build.rs",
            "fn main() {}",
            "only Rust source files",
        ),
        (
            "source/src/hidden.rs",
            "pub fn escape() { std::fs::read(\"secret\"); }",
            "forbidden construct",
        ),
        (
            "source/src/deep/nested/unused.rs",
            "pub fn escape() { include_str!(\"secret\"); }",
            "forbidden construct",
        ),
        (
            "source/src/rust-toolchain.toml",
            "channel='nightly'",
            "only Rust source files",
        ),
        (
            "source/src/unused.s",
            ".global escape",
            "only Rust source files",
        ),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/minimal");
        files.insert(name.into(), data.as_bytes().to_vec());
        let signed = helper::sign(&files, temp.path());
        let error =
            gamepkg::verify::verify_embedded(&mut Package::open(&signed).unwrap()).unwrap_err();
        assert!(error.to_string().contains(reason), "{name}: {error:#}");
    }
}

#[test]
fn shader_inclusion_cannot_read_host_files_or_load_unreviewed_modules() {
    for shader in [
        "#include \"../../secret\"",
        "#include <C:/secret>",
        "#include \"/etc/passwd\"",
        "#include_next \"secret\"",
        "#  include \"secret\"",
        "#/**/include \"secret\"",
        "#inc\\\nlude \"secret\"",
        "#inc\\\r\nlude \"secret\"",
        "#define FILE \"secret\"\n#include FILE",
        "#if 0\n#include \"secret\"\n#endif",
        "import hidden;",
        "import \"../../secret\";",
        "__include hidden;",
        "#define PASTE(a,b) a ## b\nPASTE(inc,lude)",
        "#define PASTE(a,b) a #/**/# b\nPASTE(im,port)",
    ] {
        assert!(
            gamepkg::shaders::validate_source(shader.as_bytes()).is_err(),
            "accepted shader {shader}"
        );
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/minimal");
        // Unreferenced Slang files must also be examined.
        files.insert("shaders/unused.slang".into(), shader.as_bytes().to_vec());
        let signed = helper::sign(&files, temp.path());
        let error =
            gamepkg::verify::verify_embedded(&mut Package::open(&signed).unwrap()).unwrap_err();
        assert!(format!("{error:#}").contains("shader"));
    }
    assert!(gamepkg::shaders::validate_source(
        b"#if GAMEPKG_SEPARATE_SAMPLER\nfloat safe = 1;\n#else\nfloat safe = 2;\n#endif"
    )
    .is_ok());
}

#[cfg(unix)]
#[test]
fn nested_directory_file_dangling_and_loop_symlinks_are_never_followed_by_packers() {
    use std::os::unix::fs::symlink;
    for location in [
        "assets/link",
        "shaders/link",
        "source/src/link",
        "source/link",
    ] {
        for target_kind in ["file", "directory", "dangling", "loop"] {
            let temp = tempfile::tempdir().unwrap();
            let source = temp.path().join("source");
            helper::write_fixture(&helper::fixture("examples/minimal"), &source);
            let outside = temp.path().join("outside");
            fs::create_dir(&outside).unwrap();
            let secret = outside.join("secret");
            fs::write(&secret, b"outside sentinel").unwrap();
            let link = source.join(location);
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            let target = match target_kind {
                "file" => secret.clone(),
                "directory" => outside.clone(),
                "dangling" => outside.join("missing"),
                "loop" => link.clone(),
                _ => unreachable!(),
            };
            symlink(target, &link).unwrap();
            for shared in [false, true] {
                let output = temp.path().join("rejected.gamepkg");
                let result = if shared {
                    gamepkg::pack::pack_shared(
                        &source,
                        &output,
                        &helper::SEED,
                        &temp.path().join("library"),
                    )
                } else {
                    gamepkg::pack::pack(&source, &output, &helper::SEED)
                };
                assert!(
                    result.unwrap_err().to_string().contains("symlinks"),
                    "{location}, {target_kind}, shared={shared}"
                );
                assert!(!output.exists() && !output.with_extension("gamepkg.partial").exists());
                assert_eq!(fs::read(&secret).unwrap(), b"outside sentinel");
                assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
            }
        }
    }
}
