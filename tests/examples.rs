use ed25519_dalek::SigningKey;
use gamepkg::{archive::Package, pack::pack, verify::verify};
use std::path::Path;
#[test]
fn checked_out_lockfiles_match_the_manifest_bytes_on_every_platform() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut examples = vec![root.join("minimal")];
    examples.extend(
        std::fs::read_dir(root.join("good"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir()),
    );
    for example in examples {
        let manifest = gamepkg::manifest::Manifest::parse(
            &std::fs::read(example.join("manifest.toml")).unwrap(),
        )
        .unwrap();
        let lock = std::fs::read(example.join("source/Cargo.lock")).unwrap();
        assert_eq!(
            gamepkg::verify::digest(&lock),
            manifest.lockfile_sha256,
            "{}: preserve LF checkout bytes",
            example.display()
        );
    }
}

#[test]
fn good_examples_are_signed_and_verified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in [
        "minimal",
        "good/counter",
        "good/byteorder-game",
        "good/modular-game",
        "good/asset-catalog",
        "good/rom-required",
        "good/collect-stars",
        "good/save-counter",
        "good/shape-playground",
        "good/tone-pad",
        "good/shader-switch",
        "good/component-playground",
        "good/sprite-playground",
        "good/png-sprite-playground",
        "good/shader-texture",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path().join("example.gamepkg");
        let seed = [13; 32];
        pack(&root.join("examples").join(name), &out, &seed).unwrap();
        let mut pkg = Package::open(&out).unwrap();
        verify(
            &mut pkg,
            SigningKey::from_bytes(&seed).verifying_key().as_bytes(),
        )
        .unwrap();
        if name.ends_with("asset-catalog") {
            assert_eq!(pkg.asset("levels/first.txt").unwrap(), b"level = 1\n");
        }
    }
}
#[test]
fn bad_examples_fail_for_the_expected_reason() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (name, reason) in [
        ("filesystem", "forbidden construct"),
        ("network", "forbidden construct"),
        ("include-bytes", "forbidden construct"),
        ("build-script", "Rust source files"),
        ("proc-macro", "library configuration"),
        ("unknown-crate", "allowlist"),
        ("non-slang", "only Slang"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path().join("rejected.gamepkg");
        let error = pack(&root.join("examples/bad").join(name), &out, &[13; 32])
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{name}: {error}");
        assert!(!out.exists());
        assert!(!out.with_extension("gamepkg.partial").exists());
    }
}
