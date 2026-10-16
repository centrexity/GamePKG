//! Host-selected C/WASI tools; never selected by game source or manifests.
use anyhow::{ensure, Context, Result};
use std::path::{Path, PathBuf};
const SETUP: &str = "Install clang-18, lld-18, wasi-libc and libclang-rt-18-dev-wasm32, or set the C/WASI build tools folder in Settings";

pub(crate) fn resolve(overrides: &[&str]) -> Result<PathBuf> {
    let explicit = overrides
        .iter()
        .find_map(std::env::var_os)
        .map(PathBuf::from);
    let settings = crate::settings::load()?;
    tools_available()?;
    select(
        explicit,
        &settings.wasi_sysroot_directory,
        Path::new("/usr"),
    )
}
#[cfg(feature = "gui")]
pub(crate) fn check_draft(directory: &str) -> Result<PathBuf> {
    tools_available()?;
    select(None, directory, Path::new("/usr"))
}
fn tools_available() -> Result<()> {
    for tool in ["/usr/bin/clang-18", "/usr/bin/wasm-ld-18"] {
        ensure!(Path::new(tool).is_file(), "Missing {tool}. {SETUP}");
    }
    Ok(())
}
fn select(explicit: Option<PathBuf>, saved: &str, system: &Path) -> Result<PathBuf> {
    let selected = explicit.unwrap_or_else(|| {
        if saved.is_empty() {
            system.into()
        } else {
            saved.into()
        }
    });
    let root = selected.canonicalize().with_context(|| {
        format!(
            "Cannot open C/WASI tools folder {}. {SETUP}",
            selected.display()
        )
    })?;
    let base = if root.join("usr/include/wasm32-wasi").is_dir() {
        root.join("usr")
    } else {
        root.clone()
    };
    for file in [
        "include/wasm32-wasi/stdio.h",
        "lib/wasm32-wasi/libc.a",
        "lib/llvm-18/lib/clang/18/lib/wasi/libclang_rt.builtins-wasm32.a",
    ] {
        ensure!(
            base.join(file).is_file(),
            "Missing {} in {}. {SETUP}",
            file,
            selected.display()
        );
    }
    Ok(root)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path) {
        for file in [
            "include/wasm32-wasi/stdio.h",
            "lib/wasm32-wasi/libc.a",
            "lib/llvm-18/lib/clang/18/lib/wasi/libclang_rt.builtins-wasm32.a",
        ] {
            let file = root.join(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"fixture").unwrap();
        }
    }
    #[test]
    fn detects_system_and_extracted_sysroots_and_saved_folder() {
        let root = tempfile::tempdir().unwrap();
        let system = root.path().join("system");
        fixture(&system);
        let saved = root.path().join("saved");
        fixture(&saved.join("usr"));
        assert_eq!(select(None, "", &system).unwrap(), system);
        assert_eq!(
            select(None, saved.to_str().unwrap(), &system).unwrap(),
            saved
        );
        assert_eq!(
            select(Some(system.clone()), saved.to_str().unwrap(), &saved).unwrap(),
            system
        );
    }
    #[test]
    fn missing_tools_and_invalid_explicit_folders_fail_without_falling_back() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path());
        assert!(select(Some(root.path().join("missing")), "", root.path()).is_err());
        assert!(select(None, "/nonexistent-wasi-folder", root.path()).is_err());
        std::fs::remove_file(
            root.path()
                .join("lib/llvm-18/lib/clang/18/lib/wasi/libclang_rt.builtins-wasm32.a"),
        )
        .unwrap();
        let error = select(None, "", root.path()).unwrap_err().to_string();
        assert!(error.contains("Settings") && error.contains("builtins"));
    }
}
