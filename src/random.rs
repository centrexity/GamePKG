//! Fixed-size host randomness; no device, clock or filesystem is exposed.
pub(crate) fn value() -> anyhow::Result<u64> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("host randomness unavailable: {error}"))?;
    Ok(u64::from_le_bytes(bytes))
}
