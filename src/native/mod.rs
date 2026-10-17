//! Direct machine-code builds and separately isolated native game workers.
use anyhow::{bail, ensure, Result};
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
mod build;
#[cfg(target_os = "linux")]
mod session;
#[cfg(target_os = "linux")]
pub(crate) use session::Session;
fn cpu_identity() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        use std::io::Read;
        let mut text = String::new();
        if let Ok(file) = std::fs::File::open("/proc/cpuinfo") {
            let _ = file.take(1024 * 1024).read_to_string(&mut text);
        }
        let selected = text
            .lines()
            .filter(|line| {
                line.starts_with("flags")
                    || line.starts_with("Features")
                    || line.starts_with("model name")
                    || line.starts_with("CPU architecture")
            })
            .take(4)
            .collect::<Vec<_>>()
            .join("\n");
        crate::verify::digest(selected.as_bytes())[..16].to_owned()
    })
}
fn sdk_identity() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(
            concat!(
                include_str!("build.rs"),
                include_str!("guest.rs.txt"),
                include_str!("engine.rs.txt"),
                include_str!("seccomp.rs.txt"),
                include_str!("../guest_helpers.rs.txt"),
                include_str!("../network_guest.rs.txt")
            )
            .as_bytes(),
        );
        // Adapter and engine fixes must invalidate native caches even when the
        // package signature and host version have not changed.
        #[cfg(target_os = "linux")]
        for (name, bytes) in crate::doom_sources::FILES
            .iter()
            .chain(crate::zelda3_sources::FILES.iter())
        {
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hex::encode(hash.finalize())[..16].to_owned()
    })
}
pub fn path_for_digest(root: &Path, digest: &str) -> PathBuf {
    root.join("builds")
        .join(digest)
        .join(format!(
            "native-v1-{}-{}-{}-{}-{}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            cpu_identity(),
            sdk_identity()
        ))
        .join("game")
}
pub fn cache_path(root: &Path, id: &str) -> Result<PathBuf> {
    let manifest = crate::registry::installed_manifest(root, id)?;
    let bytes = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    if manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE) {
        return Ok(crate::cube2::cache_path(
            root,
            &crate::verify::digest(&bytes),
        ));
    }
    Ok(path_for_digest(root, &crate::verify::digest(&bytes)))
}
pub fn is_built(root: &Path, id: &str) -> Result<bool> {
    let path = cache_path(root, id)?;
    let Ok(bytes) = crate::files::read_regular(&path, 64 * 1048576, false) else {
        return Ok(false);
    };
    Ok(
        validate_executable(&bytes).is_ok()
            && crate::integrity::verify_build(&path, &bytes).is_ok(),
    )
}
pub fn compile(root: &Path, id: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        if crate::registry::installed_manifest(root, id)?
            .builtin_engine
            .as_deref()
            == Some(crate::cube2::PROFILE)
        {
            return crate::cube2::build::compile_package(root, id);
        }
        build::compile(root, id)
    }
    #[cfg(not(target_os = "linux"))]
    {
        if crate::registry::installed_manifest(root, id)?
            .builtin_engine
            .as_deref()
            == Some(crate::cube2::PROFILE)
        {
            bail!("Cube 2 currently requires Linux native process isolation");
        }
        let _ = (root, id);
        bail!("native game isolation is not available on this platform; choose the separate Wasm build")
    }
}
pub(crate) fn validate_executable(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() > 64 && bytes.starts_with(b"\x7fELF"),
        "compiler did not produce a native executable"
    );
    ensure!(
        bytes[5] == 1,
        "native artifact has an incompatible byte order"
    );
    let machine = u16::from_le_bytes([bytes[18], bytes[19]]);
    let expected = match std::env::consts::ARCH {
        "x86_64" => 62,
        "x86" => 3,
        "aarch64" => 183,
        "arm" => 40,
        _ => bail!("unsupported native machine"),
    };
    ensure!(
        machine == expected,
        "native artifact belongs to a different CPU"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
pub(crate) fn source_flags(source: &str) -> Result<(bool, bool)> {
    use syn::visit::Visit;
    #[derive(Default)]
    struct Flags {
        draw: bool,
        shader: bool,
    }
    impl<'ast> Visit<'ast> for Flags {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = &*call.func {
                let names = path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>();
                if let Some(last) = names.last() {
                    self.draw |= last.starts_with("draw_");
                    self.shader |= last.starts_with("draw_shader")
                        || (last == "from_slang" && names.iter().any(|n| n == "Shader"));
                }
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut flags = Flags::default();
    flags.visit_file(&syn::parse_file(source)?);
    Ok((flags.draw || flags.shader, flags.shader))
}
