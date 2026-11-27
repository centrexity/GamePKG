use crate::{archive::Package, manifest::Manifest};
use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub fn digest(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}
pub(crate) fn digest_reader(mut input: impl std::io::Read) -> Result<String> {
    let mut hash = Sha256::new();
    std::io::copy(&mut input, &mut hash)?;
    Ok(hex::encode(hash.finalize()))
}

pub fn tree_hash(pkg: &mut Package) -> Result<Vec<u8>> {
    let mut h = Sha256::new();
    h.update(b"GamePKG tree v1\0");
    for name in pkg
        .entries
        .keys()
        .filter(|n| n.as_str() != "signature.toml")
        .cloned()
        .collect::<Vec<_>>()
    {
        let mut entry_hash = Sha256::new();
        let size = pkg.copy_entry(&name, &mut entry_hash)?;
        h.update((name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update(size.to_le_bytes());
        h.update(entry_hash.finalize());
    }
    Ok(h.finalize().to_vec())
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub public_key: String,
    pub signature: String,
}
#[derive(Debug, Clone)]
pub(crate) struct CheckedPackage {
    pub(crate) manifest: Manifest,
    pub(crate) tree_identity: Vec<u8>,
}

pub fn verify(pkg: &mut Package, trusted: &[u8; 32]) -> Result<Manifest> {
    Ok(verify_checked(pkg, trusted)?.manifest)
}

pub(crate) fn verify_checked(pkg: &mut Package, trusted: &[u8; 32]) -> Result<CheckedPackage> {
    let checked = authenticate_contents(pkg, trusted)?;
    crate::source::scan(pkg)?;
    if checked.manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE) {
        crate::cube2::validate_recipe(pkg)?;
    }
    if checked.manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
        crate::takedawn::validate_recipe(pkg)?;
    }
    if let Some(engine) = checked
        .manifest
        .builtin_engine
        .as_deref()
        .and_then(crate::external_engine::get)
    {
        crate::external_engine::validate_recipe(engine, pkg)?;
    }
    pkg.set_shared_refs(checked.manifest.shared_assets.clone());
    pkg.mark_verified_identity(checked.tree_identity.clone());
    Ok(checked)
}

// Update recovery authenticates the old archive without activating its source.
// This deliberately does not mark the package verified for asset access or play.
pub(crate) fn verify_signed_identity(pkg: &mut Package) -> Result<Manifest> {
    let m = Manifest::parse(&pkg.read("manifest.toml")?)?;
    Ok(authenticate_contents(pkg, &decode_key(&m.creator_key)?)?.manifest)
}

fn authenticate_contents(pkg: &mut Package, trusted: &[u8; 32]) -> Result<CheckedPackage> {
    let m = Manifest::parse(&pkg.read("manifest.toml")?)?;
    for name in [
        "thumbnail.png",
        "LICENSE",
        "CREDITS.md",
        "source/Cargo.toml",
        "source/Cargo.lock",
        "source/src/lib.rs",
    ] {
        if !pkg.entries.contains_key(name) {
            bail!("missing {name}");
        }
    }
    if digest(&pkg.read("source/Cargo.lock")?) != m.lockfile_sha256 {
        bail!("lockfile digest mismatch");
    }
    let thumbnail = pkg.read("thumbnail.png")?;
    if !thumbnail.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("thumbnail must be PNG");
    }
    for name in pkg.entries.keys() {
        if name.starts_with("shaders/") && !name.ends_with(".slang") {
            bail!("only Slang shader sources allowed");
        }
        if !matches!(
            name.as_str(),
            "manifest.toml"
                | "signature.toml"
                | "thumbnail.png"
                | "LICENSE"
                | "NOTICE"
                | "CREDITS.md"
                | "preview.av1.webm"
                | "preview.ogv"
        ) && !["source/", "shaders/", "assets/"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            bail!("unexpected package file: {name}");
        }
    }
    for name in pkg
        .entries
        .keys()
        .filter(|name| name.starts_with("shaders/"))
        .cloned()
        .collect::<Vec<_>>()
    {
        crate::shaders::validate_source(&pkg.read(&name)?)
            .with_context(|| format!("invalid shader source: {name}"))?;
    }
    let env: Envelope = toml::from_str(std::str::from_utf8(&pkg.read("signature.toml")?)?)?;
    if m.creator_key.is_empty() || decode_key(&m.creator_key)? != *trusted {
        bail!("manifest creator key does not match trusted signer");
    }
    if hex::decode(env.public_key)? != trusted {
        bail!("signer is not trusted");
    }
    let sig = Signature::from_slice(&hex::decode(env.signature)?)?;
    let tree_identity = tree_hash(pkg)?;
    VerifyingKey::from_bytes(trusted)?
        .verify_strict(&tree_identity, &sig)
        .context("invalid package signature")?;
    for name in ["preview.av1.webm", "preview.ogv"] {
        if pkg.entries.contains_key(name) {
            crate::previews::validate(name, &pkg.read(name)?)?;
        }
    }
    for path in m.shared_assets.keys() {
        if pkg.entries.contains_key(&format!("assets/{path}")) {
            bail!("shared asset also embedded in archive");
        }
    }
    Ok(CheckedPackage {
        manifest: m,
        tree_identity,
    })
}

pub fn decode_key(s: &str) -> Result<[u8; 32]> {
    hex::decode(s.trim())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("public key must be 32 bytes"))
}
pub fn verify_embedded(pkg: &mut Package) -> Result<Manifest> {
    let m = Manifest::parse(&pkg.read("manifest.toml")?)?;
    verify(pkg, &decode_key(&m.creator_key)?)
}

pub(crate) fn verify_embedded_checked(pkg: &mut Package) -> Result<CheckedPackage> {
    let m = Manifest::parse(&pkg.read("manifest.toml")?)?;
    verify_checked(pkg, &decode_key(&m.creator_key)?)
}
