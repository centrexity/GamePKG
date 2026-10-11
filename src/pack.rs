use crate::{
    archive::Package,
    verify::{tree_hash, Envelope},
};
use anyhow::{bail, Result};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
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
    let root = fs::canonicalize(dir)?;
    let parent = fs::canonicalize(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
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
        .open(&temp)?;
    let result = (|| -> Result<()> {
        let mut zip = ZipWriter::new(file);
        let mut total = 0u64;
        let key = SigningKey::from_bytes(seed);
        for name in files {
            let input = fs::File::open(root.join(&name))?;
            if input.metadata()?.len() > crate::archive::MAX_FILE {
                bail!("package file too large");
            }
            let mut data = Vec::new();
            input
                .take(crate::archive::MAX_FILE + 1)
                .read_to_end(&mut data)?;
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
        zip.finish()?;
        let mut pkg = Package::open(&temp)?;
        let key = SigningKey::from_bytes(seed);
        let envelope = Envelope {
            public_key: hex::encode(key.verifying_key().as_bytes()),
            signature: hex::encode(key.sign(&tree_hash(&mut pkg)?).to_bytes()),
        };
        drop(pkg);
        let mut zip =
            ZipWriter::new_append(fs::OpenOptions::new().read(true).write(true).open(&temp)?)?;
        zip.start_file(
            "signature.toml",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        zip.write_all(toml::to_string(&envelope)?.as_bytes())?;
        zip.finish()?;
        crate::verify::verify(&mut Package::open(&temp)?, key.verifying_key().as_bytes())?;
        // A hard link publishes atomically without replacing an existing file.
        fs::hard_link(&temp, out)?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}
