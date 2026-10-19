use gamepkg::pool::{Pool, Reference};
use std::{fs, process::Command, sync::Arc};
#[test]
fn duplicate_assets_are_atomic_and_corrupt_blobs_are_not_replaced() {
    let root = tempfile::tempdir().unwrap();
    let pool = Arc::new(Pool::open(root.path()).unwrap());
    let jobs: Vec<_> = (0..8)
        .map(|_| {
            let pool = pool.clone();
            std::thread::spawn(move || pool.put(b"shared content").unwrap())
        })
        .collect();
    let references: Vec<_> = jobs.into_iter().map(|job| job.join().unwrap()).collect();
    assert!(references
        .iter()
        .all(|reference| reference == &references[0]));
    assert_eq!(
        fs::read_dir(root.path().join("asset-pool"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(pool.read(&references[0]).unwrap(), b"shared content");
    let path = root
        .path()
        .join("asset-pool")
        .join(format!("{}.blob", references[0].sha256));
    fs::write(&path, b"bad content!!!").unwrap();
    assert!(pool.read(&references[0]).is_err());
    assert!(pool.put(b"shared content").is_err());
    assert_eq!(fs::read(path).unwrap(), b"bad content!!!");
    for digest in ["../escape", "A", &"A".repeat(64), &"g".repeat(64)] {
        assert!(pool
            .read(&Reference {
                sha256: digest.into(),
                size: 0
            })
            .is_err());
    }
    let empty = pool.put(b"").unwrap();
    assert!(pool.read(&empty).unwrap().is_empty());
    assert!(Reference {
        sha256: empty.sha256,
        size: gamepkg::archive::MAX_FILE + 1
    }
    .validate()
    .is_err());
}
#[test]
fn cli_import_deduplicates_signed_packages_and_preserves_sideloaded_assets() {
    let root = tempfile::tempdir().unwrap();
    let library = root.path().join("library");
    let exe = env!("CARGO_BIN_EXE_gamepkg");
    let mut reference = None;
    for seed in [[1; 32], [2; 32]] {
        let path = root.path().join(format!("{}.gamepkg", seed[0]));
        gamepkg::pack::pack(
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
            &path,
            &seed,
        )
        .unwrap();
        let result = Command::new(exe)
            .env("GAMEPKG_HOME", &library)
            .args(["pool-import"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = String::from_utf8(result.stdout).unwrap();
        let parts: Vec<_> = output.split_whitespace().collect();
        let current = Reference {
            sha256: parts[0].into(),
            size: parts[1].parse().unwrap(),
        };
        if let Some(previous) = &reference {
            assert_eq!(previous, &current);
        }
        reference = Some(current);
        let mut package = gamepkg::archive::Package::open(&path).unwrap();
        gamepkg::verify::verify_embedded(&mut package).unwrap();
        assert_eq!(
            package.asset("hello.txt").unwrap(),
            b"Hello from an archived asset!\n"
        );
    }
    assert_eq!(fs::read_dir(library.join("asset-pool")).unwrap().count(), 1);
    let reference = reference.unwrap();
    let output = root.path().join("copied.txt");
    let result = Command::new(exe)
        .env("GAMEPKG_HOME", &library)
        .args([
            "pool-read",
            &reference.sha256,
            "--size",
            &reference.size.to_string(),
            "-o",
        ])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read(output).unwrap(),
        b"Hello from an archived asset!\n"
    );
    let unsigned = root.path().join("invalid.gamepkg");
    fs::write(&unsigned, b"not signed").unwrap();
    assert!(Pool::open(&library)
        .unwrap()
        .import_package(&unsigned)
        .is_err());
    assert_eq!(fs::read_dir(library.join("asset-pool")).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn symlinked_pool_and_blob_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    symlink(external.path(), root.path().join("asset-pool")).unwrap();
    assert!(Pool::open(root.path()).is_err());
    fs::remove_file(root.path().join("asset-pool")).unwrap();
    let pool = Pool::open(root.path()).unwrap();
    let reference = pool.put(b"content").unwrap();
    let path = root
        .path()
        .join("asset-pool")
        .join(format!("{}.blob", reference.sha256));
    fs::remove_file(&path).unwrap();
    let target = external.path().join("target");
    fs::write(&target, b"content").unwrap();
    symlink(&target, &path).unwrap();
    assert!(pool.read(&reference).is_err());
    assert!(pool.put(b"content").is_err());
    assert_eq!(fs::read(target).unwrap(), b"content");
}
