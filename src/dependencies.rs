//! Guest packages have no Cargo dependencies. Host APIs are injected by the compiler.
use anyhow::{bail, Context, Result};
pub fn validate(cargo: &toml::Value, lock: &[u8]) -> Result<()> {
    let table = cargo.as_table().context("invalid Cargo manifest")?;
    if table
        .keys()
        .any(|key| !["package", "lib", "dependencies"].contains(&key.as_str()))
    {
        bail!("guest crates and additional Cargo sections are not allowed");
    }
    if let Some(dependencies) = table.get("dependencies") {
        if dependencies
            .as_table()
            .is_none_or(|entries| !entries.is_empty())
        {
            bail!("guest crates are not allowed; use the host-supplied gamepkg API");
        }
    }
    let lock: toml::Value = toml::from_str(std::str::from_utf8(lock)?)?;
    let locked = lock.as_table().context("invalid guest lockfile")?;
    if locked
        .keys()
        .any(|key| !["version", "package"].contains(&key.as_str()))
        || !matches!(
            locked.get("version").and_then(toml::Value::as_integer),
            Some(3 | 4)
        )
    {
        bail!("unsupported guest lockfile");
    }
    let name = cargo
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .context("missing Cargo package name")?;
    let version = cargo
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(toml::Value::as_str)
        .context("missing Cargo package version")?;
    semver::Version::parse(version)?;
    let root: toml::Value = toml::from_str(&format!("name = {name:?}\nversion = {version:?}"))?;
    let packages = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .context("missing locked package")?;
    if packages.len() != 1 || packages[0] != root {
        bail!("guest lockfile must contain only the root package, without dependencies or sources");
    }
    Ok(())
}
