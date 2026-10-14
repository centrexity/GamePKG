//! Pinned, host-owned guest dependencies. Guest manifests cannot choose sources.
use anyhow::{bail, Context, Result};
use std::path::Path;
pub const NAME: &str = "byteorder-lite";
pub const VERSION: &str = "0.1.0";
pub const CHECKSUM: &str = "8f1fe948ff07f4bd06c30984e69f5b4899c516a3ef74f34df92a2df2ab535495";
pub fn approved(cargo: &toml::Value, lock: &[u8]) -> Result<bool> {
    let table = cargo
        .as_table()
        .context("invalid Cargo allowlist declaration")?;
    if table
        .keys()
        .any(|key| !["package", "lib", "dependencies"].contains(&key.as_str()))
    {
        bail!("unsupported Cargo section in dependency allowlist");
    }
    let dependencies = cargo.get("dependencies").and_then(toml::Value::as_table);
    let enabled = dependencies.is_some_and(|table| !table.is_empty());
    if cargo.get("dependencies").is_some() && dependencies.is_none() {
        bail!("invalid dependency allowlist declaration");
    }
    if enabled
        && !crate::settings::load()?
            .approved_crates
            .iter()
            .any(|name| name == NAME)
    {
        bail!("crate is not approved by this host; use gamepkg allowlist add byteorder-lite");
    }
    if let Some(dependencies) = dependencies {
        for (name, value) in dependencies {
            let entry = value
                .as_table()
                .context("allowlisted crates require an explicit dependency table")?;
            if name != NAME
                || entry.get("version").and_then(toml::Value::as_str) != Some("=0.1.0")
                || entry.get("default-features").and_then(toml::Value::as_bool) != Some(false)
                || entry
                    .keys()
                    .any(|key| !["version", "default-features", "features"].contains(&key.as_str()))
                || entry.get("features").is_some_and(|features| {
                    features
                        .as_array()
                        .is_none_or(|features| !features.is_empty())
                })
            {
                bail!("dependency is not on the pinned crate allowlist");
            }
        }
    }
    let lock: toml::Value = toml::from_str(std::str::from_utf8(lock)?)?;
    if lock.as_table().is_none_or(|table| {
        table
            .keys()
            .any(|key| !["version", "package"].contains(&key.as_str()))
    }) || !matches!(
        lock.get("version").and_then(toml::Value::as_integer),
        Some(3 | 4)
    ) {
        bail!("unsupported guest lockfile");
    }
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .context("missing locked packages")?;
    if packages.len() != if enabled { 2 } else { 1 } {
        bail!("lockfile contains unapproved packages");
    }
    let name = cargo
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .context("missing Cargo package name")?;
    let version = cargo
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .context("missing Cargo package version")?;
    semver::Version::parse(version)?;
    let root: toml::Value = toml::from_str(&format!(
        "name = {name:?}\nversion = {version:?}\n{}",
        if enabled {
            "dependencies = [\"byteorder-lite\"]"
        } else {
            ""
        }
    ))?;
    let dependency: toml::Value = toml::from_str(&format!("name = {NAME:?}\nversion = {VERSION:?}\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = {CHECKSUM:?}"))?;
    if !packages.contains(&root) || (enabled && !packages.contains(&dependency)) {
        bail!("lockfile does not match the pinned dependency graph");
    }
    Ok(enabled)
}
pub fn stage(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory.join("src"))?;
    std::fs::write(
        directory.join("src/lib.rs"),
        include_bytes!("../vendor/byteorder-lite-0.1.0/src/lib.rs"),
    )?;
    std::fs::write(directory.join("Cargo.toml"), "[package]\nname = \"byteorder-lite\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = false\n[features]\ndefault = []\nstd = []\n")?;
    Ok(())
}

pub fn set_approval(name: &str, approve: bool) -> Result<()> {
    if name != NAME {
        bail!("crate is not in the bundled pinned catalog");
    }
    let mut settings = crate::settings::load()?;
    settings.approved_crates.retain(|entry| entry != name);
    if approve {
        settings.approved_crates.push(name.into());
    }
    settings.save(&crate::settings::path()?)
}
