//! Game engines are owned by Ports, not by the host. Only source-only native
//! and Wasm games are admitted by this binary.
use anyhow::{bail, Result};

pub fn filename(_profile: &str) -> Result<&'static str> {
    bail!("builtin game engines are owned by Ports")
}

pub fn validate(_profile: &str, _bytes: &[u8]) -> Result<()> {
    bail!("builtin game engines are owned by Ports")
}

pub fn build(_profile: &str) -> Result<Vec<u8>> {
    bail!("builtin game engines are owned by Ports")
}
