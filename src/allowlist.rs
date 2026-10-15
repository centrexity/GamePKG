//! Pinned, host-owned guest dependencies. Guest manifests cannot choose sources.
use anyhow::{bail, Context, Result};
use std::path::Path;
pub const NAME: &str = "gamepkg-byteorder";
pub const VERSION: &str = "0.1.0";
pub const CHECKSUM: &str = "2240d04066556f415a670196f749db4a9d6474c9d8a02bdb1a4bbc70ffbd4b19";
pub struct Entry {
    pub name: &'static str,
    pub version: &'static str,
    pub checksum: &'static str,
    source: &'static [u8],
}
pub const CATALOG: &[Entry] = &[
    Entry {
        name: "gamepkg-byteorder",
        version: "0.1.0",
        checksum: "2240d04066556f415a670196f749db4a9d6474c9d8a02bdb1a4bbc70ffbd4b19",
        source: include_bytes!("../vendor/gamepkg-byteorder-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-input",
        version: "0.1.0",
        checksum: "65261715a76403a5c383211f2eb568daea70098ee2e5582d00f45c5482901630",
        source: include_bytes!("../vendor/gamepkg-input-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-file",
        version: "0.1.0",
        checksum: "e8bb756b29e036b1e2303016c029c1997c13d8dd4bb221e61f070042a966beae",
        source: include_bytes!("../vendor/gamepkg-file-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-window",
        version: "0.1.0",
        checksum: "90f9cb1f913423b50898051a57c398f00f2c5c81af13074060dd2f009f0ef8a2",
        source: include_bytes!("../vendor/gamepkg-window-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-networking",
        version: "0.1.0",
        checksum: "ff37bff9711f8a6c1b3c8228d5248088035c8bba88a2884c486b1a9820d0b0a1",
        source: include_bytes!("../vendor/gamepkg-networking-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-shader",
        version: "0.1.0",
        checksum: "7059c9a2fcfd27a0bce50f9ba3382cb2cef405fc78022bd420897b1d17f5cf3a",
        source: include_bytes!("../vendor/gamepkg-shader-0.1.0/src/lib.rs"),
    },
    Entry {
        name: "gamepkg-audio",
        version: "0.1.0",
        checksum: "7cae234cee39b219503286f6c1451db9ed1e09b2f21e88183222a9e8f0e9d983",
        source: include_bytes!("../vendor/gamepkg-audio-0.1.0/src/lib.rs"),
    },
];
pub fn selected(cargo: &toml::Value) -> impl Iterator<Item = &'static Entry> + '_ {
    CATALOG.iter().filter(|entry| {
        cargo
            .get("dependencies")
            .and_then(|d| d.get(entry.name))
            .is_some()
    })
}
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
    let settings = crate::settings::load()?;
    for entry in selected(cargo) {
        if !settings
            .approved_crates
            .iter()
            .any(|name| name == entry.name)
        {
            bail!(
                "crate is not approved by this host; use gamepkg allowlist add {}",
                entry.name
            );
        }
    }
    if let Some(dependencies) = dependencies {
        for (name, value) in dependencies {
            let entry = value
                .as_table()
                .context("allowlisted crates require an explicit dependency table")?;
            let pinned = CATALOG
                .iter()
                .find(|e| e.name == name)
                .context("dependency is not on the pinned crate allowlist")?;
            if entry.get("version").and_then(toml::Value::as_str)
                != Some(format!("={}", pinned.version).as_str())
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
    if packages.len() != 1 + selected(cargo).count() {
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
    let mut names: Vec<_> = selected(cargo).map(|e| e.name.to_owned()).collect();
    names.sort();
    let mut root: toml::Value = toml::from_str(&format!("name = {name:?}\nversion = {version:?}"))?;
    if enabled {
        root.as_table_mut()
            .unwrap()
            .insert("dependencies".into(), toml::Value::try_from(names)?);
    }
    if !packages.contains(&root) {
        bail!("lockfile does not match the pinned dependency graph");
    }
    for entry in selected(cargo) {
        let text = format!("name = {:?}\nversion = {:?}", entry.name, entry.version);
        let dependency: toml::Value = toml::from_str(&text)?;
        if !packages.contains(&dependency) {
            bail!("lockfile does not match the pinned dependency graph");
        }
    }
    Ok(enabled)
}
pub fn stage_entry(entry: &Entry, directory: &Path) -> Result<()> {
    if crate::verify::digest(entry.source) != entry.checksum {
        bail!("bundled wrapper source pin mismatch");
    }
    std::fs::create_dir_all(directory.join("src"))?;
    std::fs::write(directory.join("src/lib.rs"), entry.source)?;
    std::fs::write(directory.join("Cargo.toml"), format!("[package]\nname = {:?}\nversion = {:?}\nedition = \"2021\"\nbuild = false\n[features]\ndefault = []\n", entry.name, entry.version))?;
    Ok(())
}

pub fn set_approval(name: &str, approve: bool) -> Result<()> {
    if !CATALOG.iter().any(|entry| entry.name == name) {
        bail!("crate is not in the bundled pinned catalog");
    }
    let mut settings = crate::settings::load()?;
    settings.approved_crates.retain(|entry| entry != name);
    if approve {
        settings.approved_crates.push(name.into());
    }
    settings.save(&crate::settings::path()?)
}
