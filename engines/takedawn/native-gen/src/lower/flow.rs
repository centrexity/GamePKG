//! Conditions and closed leaf-function control flow for the eager flag model.
//! Direct calls/tails require an explicit native eager-flag target set. Indirect
//! dispatch and original manual/kernel/lazy-flag ABI need the recovery pass.
use super::*;
use crate::decode::{Decoded, MAX_FUNCTION};
use gamepkg_takedawn_media::{Budget, Lease};
use std::collections::{BTreeMap, BTreeSet};

pub fn condition(suffix: &str) -> Result<&'static str> {
    Ok(match suffix {
        "o" => "(g_eflags & 0x800u) != 0u",
        "no" => "(g_eflags & 0x800u) == 0u",
        "b" | "c" | "nae" => "(g_eflags & 1u) != 0u",
        "ae" | "nb" | "nc" => "(g_eflags & 1u) == 0u",
        "e" | "z" => "(g_eflags & 0x40u) != 0u",
        "ne" | "nz" => "(g_eflags & 0x40u) == 0u",
        "be" | "na" => "(g_eflags & 0x41u) != 0u",
        "a" | "nbe" => "(g_eflags & 0x41u) == 0u",
        "s" => "(g_eflags & 0x80u) != 0u",
        "ns" => "(g_eflags & 0x80u) == 0u",
        "p" | "pe" => "(g_eflags & 4u) != 0u",
        "np" | "po" => "(g_eflags & 4u) == 0u",
        "l" | "nge" => "((g_eflags >> 7u) ^ (g_eflags >> 11u)) & 1u",
        "ge" | "nl" => "(((g_eflags >> 7u) ^ (g_eflags >> 11u)) & 1u) == 0u",
        "le" | "ng" => {
            "((g_eflags & 0x40u) != 0u) || (((g_eflags >> 7u) ^ (g_eflags >> 11u)) & 1u)"
        }
        "g" | "nle" => {
            "((g_eflags & 0x40u) == 0u) && ((((g_eflags >> 7u) ^ (g_eflags >> 11u)) & 1u) == 0u)"
        }
        _ => bail!("unsupported x86 condition"),
    })
}

pub(super) fn conditional_data(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    ensure!(insn.operands.len() <= 2, "invalid condition operand count");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic condition expression"
        );
    }
    if let Some(suffix) = insn.mnemonic.strip_prefix("set") {
        ensure!(insn.operands.len() == 1, "SETcc requires one destination");
        let dst = &insn.operands[0];
        ensure!(width(dst)? == 1, "SETcc requires one byte");
        return Ok(format!(
            "{{ {} }}\n",
            write(dst, &format!("(({}) ? 1u : 0u)", condition(suffix)?), &p)?
        ));
    }
    let suffix = insn
        .mnemonic
        .strip_prefix("cmov")
        .context("expected conditional move")?;
    ensure!(insn.operands.len() == 2, "CMOVcc requires two operands");
    let dst = operand_register(&insn.operands[0])?;
    let src = &insn.operands[1];
    ensure!(
        dst.width >= 2 && width(src)? == dst.width,
        "invalid CMOVcc width"
    );
    // A false condition does not suppress a memory load or its fault.
    Ok(format!(
        "{{ uint32_t v = {}; if ({}) {{ {} }} }}\n",
        read(src, &p)?,
        condition(suffix)?,
        write_register(dst, "v")
    ))
}

pub struct Leaf {
    source: String,
    dependencies: Vec<u32>,
    _memory: Lease,
}
impl Leaf {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn dependencies(&self) -> &[u32] {
        &self.dependencies
    }
}

pub(super) fn direct_target(insn: &Instruction) -> Result<u32> {
    ensure!(
        insn.operands.len() == 1,
        "branch requires one direct target"
    );
    let op = &insn.operands[0];
    ensure!(
        op.kind == "imm" && op.expr.is_none() && op.expr_src.is_none(),
        "indirect or synthetic branch"
    );
    let target = if insn.is_call() {
        insn.call_target
    } else {
        insn.jump_target
    }
    .context("missing direct transfer target")?;
    ensure!(op.imm == Some(target), "inconsistent branch target");
    Ok(target)
}
pub(super) fn branch(insn: &Instruction) -> Result<Option<(String, &'static str)>> {
    let p = prefixes(insn)?;
    ensure!(!p.operand16, "16bit IP branch requires separate lowering");
    let name = insn.mnemonic.as_str();
    if name == "jmp" {
        return Ok(None);
    }
    if matches!(name, "jecxz" | "jcxz") {
        ensure!(
            (name == "jcxz") == p.address16,
            "counter/address width mismatch"
        );
        return Ok(Some((
            String::new(),
            if p.address16 {
                "(g_ecx & 0xffffu) == 0u"
            } else {
                "g_ecx == 0u"
            },
        )));
    }
    if matches!(name, "loop" | "loope" | "loopne") {
        let decrement = if p.address16 {
            "g_ecx = (g_ecx & 0xffff0000u) | ((g_ecx - 1u) & 0xffffu); "
        } else {
            "g_ecx -= 1u; "
        };
        let test = match (p.address16, name) {
            (false, "loop") => "g_ecx != 0u",
            (false, "loope") => "(g_ecx != 0u) && ((g_eflags & 0x40u) != 0u)",
            (false, _) => "(g_ecx != 0u) && ((g_eflags & 0x40u) == 0u)",
            (true, "loop") => "(g_ecx & 0xffffu) != 0u",
            (true, "loope") => "((g_ecx & 0xffffu) != 0u) && ((g_eflags & 0x40u) != 0u)",
            (true, _) => "((g_ecx & 0xffffu) != 0u) && ((g_eflags & 0x40u) == 0u)",
        };
        return Ok(Some((decrement.into(), test)));
    }
    Ok(Some((
        String::new(),
        condition(name.strip_prefix('j').context("expected Jcc")?)?,
    )))
}

