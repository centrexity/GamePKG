//! Read only the package-verification metadata from bundled legacy recipes.
//! Historical configure, build, and launch fields are intentionally ignored.
use super::Engine;
use anyhow::{ensure, Context, Result};
use std::collections::HashSet;

pub(super) fn load(records: &[&str]) -> Result<Vec<Engine>> {
    let mut engines = Vec::new();
    let mut ids = HashSet::new();
    for record in records {
        let engine: Engine = toml::from_str(record).context("legacy engine metadata")?;
        ensure!(
            !engine.id.is_empty()
                && engine.id.len() <= 64
                && engine
                    .id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
                && ids.insert(engine.id.clone()),
            "invalid or duplicate legacy engine id"
        );
        ensure!(
            engine.commit.len() == 40
                && engine.commit.bytes().all(|byte| byte.is_ascii_hexdigit())
                && engine.license_sha256.len() == 64
                && engine
                    .license_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()),
            "invalid legacy engine provenance"
        );
        ensure!(
            engine.memory > 0
                && (engine.memory..=8 * 1024 * 1024 * 1024).contains(&engine.hard_memory)
                && !engine.launcher.is_empty()
                && engine.launcher.len() <= 4096
                && !engine.launcher.contains('\0'),
            "invalid legacy engine requirements"
        );
        ensure!(
            engine.forbidden_directories.iter().all(|directory| {
                directory.len() <= 128
                    && !directory.contains('\\')
                    && !directory.chars().any(char::is_control)
                    && !directory
                        .split('/')
                        .any(|part| matches!(part, "" | "." | ".."))
            }) && engine.forbidden_suffixes.iter().all(|suffix| {
                suffix.starts_with('.')
                    && suffix.len() <= 16
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'.')
            }),
            "invalid legacy game-data exclusions"
        );
        crate::game_data::validate_layouts(&engine.layouts)?;
        engines.push(engine);
    }
    Ok(engines)
}
