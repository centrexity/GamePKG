//! Executable admission is host policy, never a signed creator assertion.
use anyhow::{bail, Result};

pub const REVISION: &str = "gamepkg-exclusive-services-v1";

pub fn require(profile: Option<&str>) -> Result<()> {
    if let Some(profile) = profile {
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
    fn low_level_engine_initialization_cannot_skip_admission() {
        for profile in [Some("port-owned-engine"), Some("unknown")] {
            assert!(super::require(profile)
                .unwrap_err()
                .to_string()
                .contains("requires migration"));
        }
        let mut m =
            crate::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
                .unwrap();
        m.builtin_engine = Some("port-owned-engine".into());
        let error = crate::runtime::Game::from_bytes(b"not even a valid wasm module", &m)
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("requires migration"),
            "{error:#}"
        );
    }
}
