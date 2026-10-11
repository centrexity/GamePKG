use gamepkg::catalog::{self, Catalog};
use std::{fs, process::Command};
fn list(id: &str, url: &str) -> String {
    format!("format_version = 1\n[[games]]\nid = \"{id}\"\ntitle = \"Example Game\"\nversion = \"1.0.0\"\ndownload_url = \"{url}\"\nrequires_rom = true\n")
}
#[test]
fn import_is_offline_persistent_and_idempotent() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("library");
    let input = t.path().join("games.gamepkglist");
    fs::write(
        &input,
        list(
            "com.example.game",
            "https://unreachable.invalid/game.gamepkg",
        ),
    )
    .unwrap();
    assert_eq!(catalog::import(&root, &input).unwrap(), 1);
    assert_eq!(catalog::import(&root, &input).unwrap(), 0);
    let entries = catalog::entries(&root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].requires_rom, Some(true));
    assert!(gamepkg::registry::list(&root).unwrap().is_empty());
    assert!(!root.join("com.example.game.gamepkg").exists());
    assert!(!root.join(".catalog-lock").exists());
    let result = Command::new(env!("CARGO_BIN_EXE_gamepkg"))
        .env("GAMEPKG_HOME", &root)
        .arg("list")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("Not downloaded"));
}
#[test]
fn malformed_and_conflicting_lists_leave_library_unchanged() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("library");
    let input = t.path().join("games.gamepkglist");
    let initial = list(
        "com.example.game",
        "https://example.invalid/original.gamepkg",
    );
    fs::write(&input, &initial).unwrap();
    catalog::import(&root, &input).unwrap();
    let before = fs::read(root.join("catalog.gamepkglist")).unwrap();
    for bad in [
        initial.replace("format_version = 1","format_version = 9"),
        initial.replace("com.example.game","../escape"),
        initial.replace("https://example.invalid/original.gamepkg","file:///etc/passwd"),
        initial.replace("https://example.invalid/original.gamepkg","https://example.invalid/replaced.gamepkg"),
        format!("{initial}\n[[games]]\nid='com.example.game'\ntitle='Duplicate'\ndownload_url='https://example.invalid/a'\n"),
        format!("{initial}\nunknown_field=true\n"),
    ] {
        fs::write(&input,bad).unwrap();assert!(catalog::import(&root,&input).is_err());
        assert_eq!(fs::read(root.join("catalog.gamepkglist")).unwrap(),before);
    }
    assert!(Catalog::parse(&vec![b'x'; 4 * 1024 * 1024 + 1]).is_err());
}
#[test]
fn cli_imports_many_games_without_download_or_creator_trust() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("library");
    let input = t.path().join("many.gamepkglist");
    let mut data = String::from("format_version=1\n");
    for i in 0..100 {
        data+=&format!("[[games]]\nid='com.example.game{i}'\ntitle='Game {i}'\ndownload_url='https://unreachable.invalid/{i}.gamepkg'\n");
    }
    fs::write(&input, data).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_gamepkg"))
        .env("GAMEPKG_HOME", &root)
        .arg("import-list")
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(catalog::entries(&root).unwrap().len(), 100);
    assert!(gamepkg::registry::list(&root).unwrap().is_empty());
}

#[test]
fn partial_import_does_not_publish_new_entries_before_a_conflict() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    let input = temp.path().join("games.gamepkglist");
    fs::write(
        &input,
        list("com.example.old", "https://example.invalid/original"),
    )
    .unwrap();
    catalog::import(&root, &input).unwrap();
    let mut data = list("com.example.new", "https://example.invalid/new");
    data += "[[games]]\nid='com.example.old'\ntitle='Changed'\ndownload_url='https://example.invalid/replaced'\n";
    fs::write(&input, data).unwrap();
    assert!(catalog::import(&root, &input).is_err());
    let entries = catalog::entries(&root).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "com.example.old");
}
