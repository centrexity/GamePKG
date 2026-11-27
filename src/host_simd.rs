//! GamePKG-owned host SIMD capability detection.
//!
//! This module is trusted host code. It uses `core`/`std` architecture support
//! only to describe the host to other trusted host code and never exposes CPU
//! identity, SIMD levels, pointers, or intrinsics to guest games.

/// Host SIMD support observed at compile time and at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostSimd {
    /// The host binary was compiled with AVX enabled.
    pub static_avx: bool,
    /// The host binary was compiled with AVX2 enabled.
    pub static_avx2: bool,
    /// The host binary was compiled with AVX-512 enabled.
    pub static_avx512: bool,
    /// The host binary was compiled with AArch64 NEON enabled.
    pub static_neon: bool,
    /// The running CPU reports AVX at runtime.
    pub runtime_avx: bool,
    /// The running CPU reports AVX2 at runtime.
    pub runtime_avx2: bool,
    /// The running CPU reports the AVX-512 features required by the reviewed host layer.
    pub runtime_avx512: bool,
    /// The running CPU reports NEON at runtime.
    pub runtime_neon: bool,
    /// The `fearless_simd` level selected for reviewed host work.
    pub selected_level: &'static str,
}

impl HostSimd {
    /// Returns true when any statically or dynamically detected SIMD level is available.
    pub fn has_any_simd(self) -> bool {
        self.static_avx
            || self.static_avx2
            || self.static_avx512
            || self.static_neon
            || self.runtime_avx
            || self.runtime_avx2
            || self.runtime_avx512
            || self.runtime_neon
    }
}

const MAX_CHECKSUM_BYTES: usize = 16 * 1024 * 1024;

/// Computes a deterministic, bounded host checksum.
///
/// The result is identical for scalar and SIMD hosts, so guest-visible data
/// cannot be used to distinguish CPU features through this API.
pub fn checksum(bytes: &[u8]) -> anyhow::Result<u32> {
    if bytes.len() > MAX_CHECKSUM_BYTES {
        anyhow::bail!("host SIMD checksum input exceeds 16 MiB");
    }
    let mut value = 0x811c_9dc5u32;
    for byte in bytes {
        value ^= u32::from(*byte);
        value = value.wrapping_mul(0x0100_0193);
    }
    Ok(value)
}

/// Detects static and runtime host SIMD capability without exposing any
/// architecture-specific tokens outside this host-owned module.
pub fn detect() -> HostSimd {
    let selected_level = selected_level();
    HostSimd {
        static_avx: cfg!(target_feature = "avx"),
        static_avx2: cfg!(target_feature = "avx2"),
        static_avx512: cfg!(target_feature = "avx512f"),
        static_neon: cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        runtime_avx: runtime_avx(),
        runtime_avx2: runtime_avx2(),
        runtime_avx512: runtime_avx512(),
        runtime_neon: runtime_neon(),
        selected_level,
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn runtime_avx() -> bool {
    std::arch::is_x86_feature_detected!("avx")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn runtime_avx() -> bool {
    false
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn runtime_avx2() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn runtime_avx2() -> bool {
    false
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn runtime_avx512() -> bool {
    std::arch::is_x86_feature_detected!("avx512f")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn runtime_avx512() -> bool {
    false
}

#[cfg(target_arch = "aarch64")]
fn runtime_neon() -> bool {
    std::arch::is_aarch64_feature_detected!("neon")
}

#[cfg(not(target_arch = "aarch64"))]
fn runtime_neon() -> bool {
    false
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn selected_level() -> &'static str {
    let level = fearless_simd::Level::new();
    if level.as_avx512().is_some() {
        "avx512"
    } else if level.as_avx2().is_some() {
        "avx2"
    } else if level.as_sse4_2().is_some() {
        "sse4-2"
    } else if level.as_sse2().is_some() {
        "sse2"
    } else {
        "scalar"
    }
}

#[cfg(target_arch = "aarch64")]
fn selected_level() -> &'static str {
    if fearless_simd::Level::new().as_neon().is_some() {
        "neon"
    } else {
        "scalar"
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
fn selected_level() -> &'static str {
    if fearless_simd::Level::new().is_fallback() {
        "scalar"
    } else {
        "simd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_reports_only_reviewed_host_capabilities() {
        let detected = detect();
        assert_eq!(detected.static_avx, cfg!(target_feature = "avx"));
        assert_eq!(detected.static_avx2, cfg!(target_feature = "avx2"));
        assert_eq!(detected.static_avx512, cfg!(target_feature = "avx512f"));
        assert_eq!(
            detected.static_neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon"))
        );
        assert_eq!(detect(), detected);
    }

    #[test]
    fn checksum_is_bounded_and_identical_for_all_host_levels() {
        assert_eq!(checksum(b"gamepkg").unwrap(), checksum(b"gamepkg").unwrap());
        assert_eq!(checksum(b"").unwrap(), 0x811c_9dc5);
        assert!(checksum(&vec![0; MAX_CHECKSUM_BYTES + 1]).is_err());
    }
}
