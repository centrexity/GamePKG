//! Only named, host-reviewed engines can be built or loaded.
use anyhow::{bail, Result};
pub fn filename(profile: &str) -> Result<&'static str> {
    match profile {
        crate::zelda3::PROFILE => Ok("zelda3.wasm"),
        crate::doom::PROFILE => Ok("doom.wasm"),
        _ => bail!("unsupported builtin engine"),
    }
}
pub fn memory(profile: &str) -> Result<u64> {
    match profile {
        crate::zelda3::PROFILE => Ok(crate::zelda3::MEMORY),
        crate::doom::PROFILE => Ok(crate::doom::MEMORY),
        crate::cube2::PROFILE => Ok(crate::cube2::MEMORY),
        profile if crate::external_engine::get(profile).is_some() => {
            Ok(crate::external_engine::get(profile).unwrap().memory)
        }
        _ => bail!("unsupported builtin engine"),
    }
}
pub fn validate(profile: &str, bytes: &[u8]) -> Result<()> {
    match profile {
        crate::zelda3::PROFILE => crate::zelda3::validate(bytes),
        crate::doom::PROFILE => crate::doom::validate(bytes),
        _ => bail!("unsupported builtin engine"),
    }
}
pub fn build(profile: &str) -> Result<Vec<u8>> {
    match profile {
        crate::zelda3::PROFILE => crate::zelda3::build(),
        crate::doom::PROFILE => crate::doom::build(),
        _ => bail!("unsupported builtin engine"),
    }
}
