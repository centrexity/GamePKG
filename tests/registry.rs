use gamepkg::{archive::Package, pack::pack, registry, verify::verify_embedded};
use std::{fs, path::Path};
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}
fn package(temp: &Path, name: &str, version: &str, seed: &[u8; 32]) -> std::path::PathBuf {
    let dir = temp.join(name);
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &dir,
    );
    let manifest = dir.join("manifest.toml");
    let source = fs::read_to_string(&manifest)
        .unwrap()
        .replace("version = \"0.1.0\"", &format!("version = \"{version}\""));
    fs::write(&manifest, source).unwrap();
    let out = temp.join(format!("{name}.gamepkg"));
    pack(&dir, &out, seed).unwrap();
    out
}
#[test]
fn embedded_key_install_list_and_update_continuity() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("installed");
    let original = package(t.path(), "original", "0.1.0", &[5; 32]);
    let m = verify_embedded(&mut Package::open(&original).unwrap()).unwrap();
    assert_eq!(
        m.creator_key,
        hex::encode(
            ed25519_dalek::SigningKey::from_bytes(&[5; 32])
                .verifying_key()
                .as_bytes()
        )
    );
    assert!(registry::install(&root, &original, false)
        .unwrap_err()
        .to_string()
        .contains("approval"));
    assert!(registry::list(&root).unwrap().is_empty());
    registry::install(&root, &original, true).unwrap();
    assert_eq!(registry::list(&root).unwrap()[0].version, "0.1.0");
    assert!(registry::install(&root, &original, true)
        .unwrap_err()
        .to_string()
        .contains("higher version"));
    let imposter = package(t.path(), "imposter", "9.0.0", &[6; 32]);
    assert!(registry::install(&root, &imposter, true)
        .unwrap_err()
        .to_string()
        .contains("creator key differs"));
    let next = package(t.path(), "next", "0.2.0", &[5; 32]);
    registry::install(&root, &next, false).unwrap();
    assert_eq!(registry::list(&root).unwrap()[0].version, "0.2.0");
    assert!(registry::install(&root, &original, true).is_err());
    assert_eq!(registry::list(&root).unwrap()[0].version, "0.2.0");
    assert!(!root.join(".install-lock").exists());
    assert!(registry::update(&root, "com.gamepkg.minimal")
        .unwrap_err()
        .to_string()
        .contains("no update URL"));
}
#[test]
fn manifest_key_mismatch_and_unsafe_urls_are_rejected() {
    for url in [
        "http://example.com/game.gamepkg",
        "file:///etc/passwd",
        "https://",
        "https://user:pass@example.com/game.gamepkg",
        "https://example.com/ bad",
    ] {
        assert!(registry::validate_url(url).is_err());
        assert!(registry::download(url).is_err());
    }
    assert!(registry::validate_url("https://example.com/latest.gamepkg").is_ok());
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("input");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &dir,
    );
    let p = dir.join("manifest.toml");
    let source = fs::read_to_string(&p).unwrap();
    fs::write(
        &p,
        format!("creator_key = \"{}\"\n{source}", hex::encode([1; 32])),
    )
    .unwrap();
    assert!(pack(&dir, &t.path().join("bad.gamepkg"), &[5; 32])
        .unwrap_err()
        .to_string()
        .contains("does not match"));
    fs::write(
        &p,
        format!("update_url = \"http://example.com/latest.gamepkg\"\n{source}"),
    )
    .unwrap();
    assert!(pack(&dir, &t.path().join("bad.gamepkg"), &[5; 32]).is_err());
}
#[test]
fn cli_installs_and_lists_without_public_key_entry() {
    use std::process::Command;
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("library");
    let path = package(t.path(), "cli", "0.1.0", &[5; 32]);
    let exe = env!("CARGO_BIN_EXE_gamepkg");
    assert!(Command::new(exe)
        .arg("verify")
        .arg(&path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(!Command::new(exe)
        .env("GAMEPKG_HOME", &root)
        .arg("install")
        .arg(&path)
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new(exe)
        .env("GAMEPKG_HOME", &root)
        .arg("install")
        .arg(&path)
        .arg("--trust-creator")
        .output()
        .unwrap()
        .status
        .success());
    let listed = Command::new(exe)
        .env("GAMEPKG_HOME", &root)
        .arg("list")
        .output()
        .unwrap();
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains("com.gamepkg.minimal 0.1.0"));
}

#[test]
fn rom_requirement_is_signed_and_build_cache_tracks_exact_package() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("rom-game");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &dir,
    );
    let p = dir.join("manifest.toml");
    let original = fs::read_to_string(&p).unwrap();
    fs::write(&p, format!("requires_rom = true\n{original}")).unwrap();
    let archive = t.path().join("rom.gamepkg");
    pack(&dir, &archive, &[5; 32]).unwrap();
    let root = t.path().join("library");
    let m = registry::install(&root, &archive, true).unwrap();
    assert!(m.requires_rom);
    assert!(!gamepkg::build::is_built(&root, &m.id).unwrap());
    let cache = root
        .join("builds")
        .join(gamepkg::verify::digest(&fs::read(&archive).unwrap()));
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("game.wasm"), b"bad").unwrap();
    assert!(!gamepkg::build::is_built(&root, &m.id).unwrap());
    fs::write(cache.join("game.wasm"), b"\0asm\x01\0\0\0").unwrap();
    assert!(!gamepkg::build::is_built(&root, &m.id).unwrap());
    let playable = wat::parse_str(
        "(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) local.get 1))",
    )
    .unwrap();
    fs::write(cache.join("game.wasm"), &playable).unwrap();
    gamepkg::integrity::record_build(&cache.join("game.wasm"), &playable).unwrap();
    assert!(gamepkg::build::is_built(&root, &m.id).unwrap());
    fs::write(
        &p,
        format!(
            "requires_rom = true\n{}",
            original.replace("version = \"0.1.0\"", "version = \"0.2.0\"")
        ),
    )
    .unwrap();
    let next = t.path().join("next.gamepkg");
    pack(&dir, &next, &[5; 32]).unwrap();
    registry::install(&root, &next, false).unwrap();
    assert!(!gamepkg::build::is_built(&root, &m.id).unwrap());
}

