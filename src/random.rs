//! Fixed-size host randomness; no device, clock or filesystem is exposed.
pub(crate) fn value() -> anyhow::Result<u64> {
    let mut bytes = [0u8; 8];
    fill(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}
pub(crate) fn fill(bytes: &mut [u8]) -> anyhow::Result<()> {
    getrandom::fill(bytes)
        .map_err(|error| anyhow::anyhow!("host randomness unavailable: {error}"))?;
    Ok(())
}
