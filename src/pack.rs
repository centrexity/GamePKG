use crate::{
    archive::Package,
    verify::{tree_hash, Envelope},
};
use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
// Preserve policy-error messages while identifying filesystem failures on hosts
// with narrower file/publication permissions (including Android).
fn io_stage<T>(result: Result<T>, stage: &'static str) -> Result<T> {
    result.map_err(|error| {
        if error.chain().any(|cause| cause.is::<std::io::Error>()) {
            error.context(stage)
        } else {
            error
        }
    })
}
fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir).context("enumerate package input directory")? {
        let entry = entry.context("read package input directory entry")?;
        let kind = entry.file_type().context("inspect package input entry")?;
        if kind.is_symlink() {
            bail!("symlinks are forbidden");
        }
        if kind.is_dir() {
            collect(root, &entry.path(), files)?;
        } else if kind.is_file() {
            let name = entry
                .path()
                .strip_prefix(root)?
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non UTF-8 path"))?
                .replace('\\', "/");
            if !crate::archive::valid_path(&name) || name == "signature.toml" {
                bail!("invalid input path");
            }
            files.push(name);
        } else {
            bail!("special files are forbidden");
        }
    }
    Ok(())
}
pub fn pack(dir: &Path, out: &Path, seed: &[u8; 32]) -> Result<()> {
    let root = fs::canonicalize(dir).context("resolve package source directory")?;
    let parent = fs::canonicalize(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .context("resolve package output directory")?;
    if parent.starts_with(&root) {
        bail!("output must be outside package directory");
    }
    if out.exists() {
        bail!("output already exists");
    }
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    files.sort();
    // Validate in a temporary sibling; publish only after all checks pass.
    let temp = out.with_extension("gamepkg.partial");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .context("create staged package file")?;
    let result = (|| -> Result<()> {
        let mut zip = ZipWriter::new(file);
        let mut total = 0u64;
        let key = SigningKey::from_bytes(seed);
        for name in files {
            let input = fs::File::open(root.join(&name)).context("open package input file")?;
            if input
                .metadata()
                .context("inspect package input file")?
                .len()
                > crate::archive::MAX_FILE
            {
                bail!("package file too large");
            }
            let mut data = Vec::new();
            input
                .take(crate::archive::MAX_FILE + 1)
                .read_to_end(&mut data)
                .context("read package input file")?;
            if name == "manifest.toml" {
                let mut manifest = crate::manifest::Manifest::parse(&data)?;
                let public = hex::encode(key.verifying_key().as_bytes());
                if !manifest.creator_key.is_empty() && manifest.creator_key != public {
                    bail!("manifest creator key does not match signing key");
                }
                manifest.creator_key = public;
                data = toml::to_string(&manifest)?.into_bytes();
            }
            total += data.len() as u64;
            if data.len() as u64 > crate::archive::MAX_FILE || total > crate::archive::MAX_TOTAL {
                bail!("package too large");
            }
            zip.start_file(
                name,
                SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .unix_permissions(0o644),
            )?;
            zip.write_all(&data)?;
        }
        zip.finish().context("finish unsigned staged package")?;
        let mut pkg = io_stage(Package::open(&temp), "open staged package for signing")?;
        let key = SigningKey::from_bytes(seed);
        let envelope = Envelope {
            public_key: hex::encode(key.verifying_key().as_bytes()),
            signature: hex::encode(
                key.sign(&io_stage(
                    tree_hash(&mut pkg),
                    "read staged package for signing",
                )?)
                .to_bytes(),
            ),
        };
        drop(pkg);
        let mut zip = ZipWriter::new_append(
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&temp)
                .context("reopen staged package to append signature")?,
        )?;
        zip.start_file(
            "signature.toml",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        zip.write_all(toml::to_string(&envelope)?.as_bytes())?;
        zip.finish()?;
        let mut signed = io_stage(
            Package::open(&temp),
            "open signed staged package for verification",
        )?;
        io_stage(
            crate::verify::verify(&mut signed, key.verifying_key().as_bytes()),
            "verify signed staged package",
        )?;
        drop(signed);
        publish_package(&temp, out)
            .context("publish signed package without replacing an existing file")?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

// Android SELinux forbids hard links for the shell used by device tests.
// Call the kernel directly: bionic only exposes renameat2 starting at API 30,
// while our Android runtime supports older devices. Unsupported kernels fail
// closed; never fall back to a check followed by an overwriting rename.
#[cfg(target_os = "android")]
fn publish_package(staging: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    // Both terminated paths stay alive for this synchronous syscall.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "android"))]
fn publish_package(staging: &Path, target: &Path) -> std::io::Result<()> {
    fs::hard_link(staging, target)
}

/// Produce a signed store package whose assets refer to local shared blobs.
pub fn pack_shared(dir: &Path, out: &Path, seed: &[u8; 32], library: &Path) -> Result<()> {
    let root = fs::canonicalize(dir)?;
    let parent = fs::canonicalize(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    if parent.starts_with(&root) || out.exists() {
        bail!("output must be new and outside the package directory");
    }
    let pool = crate::pool::Pool::open(library)?;
    let mut manifest_bytes = Vec::new();
    fs::File::open(root.join("manifest.toml"))?
        .take(crate::archive::MAX_FILE + 1)
        .read_to_end(&mut manifest_bytes)?;
    if manifest_bytes.len() as u64 > crate::archive::MAX_FILE {
        bail!("manifest too large");
    }
    let mut manifest = crate::manifest::Manifest::parse(&manifest_bytes)?;
    for reference in manifest.shared_assets.values() {
        pool.read(reference)?;
    }
    let mut files = Vec::new();
    collect(&root, &root, &mut files)?;
    let stage = tempfile::tempdir()?;
    let mut total = 0u64;
    for name in files {
        let file = fs::File::open(root.join(&name))?;
        let mut bytes = Vec::new();
        file.take(crate::archive::MAX_FILE + 1)
            .read_to_end(&mut bytes)?;
        total += bytes.len() as u64;
        if bytes.len() as u64 > crate::archive::MAX_FILE || total > crate::archive::MAX_TOTAL {
            bail!("package too large");
        }
        if let Some(asset) = name.strip_prefix("assets/") {
            if manifest.shared_assets.contains_key(asset) {
                bail!("shared asset also embedded in input");
            }
            manifest
                .shared_assets
                .insert(asset.into(), pool.put(&bytes)?);
        } else if name != "manifest.toml" {
            let destination = stage.path().join(name);
            fs::create_dir_all(destination.parent().unwrap())?;
            fs::write(destination, bytes)?;
        }
    }
    fs::write(
        stage.path().join("manifest.toml"),
        toml::to_string(&manifest)?,
    )?;
    pack(stage.path(), out, seed)
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    #[test]
    fn publication_is_complete_and_never_overwrites_existing_entries() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("staged");
        let target = dir.path().join("published");
        fs::write(&source, b"verified package").unwrap();
        publish_package(&source, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"verified package");
        // Recreate staging as pack's next invocation does, without modifying
        // an inode that may still have a published hard link.
        if source.exists() {
            fs::remove_file(&source).unwrap();
        }
        fs::write(&source, b"next package").unwrap();
        assert!(publish_package(&source, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"verified package");
        assert_eq!(fs::read(&source).unwrap(), b"next package");
        let directory = dir.path().join("directory");
        fs::create_dir(&directory).unwrap();
        assert!(publish_package(&source, &directory).is_err());
        assert!(directory.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn publication_never_replaces_or_follows_a_dangling_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("staged");
        let target = dir.path().join("published");
        let missing = dir.path().join("missing");
        fs::write(&source, b"verified package").unwrap();
        std::os::unix::fs::symlink(&missing, &target).unwrap();
        assert!(publish_package(&source, &target).is_err());
        assert_eq!(fs::read_link(&target).unwrap(), missing);
        assert!(!missing.exists());
        assert_eq!(fs::read(&source).unwrap(), b"verified package");
    }

    #[test]
    fn concurrent_publication_has_exactly_one_winner() {
        let dir = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let target = dir.path().join("published");
        let jobs = (*b"AB").map(|value| {
            let source = dir.path().join(format!("staged-{value}"));
            fs::write(&source, vec![value; 8192]).unwrap();
            let barrier = barrier.clone();
            let target = target.clone();
            std::thread::spawn(move || {
                barrier.wait();
                (value, publish_package(&source, &target).is_ok())
            })
        });
        let results = jobs.map(|job| job.join().unwrap());
        assert_eq!(results.iter().filter(|(_, won)| *won).count(), 1);
        let winner = results.iter().find(|(_, won)| *won).unwrap().0;
        assert_eq!(fs::read(target).unwrap(), vec![winner; 8192]);
    }
}
