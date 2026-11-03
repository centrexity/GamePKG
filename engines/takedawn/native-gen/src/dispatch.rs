//! Qualified native instruction map and reenterable dispatcher block source.
use crate::{
    lower::dispatch::{self, Block},
    program::Unit,
};
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_media::{Budget, Lease};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Write};
// The guest PC selects decoding. Different starts may overlap on x86; shared
// bytes must agree and each exact PC must retain one identical typed instruction.
// Monotone unit starts and MAX_FUNCTION extents bound live overlap to one ring.
struct InstructionWindow {
    bytes: Vec<(u32, u8, bool)>,
    instructions: Vec<(u32, [u8; 32], bool)>,
}
impl InstructionWindow {
    fn new() -> Self {
        Self {
            bytes: vec![(0, 0, false); crate::decode::MAX_FUNCTION],
            instructions: vec![(0, [0; 32], false); crate::decode::MAX_FUNCTION],
        }
    }
    fn admit(&mut self, i: &crate::decode::Instruction) -> Result<()> {
        ensure!(
            (1..=15).contains(&i.size),
            "invalid dispatch instruction width"
        );
        let raw = hex::decode(&i.bytes_hex)?;
        ensure!(
            raw.len() == usize::from(i.size),
            "invalid dispatch instruction bytes"
        );
        struct HashOnly<'a>(&'a mut Sha256);
        impl Write for HashOnly<'_> {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.update(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut hash = Sha256::new();
        serde_json::to_writer(HashOnly(&mut hash), i)?;
        let fingerprint: [u8; 32] = hash.finalize().into();
        let slot = i.address as usize % self.instructions.len();
        let (old_pc, old_hash, occupied) = self.instructions[slot];
        ensure!(
            !occupied || old_pc <= i.address,
            "dispatch instruction window moved backward"
        );
        ensure!(
            !occupied || old_pc != i.address || old_hash == fingerprint,
            "conflicting same-PC dispatch instruction at {:#010x}",
            i.address
        );
        for (offset, &byte) in raw.iter().enumerate() {
            let address = i
                .address
                .checked_add(offset as u32)
                .context("dispatch instruction overflow")?;
            let (old_at, old_byte, used) = self.bytes[address as usize % self.bytes.len()];
            ensure!(
                !used || old_at <= address,
                "dispatch byte window moved backward"
            );
            ensure!(
                !used || old_at != address || old_byte == byte,
                "conflicting dispatch input byte at {address:#010x}"
            );
        }
        self.instructions[slot] = (i.address, fingerprint, true);
        for (offset, &byte) in raw.iter().enumerate() {
            let address = i.address + offset as u32;
            let at = address as usize % self.bytes.len();
            self.bytes[at] = (address, byte, true);
        }
        Ok(())
    }
}
pub struct DispatchProgram {
    blocks: Vec<(u32, Block)>,
    map: Vec<(u32, u32)>,
    source_bytes: u64,
    controlled: bool,
    _memory: Lease,
}
impl DispatchProgram {
    pub fn build(units: &[Unit<'_>], budget: &Budget, max_source_bytes: u64) -> Result<Self> {
        Self::build_profile(units, budget, max_source_bytes, false)
    }
    pub fn build_controlled(
        units: &[Unit<'_>],
        budget: &Budget,
        max_source_bytes: u64,
    ) -> Result<Self> {
        Self::build_profile(units, budget, max_source_bytes, true)
    }
    fn build_profile(
        units: &[Unit<'_>],
        budget: &Budget,
        max_source_bytes: u64,
        controlled: bool,
    ) -> Result<Self> {
        ensure!(
            !units.is_empty() && units.len() <= 32768,
            "dispatch function count limit"
        );
        let count = units.iter().try_fold(0usize, |v, u| {
            v.checked_add(u.decoded.instructions().len())
                .context("dispatch instruction count overflow")
        })?;
        ensure!(
            count > 0 && count <= 1048576,
            "dispatch instruction count limit"
        );
        let memory = budget.reserve(units.len() as u64 * 256 + count as u64 * 128 + 1048576)?;
        let mut ordered = BTreeMap::new();
        for u in units {
            ensure!(
                u.entry < u.end && ordered.insert(u.entry, u).is_none(),
                "invalid/duplicate dispatch unit"
            );
        }
        let mut entries = BTreeMap::new();
        let mut window = InstructionWindow::new();
        for (&owner, u) in &ordered {
            for i in u.decoded.instructions() {
                window.admit(i)?;
                if let Some((old, _)) = entries.get(&i.address) {
                    ensure!(*old == i, "conflicting shared dispatch instruction");
                } else {
                    entries.insert(i.address, (i, owner));
                }
            }
        }
        let map: Vec<_> = entries.iter().map(|(&a, &(_, owner))| (a, owner)).collect();
        let mut blocks = Vec::new();
        blocks.try_reserve_exact(units.len())?;
        for (entry, u) in ordered {
            let block = if controlled {
                dispatch::controlled_block(u.decoded, entry, u.end, budget)?
            } else {
                dispatch::block(u.decoded, entry, u.end, budget)?
            };
            blocks.push((entry, block));
        }
        let mut program = Self {
            blocks,
            map,
            source_bytes: 0,
            controlled,
            _memory: memory,
        };
        struct Counter {
            bytes: u64,
            limit: u64,
        }
        impl Write for Counter {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                let next = self
                    .bytes
                    .checked_add(b.len() as u64)
                    .filter(|v| *v <= self.limit)
                    .ok_or_else(|| std::io::Error::other("dispatch source byte cap"))?;
                self.bytes = next;
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut counter = Counter {
            bytes: 0,
            limit: max_source_bytes,
        };
        program.write_source(&mut counter)?;
        program.source_bytes = counter.bytes;
        Ok(program)
    }
    pub fn instruction_count(&self) -> usize {
        self.map.len()
    }
    pub fn function_count(&self) -> usize {
        self.blocks.len()
    }
    pub fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
    pub fn write_source(&self, out: &mut impl Write) -> Result<()> {
        let prefix = if self.controlled {
            "gamepkg_cpu_block"
        } else {
            "gamepkg_block"
        };
        let map_prefix = if self.controlled {
            "gamepkg_x86_cpu_generated"
        } else {
            "gamepkg_x86_generated"
        };
        writeln!(out, "#include \"gamepkg_x86_dispatch.h\"")?;
        for (entry, _) in &self.blocks {
            writeln!(out, "void {prefix}_{entry:08x}(uint32_t);")?;
        }
        for (_, block) in &self.blocks {
            out.write_all(block.source().as_bytes())?;
        }
        writeln!(
            out,
            "const GamePkgX86DispatchEntry {map_prefix}_entries[]={{"
        )?;
        for (address, owner) in &self.map {
            writeln!(out, "{{0x{address:08x}u,{prefix}_{owner:08x}}},")?;
        }
        writeln!(
            out,
            "}};\nconst size_t {map_prefix}_count={}u;",
            self.map.len()
        )?;
        Ok(())
    }
}

pub struct StreamedDispatchSummary {
    pub functions: usize,
    pub instruction_entries: usize,
    pub shared_tail_entries: usize,
    pub overlapping_units: usize,
    pub bytes: u64,
    pub source_sha256: String,
    pub map_sha256: String,
}
/// Validate the complete selected input before output, then regenerate one
/// block at a time. Only bounded map records, hashes and an address-stamped
/// instruction window remain live. Loader input must be immutable on both passes. The
/// caller stages output: a changed second-pass input or I/O failure can leave
/// partial bytes, which must never be published as an executable source kit.
pub fn write_streamed<'a>(
    ranges: &[(u32, u32)],
    load: impl Fn(u32, u32) -> Result<&'a [u8]>,
    decoder: &crate::decode::Decoder,
    budget: &Budget,
    max_source_bytes: u64,
    out: &mut impl Write,
) -> Result<StreamedDispatchSummary> {
    write_streamed_profile(
        ranges,
        load,
        decoder,
        budget,
        max_source_bytes,
        out,
        Generation {
            controlled: false,
            tables: None,
        },
    )
}
pub fn write_streamed_controlled<'a>(
    ranges: &[(u32, u32)],
    load: impl Fn(u32, u32) -> Result<&'a [u8]>,
    decoder: &crate::decode::Decoder,
    budget: &Budget,
    max_source_bytes: u64,
    out: &mut impl Write,
) -> Result<StreamedDispatchSummary> {
    write_streamed_profile(
        ranges,
        load,
        decoder,
        budget,
        max_source_bytes,
        out,
        Generation {
            controlled: true,
            tables: None,
        },
    )
}
/// Checked table profiles participate in both immutable-input passes and in
/// same-PC/byte conflict validation. Every supplied profile must match a unit.
pub fn write_streamed_controlled_tables<'a>(
    ranges: &[(u32, u32)],
    load: impl Fn(u32, u32) -> Result<&'a [u8]>,
    decoder: &crate::decode::Decoder,
    budget: &Budget,
    max_source_bytes: u64,
    out: &mut impl Write,
    tables: &BTreeMap<u32, crate::jump_table::Function>,
) -> Result<StreamedDispatchSummary> {
    write_streamed_profile(
        ranges,
        load,
        decoder,
        budget,
        max_source_bytes,
        out,
        Generation {
            controlled: true,
            tables: Some(tables),
        },
    )
}
struct Generation<'a> {
    controlled: bool,
    tables: Option<&'a BTreeMap<u32, crate::jump_table::Function>>,
}
impl Generation<'_> {
    fn decode(
        &self,
        decoder: &crate::decode::Decoder,
        input: &[u8],
        entry: u32,
        end: u32,
        budget: &Budget,
    ) -> Result<crate::decode::Decoded> {
        if let Some(profile) = self.tables.and_then(|t| t.get(&entry)) {
            ensure!(
                profile.start == entry && profile.end == end,
                "jump-table profile range mismatch"
            );
            profile.decode(decoder, input, budget)
        } else {
            decoder.cfg(
                input,
                entry,
                &[entry],
                &Default::default(),
                &Default::default(),
            )
        }
    }
}
fn write_streamed_profile<'a>(
    ranges: &[(u32, u32)],
    load: impl Fn(u32, u32) -> Result<&'a [u8]>,
    decoder: &crate::decode::Decoder,
    budget: &Budget,
    max_source_bytes: u64,
    out: &mut impl Write,
    generation: Generation<'_>,
) -> Result<StreamedDispatchSummary> {
    use crate::decode::MAX_FUNCTION;
    ensure!(
        !ranges.is_empty() && ranges.len() <= 32768,
        "dispatch function count limit"
    );
    let _metadata = budget.reserve(ranges.len() as u64 * 256 + 1048576)?;
    let mut ordered = ranges.to_vec();
    ordered.sort_unstable();
    let controlled = generation.controlled;
    if let Some(tables) = generation.tables {
        ensure!(
            tables
                .iter()
                .all(|(&entry, p)| p.start == entry && ordered.contains(&(entry, p.end))),
            "unused/mismatched jump-table profile"
        );
    }
    let (mut previous_end, mut previous_entry, mut tails, mut overlaps) = (0, None, 0usize, 0usize);
    for &(entry, end) in &ordered {
        ensure!(
            entry < end && (end - entry) as usize <= MAX_FUNCTION,
            "invalid dispatch extent"
        );
        ensure!(previous_entry != Some(entry), "duplicate dispatch entry");
        overlaps += usize::from(entry < previous_end);
        tails += usize::from(entry < previous_end && end == previous_end);
        previous_entry = Some(entry);
        previous_end = previous_end.max(end);
    }
    let mut checks = Vec::new();
    checks.try_reserve_exact(ordered.len())?;
    let mut map = Vec::<(u32, u32)>::new();
    let mut map_memory = Vec::new();
    map_memory.try_reserve_exact(64)?;
    let mut window = InstructionWindow::new();
    let mut body_bytes = 0u64;
    for &(entry, end) in &ordered {
        let input = load(entry, end)?;
        ensure!(
            input.len() == (end - entry) as usize,
            "dispatch loader returned wrong extent"
        );
        let decoded = generation.decode(decoder, input, entry, end, budget)?;
        for i in decoded.instructions() {
            ensure!(
                i.address >= entry && i.end() <= end,
                "dispatch instruction outside extent"
            );
            window.admit(i)?;
        }
        let lower = if controlled {
            dispatch::controlled_block
        } else {
            dispatch::block
        };
        let block = lower(&decoded, entry, end, budget)
            .with_context(|| format!("dispatch block {entry:#010x}"))?;
        body_bytes = body_bytes
            .checked_add(block.source().len() as u64)
            .context("dispatch source size overflow")?;
        ensure!(body_bytes <= max_source_bytes, "dispatch source byte cap");
        for &address in block.addresses() {
            ensure!(map.len() < 1048576, "dispatch instruction count limit");
            if map.len() == map.capacity() {
                let chunk = (1048576 - map.len()).min(16384);
                let lease = budget.reserve(chunk as u64 * 16)?;
                map.try_reserve_exact(chunk)?;
                map_memory.push(lease);
            }
            map.push((address, entry));
        }
        checks.push((
            Sha256::digest(input),
            Sha256::digest(block.source().as_bytes()),
        ));
    }
    // Earlier owners are canonical for a qualified shared instruction. Their
    // bytes/boundaries already agree; retaining one prevents duplicate lookup PCs.
    map.sort_unstable();
    map.dedup_by_key(|v| v.0);
    ensure!(!map.is_empty(), "empty dispatch map");
    let prelude = b"#include \"gamepkg_x86_dispatch.h\"\n";
    let prefix = if controlled {
        "gamepkg_cpu_block"
    } else {
        "gamepkg_block"
    };
    let map_prefix = if controlled {
        "gamepkg_x86_cpu_generated"
    } else {
        "gamepkg_x86_generated"
    };
    let table_start = format!("const GamePkgX86DispatchEntry {map_prefix}_entries[]={{\n");
    let table_end = format!("}};\nconst size_t {map_prefix}_count={}u;\n", map.len());
    let declaration_bytes = format!("void {prefix}_{:08x}(uint32_t);\n", ordered[0].0).len() as u64;
    let map_row_bytes =
        format!("{{0x{:08x}u,{prefix}_{:08x}}},\n", map[0].0, map[0].1).len() as u64;
    let expected = body_bytes
        .checked_add(prelude.len() as u64)
        .and_then(|v| v.checked_add(ordered.len() as u64 * declaration_bytes))
        .and_then(|v| v.checked_add(table_start.len() as u64))
        .and_then(|v| v.checked_add(map.len() as u64 * map_row_bytes))
        .and_then(|v| v.checked_add(table_end.len() as u64))
        .context("dispatch source size overflow")?;
    ensure!(expected <= max_source_bytes, "dispatch source byte cap");
    struct Hashed<'b, W> {
        out: &'b mut W,
        hash: Sha256,
        bytes: u64,
    }
    impl<W: Write> Write for Hashed<'_, W> {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            let n = self.out.write(b)?;
            self.hash.update(&b[..n]);
            self.bytes += n as u64;
            Ok(n)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.out.flush()
        }
    }
    let mut writer = Hashed {
        out,
        hash: Sha256::new(),
        bytes: 0,
    };
    writer.write_all(prelude)?;
    for &(entry, _) in &ordered {
        writeln!(writer, "void {prefix}_{entry:08x}(uint32_t);")?;
    }
    for (&(entry, end), (input_hash, source_hash)) in ordered.iter().zip(checks) {
        let input = load(entry, end)?;
        ensure!(
            input.len() == (end - entry) as usize && Sha256::digest(input) == input_hash,
            "dispatch input changed between passes"
        );
        let decoded = generation.decode(decoder, input, entry, end, budget)?;
        let lower = if controlled {
            dispatch::controlled_block
        } else {
            dispatch::block
        };
        let block = lower(&decoded, entry, end, budget)?;
        ensure!(
            Sha256::digest(block.source().as_bytes()) == source_hash,
            "dispatch lowering changed between passes"
        );
        writer.write_all(block.source().as_bytes())?;
    }
    writer.write_all(table_start.as_bytes())?;
    let mut map_hash = Sha256::new();
    for &(address, owner) in &map {
        writeln!(writer, "{{0x{address:08x}u,{prefix}_{owner:08x}}},")?;
        map_hash.update(address.to_le_bytes());
        map_hash.update(owner.to_le_bytes());
    }
    writer.write_all(table_end.as_bytes())?;
    ensure!(
        writer.bytes == expected,
        "dispatch source planning mismatch"
    );
    Ok(StreamedDispatchSummary {
        functions: ordered.len(),
        instruction_entries: map.len(),
        shared_tail_entries: tails,
        overlapping_units: overlaps,
        bytes: writer.bytes,
        source_sha256: hex::encode(writer.hash.finalize()),
        map_sha256: hex::encode(map_hash.finalize()),
    })
}