/// Emit a complete closed leaf under the original C-call/guest-stack contract:
/// host calls this C function with a guest return-address slot already present;
/// RET removes that slot and returns to its C caller. No dynamic return dispatch
/// or call/SEH/legacy-lazy-flag interoperability is implied.
pub fn leaf(decoded: &Decoded, entry: u32, end: u32, budget: &Budget) -> Result<Leaf> {
    direct_function(decoded, entry, end, &BTreeSet::new(), budget)
}

/// Provisional function emission for an explicit set of native eager-flag
/// function entries. Every dependency must be successfully generated and
/// linked before executing this source. Use program::Program for a closed set.
pub fn direct_function(
    decoded: &Decoded,
    entry: u32,
    end: u32,
    targets: &BTreeSet<u32>,
    budget: &Budget,
) -> Result<Leaf> {
    ensure!(targets.len() <= 32768, "native target count limit exceeded");
    let instructions = decoded.instructions();
    ensure!(
        entry < end && !instructions.is_empty() && instructions.len() <= MAX_FUNCTION,
        "invalid leaf extent/count"
    );
    // Covers output, address/work/visited maps and bounded temporary scopes.
    // The returned owner retains the lease for as long as its C source exists.
    let cap = instructions
        .len()
        .checked_mul(2048)
        .and_then(|n| n.checked_add(65536))
        .context("leaf output extent overflow")?;
    let memory = budget.reserve(instructions.len() as u64 * 4096 + 131072)?;
    let mut lookup = BTreeMap::new();
    let mut previous_end = entry;
    for insn in instructions {
        ensure!(
            insn.address >= previous_end && insn.address >= entry && insn.size > 0,
            "overlapping/out-of-range leaf instruction"
        );
        let next = insn
            .address
            .checked_add(u32::from(insn.size))
            .context("leaf instruction overflow")?;
        ensure!(next <= end, "instruction exceeds leaf extent");
        previous_end = next;
        lookup.insert(insn.address, insn);
    }
    ensure!(lookup.contains_key(&entry), "leaf entry was not decoded");
    let mut source = String::new();
    source.try_reserve_exact(cap)?;
    source.push_str(&format!(
        "void sub_{entry:08x}(void) {{\nuint32_t ebp = g_ebp;\ngoto L_{entry:08x};\n"
    ));
    let mut pending = vec![entry];
    let mut visited = BTreeSet::new();
    let mut dependencies = BTreeSet::new();
    while let Some(address) = pending.pop() {
        if !visited.insert(address) {
            continue;
        }
        let insn = lookup
            .get(&address)
            .context("leaf edge reaches an undecoded address")?;
        let mut next = Vec::with_capacity(2);
        let body = if insn.is_call() {
            let p = prefixes(insn)?;
            ensure!(!p.operand16, "16bit IP call requires separate lowering");
            let target = direct_target(insn)?;
            ensure!(targets.contains(&target), "unresolved native call target");
            dependencies.insert(target);
            next.push(insn.end());
            format!("{{ uint32_t call_sp = g_esp - 4u; xbox_MmioStore(call_sp, 0x{:08x}u, 4); g_esp = call_sp; g_ebp = ebp; sub_{target:08x}(); ebp = g_ebp; }}\ngoto L_{:08x};\n", insn.end(), insn.end())
        } else if insn.is_jump() || insn.is_conditional() {
            let target = direct_target(insn)?;
            let internal = lookup.contains_key(&target);
            let transfer = if internal {
                next.push(target);
                format!("goto L_{target:08x};")
            } else {
                ensure!(
                    !(entry..end).contains(&target) && targets.contains(&target),
                    "external or mid-instruction leaf branch"
                );
                dependencies.insert(target);
                // Tail transfer consumes the existing guest return slot.
                format!("g_ebp = ebp; sub_{target:08x}(); return;")
            };
            if let Some((decrement, condition)) = branch(insn)? {
                next.push(insn.end());
                let taken = if internal {
                    transfer
                } else {
                    format!("{{ {transfer} }}")
                };
                format!(
                    "{decrement}if ({condition}) {taken}\ngoto L_{:08x};\n",
                    insn.end()
                )
            } else {
                format!("{transfer}\n")
            }
        } else if insn.mnemonic == "ret" || insn.mnemonic == "retn" {
            let p = prefixes(insn)?;
            let extra = match insn.operands.as_slice() {
                [] => 0,
                [op] if op.kind == "imm" && op.expr.is_none() && op.expr_src.is_none() => op
                    .imm
                    .filter(|n| *n <= 65535)
                    .context("invalid RET cleanup")?,
                _ => bail!("invalid RET operands"),
            };
            format!(
                "g_esp += {}u; g_ebp = ebp; return;\n",
                extra + if p.operand16 { 2 } else { 4 }
            )
        } else {
            let body = instruction(insn)?;
            next.push(insn.end());
            format!("{body}goto L_{:08x};\n", insn.end())
        };
        ensure!(
            source.len() + body.len() + 32 <= cap,
            "leaf output byte limit exceeded"
        );
        source.push_str(&format!("L_{address:08x}:;\n{body}"));
        for target in next {
            ensure!(
                lookup.contains_key(&target),
                "leaf falls outside decoded closed extent"
            );
            if !visited.contains(&target) {
                pending.push(target);
            }
        }
    }
    source.push_str("}\n");
    Ok(Leaf {
        source,
        dependencies: dependencies.into_iter().collect(),
        _memory: memory,
    })
}
