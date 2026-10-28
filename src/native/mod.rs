//! Direct machine-code builds and separately isolated native game workers.
#[cfg(any(windows, test))]
use anyhow::Context;
use anyhow::{bail, ensure, Result};
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "linux", windows))]
const MAX_CALLS_PER_TICK: usize = 8192;
#[cfg(any(target_os = "linux", windows))]
mod build;
#[cfg(any(target_os = "linux", windows))]
mod engine_sources;
#[cfg(all(test, any(target_os = "linux", windows)))]
pub(crate) fn compile_with_settings(
    root: &Path,
    id: &str,
    settings: &crate::settings::Settings,
) -> Result<()> {
    build::compile_with_settings(root, id, settings)
}
#[cfg(any(target_os = "linux", windows))]
mod session;
#[cfg(all(test, any(target_os = "linux", windows)))]
mod signed_fixture;
#[cfg(any(target_os = "linux", windows))]
mod transport;
#[cfg(windows)]
mod windows_build;
#[cfg(any(target_os = "linux", windows))]
pub(crate) use session::Session;
pub(crate) fn cpu_identity() -> &'static str {
    static ID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        #[cfg(all(windows, any(target_arch = "x86_64", target_arch = "x86")))]
        {
            #[cfg(target_arch = "x86")]
            use std::arch::x86::{__cpuid, __cpuid_count};
            #[cfg(target_arch = "x86_64")]
            use std::arch::x86_64::{__cpuid, __cpuid_count};
            let mut identity = Vec::new();
            // Exclude the per-core APIC ID; include model, features, brand and
            // OS vector-state support so native artifacts cannot cross CPUs.
            // Rust 1.89 requires unsafe CPUID calls; newer toolchains make
            // these same intrinsics safe. Keep compatibility with our MSRV.
            #[allow(unused_unsafe)]
            unsafe {
                let max = __cpuid(0).eax;
                let extended = __cpuid(0x80000000).eax;
                for leaf in [0, 1, 7, 0x80000001, 0x80000002, 0x80000003, 0x80000004] {
                    if (leaf < 0x80000000 && leaf > max) || (leaf >= 0x80000000 && leaf > extended)
                    {
                        continue;
                    }
                    let mut registers = __cpuid_count(leaf, 0);
                    if leaf == 1 {
                        registers.ebx &= 0x00ff_ffff;
                    }
                    for value in [registers.eax, registers.ebx, registers.ecx, registers.edx] {
                        identity.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
            identity.extend_from_slice(&[
                std::is_x86_feature_detected!("avx") as u8,
                std::is_x86_feature_detected!("avx2") as u8,
                std::is_x86_feature_detected!("avx512f") as u8,
            ]);
            crate::verify::digest(&identity)[..16].to_owned()
        }
        #[cfg(not(all(windows, any(target_arch = "x86_64", target_arch = "x86"))))]
        {
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
        }
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
                include_str!("engine_sources.rs"),
                include_str!("guest.rs.txt"),
                include_str!("engine.rs.txt"),
                include_str!("pixels.rs.txt"),
                include_str!("seccomp.rs.txt"),
                include_str!("../guest_helpers.rs.txt"),
                include_str!("../network_guest.rs.txt"),
                include_str!("../doom.rs"),
                include_str!("../zelda3.rs")
            )
            .as_bytes(),
        );
        #[cfg(any(target_os = "linux", windows))]
        {
            // The generated SDK embeds this value in addition to its source.
            hash.update(b"native-host-call-limit\0");
            hash.update((MAX_CALLS_PER_TICK as u64).to_le_bytes());
        }
        // Adapter and engine fixes must invalidate native caches even when the
        // package signature and host version have not changed.
        #[cfg(windows)]
        {
            hash.update(include_str!("windows_build.rs").as_bytes());
            hash.update(include_str!("../windows_compiler.rs").as_bytes());
            hash.update(include_str!("../../engines/native/windows-rust.json").as_bytes());
            hash.update(
                include_str!("../../engines/takedawn/windows-dependencies.json").as_bytes(),
            );
            hash.update(include_str!("../takedawn/windows_build.rs").as_bytes());
            hash.update(crate::takedawn::build::SDK.as_bytes());
            for (name, source) in crate::takedawn::build::SOURCES {
                hash.update(name.as_bytes());
                hash.update(source.as_bytes());
            }
        }
        #[cfg(any(target_os = "linux", windows))]
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
    if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
        return Ok(crate::takedawn::cache_path(
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
    #[cfg(windows)]
    if crate::registry::installed_manifest(root, id)?
        .builtin_engine
        .as_deref()
        == Some(crate::takedawn::PROFILE)
    {
        let companion = path.with_file_name("SDL2.dll");
        let Ok(dll) = crate::files::read_regular(&companion, 32 * 1048576, false) else {
            return Ok(false);
        };
        if crate::integrity::verify_build(&companion, &dll).is_err() {
            return Ok(false);
        }
    }
    Ok(
        validate_executable(&bytes).is_ok()
            && crate::integrity::verify_build(&path, &bytes).is_ok(),
    )
}
pub fn build_supported(manifest: &crate::manifest::Manifest) -> bool {
    if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
        return cfg!(all(
            target_arch = "x86_64",
            any(target_os = "linux", windows)
        ));
    }
    cfg!(target_os = "linux")
        || (cfg!(all(
            windows,
            any(target_arch = "x86_64", target_arch = "x86")
        )) && manifest.builtin_engine.is_none())
}
pub fn play_supported(manifest: &crate::manifest::Manifest) -> bool {
    build_supported(manifest)
}
pub fn compile(root: &Path, id: &str) -> Result<()> {
    #[cfg(windows)]
    {
        let manifest = crate::registry::installed_manifest(root, id)?;
        ensure!(build_supported(&manifest), "native build is not supported for this game on this platform; choose the separate Wasm build");
        if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
            return crate::takedawn::build::compile_package(root, id);
        }
        build::compile(root, id)
    }
    #[cfg(target_os = "linux")]
    {
        let manifest = crate::registry::installed_manifest(root, id)?;
        ensure!(
            build_supported(&manifest),
            "native build is not supported for this game on this platform"
        );
        if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
            return crate::takedawn::build::compile_package(root, id);
        }
        if manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE) {
            return crate::cube2::build::compile_package(root, id);
        }
        build::compile(root, id)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        if crate::registry::installed_manifest(root, id)?
            .builtin_engine
            .as_deref()
            == Some(crate::takedawn::PROFILE)
        {
            bail!("Takedawn native process isolation currently requires Linux");
        }
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
    #[cfg(windows)]
    return validate_pe(bytes, std::env::consts::ARCH);
    #[cfg(not(windows))]
    {
        ensure!(
            bytes.len() > 64 && bytes.starts_with(b"\x7fELF"),
            "compiler did not produce a native executable"
        );
        ensure!(
            bytes[4] == if usize::BITS == 64 { 2 } else { 1 },
            "native artifact has an incompatible pointer width"
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
}

#[cfg(any(windows, test))]
fn validate_pe(bytes: &[u8], architecture: &str) -> Result<()> {
    ensure!(
        bytes.len() >= 64 && bytes.starts_with(b"MZ"),
        "compiler did not produce a Windows executable"
    );
    let offset = u32::from_le_bytes(bytes[60..64].try_into()?) as usize;
    let header = bytes
        .get(offset..)
        .context("invalid Windows executable header")?;
    ensure!(
        offset >= 64 && header.len() >= 24 && header.starts_with(b"PE\0\0"),
        "invalid Windows executable header"
    );
    let machine = u16::from_le_bytes(header[4..6].try_into()?);
    let (expected, magic) = match architecture {
        "x86_64" => (0x8664, 0x20b),
        "x86" => (0x14c, 0x10b),
        "aarch64" => (0xaa64, 0x20b),
        _ => bail!("unsupported native Windows machine"),
    };
    ensure!(
        machine == expected,
        "native artifact belongs to a different CPU"
    );
    let optional = usize::from(u16::from_le_bytes(header[20..22].try_into()?));
    let flags = u16::from_le_bytes(header[22..24].try_into()?);
    ensure!(
        optional >= 2 && header.len() >= 24 + optional && flags & 2 != 0 && flags & 0x2000 == 0,
        "native artifact is not a Windows executable"
    );
    ensure!(
        u16::from_le_bytes(header[24..26].try_into()?) == magic,
        "incompatible Windows executable width"
    );
    Ok(())
}

#[cfg(any(target_os = "linux", windows))]
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

#[cfg(test)]
mod pixel_tests;

#[cfg(test)]
mod windows_artifact_tests {
    use super::validate_pe;
    #[test]
    fn checks_windows_format_width_machine_and_dll_flag() {
        let mut bytes = vec![0u8; 512];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[84..86].copy_from_slice(&240u16.to_le_bytes());
        bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
        validate_pe(&bytes, "x86_64").unwrap();
        assert!(validate_pe(&bytes, "x86").is_err());
        bytes[86..88].copy_from_slice(&0x2002u16.to_le_bytes());
        assert!(validate_pe(&bytes, "x86_64").is_err());
        bytes[86..88].copy_from_slice(&2u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x10bu16.to_le_bytes());
        assert!(validate_pe(&bytes, "x86_64").is_err());
        for length in 0..bytes.len() {
            let _ = validate_pe(&bytes[..length], "x86_64");
        }
        bytes[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_pe(&bytes, "x86_64").is_err());
    }
}