#[cfg(feature = "gui")]
#[test]
fn example_thumbnail_decodes_for_the_library_table() {
    let bytes =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal/thumbnail.png"))
            .unwrap();
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).unwrap();
}

#[test]
fn onion_urls_allow_http_only_for_v3_onion_hosts() {
    let service = "a".repeat(56);
    for value in [
        format!("http://{service}.onion/latest.gamepkg"),
        format!("https://{service}.onion/latest.gamepkg"),
        format!("http://{service}.onion./latest.gamepkg"),
        format!("http://assets.{service}.onion/latest.gamepkg"),
    ] {
        registry::validate_url(&value).unwrap();
    }
    for value in [
        "http://example.com/latest.gamepkg",
        "http://short.onion/latest.gamepkg",
        "ftp://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.onion/file",
        "http://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1.onion/file",
        "http://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.onion.evil.test/file",
        "https://user@example.com/file",
    ] {
        assert!(registry::validate_url(value).is_err(), "accepted {value}");
    }
}

#[test]
fn rejected_package_does_not_hide_valid_games_but_strict_listing_still_fails() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &source,
    );
    let package = temp.path().join("game.gamepkg");
    pack(&source, &package, &[9; 32]).unwrap();
    let root = temp.path().join("library");
    let installed = registry::install(&root, &package, true).unwrap();
    fs::write(root.join("broken.gamepkg"), b"not an archive").unwrap();
    fs::copy(&package, root.join("wrong-id.gamepkg")).unwrap();
    let report = registry::inspect_library(&root).unwrap();
    assert_eq!(report.games.len(), 1);
    assert_eq!(report.games[0].id, installed.id);
    assert_eq!(report.rejected.len(), 2);
    assert_eq!(report.rejected[0].0, "broken.gamepkg");
    assert!(report.rejected[1].1.contains("ID mismatch"));
    assert!(registry::list(&root).is_err());
    assert_eq!(
        registry::installed_manifest(&root, &installed.id)
            .unwrap()
            .id,
        installed.id
    );
    for id in ["../escape", "broken", "wrong-id"] {
        assert!(registry::installed_manifest(&root, id).is_err());
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gamepkg"))
        .env("GAMEPKG_HOME", &root)
        .args(["list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("broken.gamepkg — Unavailable"));
    assert!(text.contains("com.gamepkg.minimal 0.1.0"));
}

#[cfg(unix)]
#[test]
fn github_project_download_installs_only_verified_release_packages() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let temp = tempfile::tempdir().unwrap();
    let archive = package(temp.path(), "release", "0.1.0", &[31; 32]);
    let tools = temp.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let script = tools.join("curl");
    fs::write(&script, r#"#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
out = pathlib.Path(args[args.index('--output') + 1])
url = args[args.index('--url') + 1]
if url == 'https://api.github.com/repos/creator/game/releases/latest':
    out.write_text(json.dumps({'draft': False, 'assets': [{'name': 'game.gamepkg', 'size': pathlib.Path(os.environ['TEST_PACKAGE']).stat().st_size, 'browser_download_url': 'https://github.com/creator/game/releases/download/v1/game.gamepkg'}]}))
elif url == 'https://github.com/creator/game/releases/download/v1/game.gamepkg':
    out.write_bytes(pathlib.Path(os.environ['TEST_PACKAGE']).read_bytes())
else:
    sys.exit(23)
print('200', end='')
"#).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", tools.display(), std::env::var("PATH").unwrap());
    let root = temp.path().join("library");
    let install = || {
        Command::new(env!("CARGO_BIN_EXE_gamepkg"))
            .env("PATH", &path)
            .env("TEST_PACKAGE", &archive)
            .env("GAMEPKG_HOME", &root)
            .args([
                "install",
                "https://github.com/creator/game",
                "--trust-creator",
            ])
            .output()
            .unwrap()
    };
    let result = install();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(registry::list(&root).unwrap().len(), 1);
    fs::remove_dir_all(&root).unwrap();
    fs::write(&archive, b"unsigned release asset").unwrap();
    assert!(!install().status.success());
    assert!(registry::list(&root).unwrap().is_empty());
}

// Model an authentic package from an older host that allowed Rust macros.
// Signing happens outside pack(), because the current packer must reject it.
fn legacy_archive(path: &Path, source: &[u8], seed: &[u8; 32], resign: bool) {
    use ed25519_dalek::Signer;
    let mut original = Package::open(path).unwrap();
    let mut entries = std::collections::BTreeMap::new();
    for name in original.entries.keys().cloned().collect::<Vec<_>>() {
        entries.insert(name.clone(), original.read(&name).unwrap());
    }
    entries.insert("source/src/lib.rs".into(), source.to_vec());
    fn write(path: &Path, entries: &std::collections::BTreeMap<String, Vec<u8>>) {
        use std::io::Write;
        let mut archive = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in entries {
            archive
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(bytes).unwrap();
        }
        archive.finish().unwrap();
    }
    drop(original);
    write(path, &entries);
    if resign {
        let digest = gamepkg::verify::tree_hash(&mut Package::open(path).unwrap()).unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(seed);
        let envelope = gamepkg::verify::Envelope {
            public_key: hex::encode(key.verifying_key().as_bytes()),
            signature: hex::encode(key.sign(&digest).to_bytes()),
        };
        entries.insert(
            "signature.toml".into(),
            toml::to_string(&envelope).unwrap().into_bytes(),
        );
        write(path, &entries);
    }
}

#[test]
fn policy_rejected_legacy_packages_can_only_upgrade_to_verified_same_creator_source() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    let old = package(temp.path(), "legacy", "0.1.0", &[5; 32]);
    let source = b"pub fn game_tick(input: u32, state: u64) -> u64 { assert!(input < 256); state }";
    legacy_archive(&old, source, &[5; 32], true);
    assert!(verify_embedded(&mut Package::open(&old).unwrap()).is_err());
    assert!(registry::install(&root, &old, true).is_err());
    // A previous host installed these signed bytes; the current host quarantines them.
    fs::create_dir_all(root.join("saves")).unwrap();
    let installed = root.join("com.gamepkg.minimal.gamepkg");
    fs::copy(&old, &installed).unwrap();
    fs::write(root.join("saves/retained"), b"player progress").unwrap();
    assert!(registry::installed_manifest(&root, "com.gamepkg.minimal").is_err());
    assert!(registry::list(&root).is_err());
    let quarantined = registry::inspect_library(&root).unwrap();
    assert!(quarantined.games.is_empty());
    assert_eq!(quarantined.rejected.len(), 1);
    let original = fs::read(&installed).unwrap();
    let unsafe_next = package(temp.path(), "unsafe-next", "0.2.0", &[5; 32]);
    legacy_archive(&unsafe_next, source, &[5; 32], true);
    assert!(registry::install(&root, &unsafe_next, false).is_err());
    let imposter = package(temp.path(), "legacy-imposter", "9.0.0", &[6; 32]);
    assert!(registry::install(&root, &imposter, true).is_err());
    assert_eq!(fs::read(&installed).unwrap(), original);
    let next = package(temp.path(), "safe-next", "0.2.0", &[5; 32]);
    registry::install(&root, &next, false).unwrap();
    assert_eq!(
        registry::installed_manifest(&root, "com.gamepkg.minimal")
            .unwrap()
            .version,
        "0.2.0"
    );
    assert_eq!(
        fs::read(root.join("saves/retained")).unwrap(),
        b"player progress"
    );
    // An altered old archive cannot establish the creator identity for recovery.
    fs::copy(&old, &installed).unwrap();
    legacy_archive(
        &installed,
        b"pub fn game_tick(input: u32, state: u64) -> u64 { assert!(input < 512); state }",
        &[5; 32],
        false,
    );
    let tampered = fs::read(&installed).unwrap();
    assert!(registry::install(&root, &next, false).is_err());
    assert_eq!(fs::read(&installed).unwrap(), tampered);
}
