//! Creator keys stay outside packages; only their public identity is published.
use anyhow::{bail, Context, Result};
use ed25519_dalek::SigningKey;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};

pub fn generate(path: &Path) -> Result<String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| anyhow::anyhow!("unable to generate signing key: {error}"))?;
    let public = hex::encode(SigningKey::from_bytes(&seed).verifying_key().as_bytes());
    let mut temp = tempfile::NamedTempFile::new_in(parent).context("open key output directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    writeln!(temp, "{}", hex::encode(seed))?;
    temp.as_file().sync_all()?;
    temp.persist_noclobber(path)
        .map_err(|error| error.error)
        .context("write private key without replacing an existing file")?;
    Ok(public)
}

pub fn read_seed(path: &Path) -> Result<[u8; 32]> {
    let file = File::open(path).context("open creator signing key")?;
    if !file.metadata()?.is_file() {
        bail!("signing key must be a regular file");
    }
    let mut bytes = Vec::new();
    file.take(257).read_to_end(&mut bytes)?;
    if bytes.len() > 256 {
        bail!("signing key file is too large");
    }
    hex::decode(std::str::from_utf8(&bytes)?.trim())?
        .try_into()
        .map_err(|_| anyhow::anyhow!("signing key must contain 32 bytes encoded as hexadecimal"))
}
