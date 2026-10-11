use std::{fs, path::Path};
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}
#[test]
fn every_source_file_is_checked_and_modules_must_resolve_unambiguously() {
    for case in [
        "unused-forbidden",
        "missing",
        "ambiguous",
        "redirect",
        "build-script",
        "non-rust",
        "invalid-utf8",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source");
        copy(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/examples/good/modular-game"
            )),
            &input,
        );
        let source = input.join("source/src");
        match case {
            "unused-forbidden" => fs::write(
                source.join("unused.rs"),
                "pub fn x() { std::fs::read(\"secret\"); }",
            )
            .unwrap(),
            "missing" => fs::remove_file(source.join("score/bytes.rs")).unwrap(),
            "ambiguous" => {
                fs::create_dir(source.join("movement")).unwrap();
                fs::write(
                    source.join("movement/mod.rs"),
                    "pub fn next(_: u32, x: u32) -> u32 { x }",
                )
                .unwrap();
            }
            "redirect" => fs::write(
                source.join("score/mod.rs"),
                "#[path=\"../../outside.rs\"] mod bytes;",
            )
            .unwrap(),
            "build-script" => fs::write(source.join("build.rs"), "fn main() {}").unwrap(),
            "non-rust" => fs::write(source.join("data.txt"), "unverified input").unwrap(),
            "invalid-utf8" => fs::write(source.join("unused.rs"), [255]).unwrap(),
            _ => unreachable!(),
        }
        let output = temp.path().join("rejected.gamepkg");
        assert!(
            gamepkg::pack::pack(&input, &output, &[7; 32]).is_err(),
            "accepted {case}"
        );
        assert!(!output.exists());
    }
}
