#[allow(dead_code)]
#[path = "support/adversarial.rs"]
mod helper;
#[cfg(target_os = "linux")]
use gamepkg::port_host::admission::AuditedStage;
use gamepkg::port_host::admission::CheckedSources;

#[test]
fn source_snapshot_precedes_bindings_and_rejects_signed_service_bypasses() {
    let temp = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let valid = helper::sign(&files, temp.path());
    let mut package = gamepkg::archive::Package::open(&valid).unwrap();
    let sources = CheckedSources::verify(&mut package).unwrap();
    assert_eq!(sources.identity().len(), 64);
    assert!(sources.files().any(|(name, _)| name == "source/src/lib.rs"));
    files.insert(
        "source/src/lib.rs".into(),
        b"pub fn tick(){std::fs::read(\"private\");}".to_vec(),
    );
    let invalid = helper::sign(&files, temp.path());
    let mut package = gamepkg::archive::Package::open(&invalid).unwrap();
    assert!(CheckedSources::verify(&mut package).is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn exact_object_receipt_uses_private_audited_copies_and_detects_tampering() {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
    };
    let temp = tempfile::tempdir().unwrap();
    let signed = helper::sign(&helper::fixture("examples/minimal"), temp.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&signed).unwrap()).unwrap();
    let source = temp.path().join("unit.c");
    let object = temp.path().join("unit.o");
    fs::write(&source, b"int game_unit(int x){return x+1;}").unwrap();
    let compiler = Path::new("/usr/bin/clang-18").canonicalize().unwrap();
    let nm = Path::new("/usr/bin/llvm-nm-18").canonicalize().unwrap();
    assert!(Command::new(&compiler)
        .args(["-c", "-O3", "-fno-lto"])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    let hash = gamepkg::verify::digest(&fs::read(&object).unwrap());
    let stage = AuditedStage::seal(
        &sources,
        &nm,
        &compiler,
        "x86_64-unknown-linux-gnu",
        vec!["ab".repeat(32)],
        std::slice::from_ref(&object),
    )
    .unwrap();
    let receipt = stage.receipt();
    assert_eq!(receipt.sdk, gamepkg::port_host::build_identity());
    assert_eq!(receipt.source_kit, sources.identity());
    assert_eq!(receipt.shaders, vec!["ab".repeat(32)]);
    assert_eq!(receipt.objects[0].sha256, hash);
    assert_eq!(
        receipt.compiler,
        gamepkg::verify::digest(&fs::read(&compiler).unwrap())
    );
    let bytes = serde_json::to_vec(receipt).unwrap();
    let decoded: gamepkg::port_host::admission::Receipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(receipt, &decoded);
    // Compiler-side objects are never the linker inputs after sealing.
    fs::write(&object, b"changed after audit").unwrap();
    stage.verify().unwrap();
    let private = stage.objects().unwrap().remove(0);
    assert_ne!(private, object);
    let mut permissions = fs::metadata(&private).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o600);
    fs::set_permissions(&private, permissions).unwrap();
    fs::write(private, b"tampered private input").unwrap();
    assert!(stage.objects().is_err());
    assert!(AuditedStage::seal(&sources, &nm, &compiler, "x64", Vec::new(), &[object]).is_err());
    assert!(AuditedStage::seal(
        &sources,
        &nm,
        &compiler,
        "",
        Vec::new(),
        &[PathBuf::from("missing")]
    )
    .is_err());
}
