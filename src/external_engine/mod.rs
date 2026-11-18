//! Read-only records for identifying previously published source recipes.
//! Executable admission is owned by the shared native compatibility layer.
use crate::manifest::DataLayout;
use anyhow::{ensure, Result};
use std::sync::OnceLock;

mod catalog;

/// Metadata needed to recognize and verify an existing signed port package.
#[derive(serde::Deserialize)]
pub struct Engine {
    pub id: String,
    pub commit: String,
    pub license_sha256: String,
    pub memory: u64,
    pub hard_memory: u64,
    pub network_allowed: bool,
    pub launcher: String,
    pub forbidden_directories: Vec<String>,
    pub forbidden_suffixes: Vec<String>,
    pub layouts: Vec<DataLayout>,
}

impl Engine {
    pub fn layouts(&self) -> Vec<DataLayout> {
        self.layouts.clone()
    }
}

include!(concat!(env!("OUT_DIR"), "/external_engine_records.rs"));

fn catalog() -> &'static [Engine] {
    static CATALOG: OnceLock<Vec<Engine>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        catalog::load(RECORDS)
            .unwrap_or_else(|error| panic!("reviewed external-engine catalog is invalid: {error}"))
    })
}

pub fn get(id: &str) -> Option<&'static Engine> {
    catalog().iter().find(|engine| engine.id == id)
}

pub(crate) fn validate_recipe(
    engine: &Engine,
    package: &mut crate::archive::Package,
) -> Result<()> {
    ensure!(
        package.read("source/src/lib.rs")? == engine.launcher.as_bytes(),
        "this engine accepts only its reviewed launcher"
    );
    ensure!(
        !package
            .entries
            .keys()
            .any(|name| name.starts_with("assets/") || name.starts_with("shaders/")),
        "this engine cannot ship package media or shaders in place of game data"
    );
    for name in package.entries.keys() {
        let lower = name.to_ascii_lowercase();
        ensure!(
            !engine
                .forbidden_suffixes
                .iter()
                .any(|suffix| lower.ends_with(suffix.as_str()))
                && !engine
                    .forbidden_directories
                    .iter()
                    .any(|dir| contains_directory(&lower, dir)),
            "this package cannot contain the engine's game data"
        );
        if name.starts_with("source/") {
            ensure!(
                matches!(
                    name.as_str(),
                    "source/Cargo.toml" | "source/Cargo.lock" | "source/src/lib.rs"
                ),
                "this engine cannot add package-supplied source"
            );
        }
    }
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)
}

fn contains_directory(name: &str, directory: &str) -> bool {
    name == directory
        || name.starts_with(&format!("{directory}/"))
        || name.contains(&format!("/{directory}/"))
}
