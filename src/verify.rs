use crate::{archive::Package, manifest::Manifest};
use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

pub fn digest(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
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
        let data = pkg.read(&name)?;
        h.update((name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update((data.len() as u64).to_le_bytes());
        h.update(Sha256::digest(&data));
    }
    Ok(h.finalize().to_vec())
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub public_key: String,
    pub signature: String,
}
pub fn verify(pkg: &mut Package, trusted: &[u8; 32]) -> Result<Manifest> {
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
        if name == "preview.av1.webm" || name == "preview.ogv" {
            bail!("preview validation is not implemented");
        }
        if !matches!(
            name.as_str(),
            "manifest.toml"
                | "signature.toml"
                | "thumbnail.png"
                | "LICENSE"
                | "NOTICE"
                | "CREDITS.md"
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
    VerifyingKey::from_bytes(trusted)?
        .verify_strict(&tree_hash(pkg)?, &sig)
        .context("invalid package signature")?;
    crate::source::scan(pkg)?;
    Ok(m)
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
