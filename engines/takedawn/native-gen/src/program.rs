//! Closed programs of native eager-flag functions. No unresolved stubs,
//! original lazy-flag/manual/kernel function calls or indirect dispatch.
use crate::decode::{Decoder, MAX_FUNCTION};
use crate::{
    decode::Decoded,
    lower::flow::{self, Leaf},
};
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_media::{Budget, Lease};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

pub struct ClosedEntries {
    entries: BTreeSet<u32>,
    _memory: Lease,
}
impl ClosedEntries {
    pub fn entries(&self) -> &BTreeSet<u32> {
        &self.entries
    }
}
/// Dependency planning only: preserve closed cycles, remove missing targets and
/// their transitive callers with a bounded iterative worklist. This does not
/// prove nonoverlapping layout, successful linking or game semantics.
pub fn closed_entries(
    dependencies: &BTreeMap<u32, Vec<u32>>,
    budget: &Budget,
) -> Result<ClosedEntries> {
    ensure!(dependencies.len() <= 32768, "native dependency node limit");
    let mut edges = 0usize;
    for targets in dependencies.values() {
        edges = edges
            .checked_add(targets.len())
            .context("native dependency edge overflow")?;
        ensure!(edges <= 262144, "native dependency edge limit");
    }
    let memory = budget.reserve(dependencies.len() as u64 * 256 + edges as u64 * 16 + 65536)?;
    let mut entries: BTreeSet<_> = dependencies.keys().copied().collect();
    let mut reverse = BTreeMap::<u32, Vec<u32>>::new();
    let mut pending = Vec::new();
    pending.try_reserve_exact(dependencies.len() + edges)?;
    for (&caller, targets) in dependencies {
        if targets.iter().any(|target| !entries.contains(target)) {
            pending.push(caller);
        }
        for target in targets {
            if entries.contains(target) {
                reverse.entry(*target).or_default().push(caller);
            }
        }
    }
    while let Some(removed) = pending.pop() {
        if entries.remove(&removed) {
            if let Some(callers) = reverse.remove(&removed) {
                pending.extend(callers);
            }
        }
    }
    Ok(ClosedEntries {
        entries,
        _memory: memory,
    })
}

pub struct Unit<'a> {
    pub decoded: &'a Decoded,
    pub entry: u32,
    pub end: u32,
}

pub struct StreamedSummary {
    pub functions: usize,
    pub shared_tail_entries: usize,
    pub bytes: u64,
    pub source_sha256: String,
}

