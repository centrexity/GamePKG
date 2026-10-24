//! Reviewed static jump-table declarations for an immutable executable span.
//! This recovers CFG entries, not native callbacks or an index-range proof.
//! Runtime jumps still load the actual guest table and check their exact PC.
use crate::decode::{Decoded, Decoder, MAX_FUNCTION};
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_media::{Budget, Lease};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read};

#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Exact instruction PC of canonical JMP [index*4+disp32], no prefixes.
    pub branch: u32,
    /// Raw x86 index bits, including negative/wrapped indexes when declared.
    pub first_index: u32,
    pub entries: u32,
}

/// Trusted source-kit metadata. Parse it with the caller's bounded metadata
/// reader; a matching hash binds declarations to exact immutable function bytes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Function {
    pub start: u32,
    pub end: u32,
    pub sha256: String,
    pub tables: Vec<Table>,
    #[serde(skip)]
    _memory: Option<Lease>,
}
impl Function {
    pub fn read(reader: impl Read, budget: &Budget) -> Result<Self> {
        let memory = budget.reserve(512 * 1024)?;
        let mut bytes = Vec::new();
        reader.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "jump-table metadata byte limit");
        let mut profile: Self = serde_json::from_slice(&bytes)?;
        profile._memory = Some(memory);
        Ok(profile)
    }
    pub fn decode(&self, decoder: &Decoder, bytes: &[u8], budget: &Budget) -> Result<Decoded> {
        ensure!(
            self.start.checked_add(u32::try_from(bytes.len())?) == Some(self.end),
            "table profile extent mismatch"
        );
        ensure!(
            self.sha256.len() == 64 && hex::encode(Sha256::digest(bytes)) == self.sha256,
            "table profile function hash mismatch"
        );
        decode(decoder, bytes, self.start, &self.tables, budget)
    }
}

/// Decode from `start`, recursively adding only targets from explicitly
/// declared tables in this same raw-backed executable span. Callers own the
/// declaration review and image identity. There is no guessed table length,
/// linear sweep over data, replacement switch/copy routine, or changed jump.
/// All declarations must be reached; all targets must decode. Table bytes may
/// be shared by branches, but cannot overlap any admitted instruction/target.
/// Unsupported forms, wrong bounds and conflicting metadata fail closed.
pub fn decode(
    decoder: &Decoder,
    bytes: &[u8],
    start: u32,
    tables: &[Table],
    budget: &Budget,
) -> Result<Decoded> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_FUNCTION,
        "invalid table span"
    );
    let end = start
        .checked_add(u32::try_from(bytes.len())?)
        .context("table span wraps")?;
    ensure!(
        tables.len() <= bytes.len() / 7,
        "too many table declarations"
    );
    let _metadata = budget.reserve(bytes.len() as u64 * 64 + tables.len() as u64 * 64)?;
    let mut branches = BTreeSet::new();
    let mut ranges = Vec::new();
    for table in tables {
        ensure!(branches.insert(table.branch), "duplicate table branch");
        let at = table
            .branch
            .checked_sub(start)
            .context("table branch before span")? as usize;
        let b = bytes
            .get(at..at.checked_add(7).context("table branch wraps")?)
            .context("table branch outside span")?;
        ensure!(
            b[0] == 0xff && b[1] == 0x24 && b[2] & 0xc7 == 0x85 && (b[2] >> 3) & 7 != 4,
            "unsupported table branch encoding"
        );
        let base = u32::from_le_bytes(b[3..7].try_into()?);
        let first = base.wrapping_add(table.first_index.wrapping_mul(4));
        let count = table
            .entries
            .checked_mul(4)
            .filter(|&n| n != 0)
            .context("invalid table count")?;
        let last = first.checked_add(count).context("table data wraps")?;
        ensure!(
            first >= start && last <= end,
            "table data outside executable span"
        );
        ranges.push((first, last));
    }
    let mut seeds = BTreeSet::from([start]);
    let mut resolved = BTreeSet::new();
    loop {
        let decoded = decoder.cfg(
            bytes,
            start,
            &seeds.iter().copied().collect::<Vec<_>>(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )?;
        for i in decoded.instructions() {
            ensure!(
                ranges.iter().all(|&(a, b)| i.end() <= a || i.address >= b),
                "instruction overlaps declared table data"
            );
        }
        let mut changed = false;
        for (table, &(first, last)) in tables.iter().zip(&ranges) {
            if resolved.contains(&table.branch) {
                continue;
            }
            let Some(i) = decoded
                .instructions()
                .iter()
                .find(|i| i.address == table.branch)
            else {
                continue;
            };
            ensure!(
                i.is_jump() && i.size == 7,
                "table declaration is not a decoded jump"
            );
            for at in (first..last).step_by(4) {
                let offset = (at - start) as usize;
                let target = u32::from_le_bytes(bytes[offset..offset + 4].try_into()?);
                ensure!(
                    (start..end).contains(&target),
                    "table target outside executable span"
                );
                ensure!(
                    ranges.iter().all(|&(a, b)| target < a || target >= b),
                    "table target is table data"
                );
                changed |= seeds.insert(target);
            }
            resolved.insert(table.branch);
        }
        if !changed {
            ensure!(
                resolved.len() == tables.len(),
                "unreached table declaration"
            );
            ensure!(
                seeds
                    .iter()
                    .all(|pc| decoded.instructions().iter().any(|i| i.address == *pc)),
                "table target failed to decode"
            );
            return Ok(decoded);
        }
    }
}
