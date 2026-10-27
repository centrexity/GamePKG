use std::path::Path;

#[test]
fn host_sources_do_not_name_a_reviewed_engine() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let needles = [
        "OpenCE", "opence", "HALO_", "ui.map", ".xiso", "SDL3", "halo.exe",
    ];
    for entry in walk(&root) {
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        for needle in needles {
            assert!(!text.contains(needle), "{} names {needle}", entry.display());
        }
    }
}

#[test]
fn host_header_in_the_engine_patch_matches_the_canonical_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir().unwrap();
    let header = dir.path().join("port/include/gamepkg_host.h");
    std::fs::create_dir_all(header.parent().unwrap()).unwrap();
    let introduced =
        std::fs::read_to_string(root.join("engines/halo/patches/0002-gamepkg-host-log.patch"))
            .unwrap();
    std::fs::write(
        &header,
        extract_new_file(&introduced, "port/include/gamepkg_host.h"),
    )
    .unwrap();
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args([
            "apply",
            "--whitespace=nowarn",
            "--include=port/include/gamepkg_host.h",
        ])
        .arg(root.join("engines/halo/patches/0007-gamepkg-clock.patch"))
        .status()
        .unwrap();
    assert!(status.success());
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args([
            "apply",
            "--whitespace=nowarn",
            "--include=port/include/gamepkg_host.h",
        ])
        .arg(root.join("engines/halo/patches/0009-gamepkg-threads.patch"))
        .status()
        .unwrap();
    assert!(status.success());
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .args([
            "apply",
            "--whitespace=nowarn",
            "--include=port/include/gamepkg_host.h",
        ])
        .arg(root.join("engines/halo/patches/0010-gamepkg-directory.patch"))
        .status()
        .unwrap();
    assert!(status.success());
    let canonical = std::fs::read_to_string(root.join("include/gamepkg_host.h")).unwrap();
    assert_eq!(std::fs::read_to_string(&header).unwrap(), canonical);
}

fn extract_new_file(patch: &str, path: &str) -> String {
    let marker = format!("+++ b/{path}\n");
    let start = patch.find(&marker).expect("patch file") + marker.len();
    let mut out = String::new();
    let mut in_hunk = false;
    for line in patch[start..].lines() {
        if line.starts_with("diff --git ") {
            break;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if in_hunk {
            if let Some(body) = line.strip_prefix('+') {
                out.push_str(body);
                out.push('\n');
            }
        }
    }
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}