/// Validate every function before writing, then regenerate one function at a
/// time. Neither decoded instructions nor generated bodies accumulate. The
/// loader must return exactly the requested immutable extent on both passes.
/// Nested ranges sharing an end are supported only when all reachable shared
/// bytes and instruction boundaries agree. Crossing ranges still need recovery.
/// Callers must stage output: I/O failure or changed input can leave partial
/// bytes in the writer, and those bytes must never be published as a program.
pub fn write_streamed<'a>(
    ranges: &[(u32, u32)],
    load: impl Fn(u32, u32) -> Result<&'a [u8]>,
    decoder: &Decoder,
    budget: &Budget,
    max_source_bytes: u64,
    output: &mut impl Write,
) -> Result<StreamedSummary> {
    ensure!(
        !ranges.is_empty() && ranges.len() <= 32768,
        "native program function count limit"
    );
    let _memory = budget.reserve(ranges.len() as u64 * 256 + 65536)?;
    let mut ordered = ranges.to_vec();
    ordered.sort_unstable();
    let mut previous_end = 0;
    let mut previous_entry = None;
    let mut shared_tail_entries = 0;
    for &(entry, end) in &ordered {
        ensure!(
            entry < end && (end - entry) as usize <= MAX_FUNCTION,
            "invalid native program extent"
        );
        ensure!(
            previous_entry != Some(entry),
            "duplicate native program entry"
        );
        ensure!(
            entry >= previous_end || end == previous_end,
            "overlapping native program units require recovery: {entry:#010x}..{end:#010x} crosses prior end {previous_end:#010x}"
        );
        shared_tail_entries += usize::from(entry < previous_end);
        previous_entry = Some(entry);
        previous_end = end;
    }
    let targets: BTreeSet<_> = ordered.iter().map(|&(entry, _)| entry).collect();
    let mut checks = Vec::new();
    checks.try_reserve_exact(ordered.len())?;
    let mut bytes = 0u64;
    // Two byte maps fit inside the fixed metadata lease. Only one nested
    // cluster is live; its complete span cannot exceed MAX_FUNCTION.
    let mut boundaries = vec![0u8; MAX_FUNCTION];
    let mut contents = vec![0u8; MAX_FUNCTION];
    let mut cluster_start = 0;
    let mut cluster_end = 0;
    for &(entry, end) in &ordered {
        let input = load(entry, end)?;
        ensure!(
            input.len() == (end - entry) as usize,
            "native loader returned wrong extent"
        );
        let decoded = decoder.cfg(input, entry, &[entry], &BTreeSet::new(), &BTreeSet::new())?;
        if entry >= cluster_end {
            cluster_start = entry;
            cluster_end = end;
            boundaries.fill(0);
        }
        for instruction in decoded.instructions() {
            for offset in 0..u32::from(instruction.size) {
                let address = instruction
                    .address
                    .checked_add(offset)
                    .context("native instruction extent overflow")?;
                ensure!(
                    address >= entry && address < end,
                    "native instruction outside function extent"
                );
                let index = (address - cluster_start) as usize;
                let marker = offset as u8 + 1;
                let byte = input[(address - entry) as usize];
                ensure!(
                    boundaries[index] == 0
                        || (boundaries[index] == marker && contents[index] == byte),
                    "conflicting native shared-tail instructions at {address:#010x}"
                );
                boundaries[index] = marker;
                contents[index] = byte;
            }
        }
        let function = flow::direct_function(&decoded, entry, end, &targets, budget)
            .with_context(|| format!("native function {entry:#010x}"))?;
        bytes = bytes
            .checked_add(format!("void sub_{entry:08x}(void);\n").len() as u64)
            .and_then(|n| n.checked_add(function.source().len() as u64))
            .context("native source size overflow")?;
        ensure!(bytes <= max_source_bytes, "native source byte limit");
        checks.push((
            Sha256::digest(input),
            Sha256::digest(function.source().as_bytes()),
        ));
    }
    let mut hash = Sha256::new();
    for &(entry, _) in &ordered {
        let declaration = format!("void sub_{entry:08x}(void);\n");
        output.write_all(declaration.as_bytes())?;
        hash.update(declaration.as_bytes());
    }
    for (&(entry, end), (input_hash, source_hash)) in ordered.iter().zip(checks) {
        let input = load(entry, end)?;
        ensure!(
            input.len() == (end - entry) as usize && Sha256::digest(input) == input_hash,
            "native source changed between passes"
        );
        let decoded = decoder.cfg(input, entry, &[entry], &BTreeSet::new(), &BTreeSet::new())?;
        let function = flow::direct_function(&decoded, entry, end, &targets, budget)?;
        ensure!(
            Sha256::digest(function.source().as_bytes()) == source_hash,
            "native lowering changed between passes"
        );
        output.write_all(function.source().as_bytes())?;
        hash.update(function.source().as_bytes());
    }
    Ok(StreamedSummary {
        functions: ordered.len(),
        shared_tail_entries,
        bytes,
        source_sha256: hex::encode(hash.finalize()),
    })
}
pub struct Program {
    functions: Vec<(u32, Leaf)>,
    _memory: Lease,
}
impl Program {
    /// All input functions must lower successfully. Typed entry addresses become
    /// declarations; external free-form symbols/source are never accepted.
    /// Units may be supplied out of order, but cannot overlap or share entries.
    pub fn build(units: &[Unit<'_>], budget: &Budget) -> Result<Self> {
        ensure!(
            !units.is_empty() && units.len() <= 32768,
            "native program function count limit"
        );
        let memory = budget.reserve(units.len() as u64 * 256 + 65536)?;
        let mut ordered = BTreeMap::new();
        for unit in units {
            ensure!(
                unit.entry < unit.end && ordered.insert(unit.entry, unit).is_none(),
                "invalid/duplicate native program entry"
            );
        }
        let mut previous_end = 0;
        for unit in ordered.values() {
            ensure!(
                unit.entry >= previous_end,
                "overlapping native program units require recovery"
            );
            previous_end = unit.end;
        }
        let targets: BTreeSet<_> = ordered.keys().copied().collect();
        let mut functions = Vec::new();
        functions.try_reserve_exact(units.len())?;
        for (&entry, unit) in &ordered {
            let function = flow::direct_function(unit.decoded, entry, unit.end, &targets, budget)
                .with_context(|| format!("native function {entry:#010x}"))?;
            ensure!(
                function
                    .dependencies()
                    .iter()
                    .all(|target| targets.contains(target)),
                "native program is not closed"
            );
            functions.push((entry, function));
        }
        Ok(Self {
            functions,
            _memory: memory,
        })
    }
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
    /// Build validates the entire program before any successful Program exists.
    /// A writer failure is returned; the build caller owns staged-file cleanup.
    pub fn write_source(&self, output: &mut impl Write) -> Result<()> {
        for (entry, _) in &self.functions {
            writeln!(output, "void sub_{entry:08x}(void);")?;
        }
        for (_, function) in &self.functions {
            output.write_all(function.source().as_bytes())?;
        }
        Ok(())
    }
}
