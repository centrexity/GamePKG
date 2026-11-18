//! Executable admission is host policy, never a signed creator assertion.
use anyhow::{bail, Result};
pub const REVISION: &str = "gamepkg-exclusive-services-v1";
pub fn require(profile: Option<&str>) -> Result<()> {
    if let Some(profile) = profile {
        // No existing native/builtin source kit has completed the service
        // migration. Signatures, source hashes and old receipts cannot opt out.
        bail!("Port {profile} requires migration to GamePKG shared services before building or playing. Direct SDL/system access and game crates are not permitted.");
    }
    Ok(())
}
pub fn supported(profile: Option<&str>) -> bool {
    require(profile).is_ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn low_level_engine_builders_and_runtime_initialization_cannot_skip_admission() {
        let builders = [
            crate::doom::build,
            crate::zelda3::build,
            #[cfg(target_os = "linux")]
            crate::cube2::build::compile,
        ];
        for build in builders {
            assert!(build()
                .unwrap_err()
                .to_string()
                .contains("requires migration"));
        }
        #[cfg(target_os = "linux")]
        assert!(
            crate::takedawn::build::compile(std::path::Path::new("nonexistent-source-kit"))
                .unwrap_err()
                .to_string()
                .contains("requires migration")
        );
        let mut m =
            crate::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
                .unwrap();
        for p in [
            crate::doom::PROFILE,
            crate::zelda3::PROFILE,
            crate::cube2::PROFILE,
            crate::takedawn::PROFILE,
            "unknown",
        ] {
            m.builtin_engine = Some(p.into());
            let e = crate::runtime::Game::from_bytes(b"not even a valid wasm module", &m)
                .err()
                .unwrap();
            assert!(e.to_string().contains("requires migration"), "{e:#}");
        }
    }
}
