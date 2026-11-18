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
    let package = table
        .get("package")
        .and_then(toml::Value::as_table)
        .context("missing Cargo package")?;
    if package
        .keys()
        .any(|k| !["name", "version", "edition", "build"].contains(&k.as_str()))
        || package.get("build") != Some(&toml::Value::Boolean(false))
    {
        bail!("guest Cargo package must disable build scripts and cannot supply metadata, links or compiler configuration");
    }
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .context("missing package name")?;
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        || !(name.as_bytes()[0].is_ascii_alphabetic() || name.as_bytes()[0] == b'_')
    {
        bail!("invalid Cargo package name");
    }
    if package
        .get("edition")
        .is_some_and(|v| !matches!(v.as_str(), Some("2015" | "2018" | "2021" | "2024")))
    {
        bail!("unsupported Rust edition");
    }
    if let Some(lib) = table.get("lib") {
        let lib = lib.as_table().context("invalid Cargo library")?;
        if lib
            .iter()
            .any(|(k, v)| k != "path" || v.as_str() != Some("src/lib.rs"))
        {
            bail!("guest library cannot supply compiler/linker configuration");
        }
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
