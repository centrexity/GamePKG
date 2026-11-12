//! Instruction-address native dispatcher blocks; no native C guest call stack.
use super::*;
use crate::decode::{Decoded, MAX_FUNCTION};
use gamepkg_takedawn_media::{Budget, Lease};
use std::collections::BTreeMap;
pub struct Block {
    source: String,
    addresses: Vec<u32>,
    _memory: Lease,
}
impl Block {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn addresses(&self) -> &[u32] {
        &self.addresses
    }
}
fn transfer(target: &str) -> String {
    format!("gamepkg_x86_dispatch_transfer({target},ebp); return;\n")
}
fn target(insn: &Instruction, p: &Prefixes) -> Result<String> {
    ensure!(insn.operands.len() == 1, "transfer operand count");
    let op = &insn.operands[0];
    ensure!(
        op.expr.is_none() && op.expr_src.is_none(),
        "synthetic transfer operand"
    );
    if op.kind == "imm" {
        return Ok(format!("0x{:08x}u", flow::direct_target(insn)?));
    }
    ensure!(width(op)? == 4, "transfer target must be32bit");
    if op.kind == "reg" {
        return read(op, p);
    }
    ensure!(op.kind == "mem", "unsupported transfer operand");
    let _ = address(op, p, false)?;
    let segment = if op.mem_seg.as_deref() == Some("fs") {
        2
    } else if matches!(op.mem_base.as_deref(), Some("ebp" | "esp" | "bp" | "sp")) {
        1
    } else {
        0
    };
    Ok(format!(
        "gamepkg_x86_dispatch_load({},{segment}u,0x{:08x}u,ebp)",
        address(op, p, true)?,
        insn.address
    ))
}
/// Exact CLI/STI only; emitted calls require a reviewed virtual-CPU provider.
/// This is not ordinary guest instruction lowering or a native CLI/STI opcode.
pub fn controlled_interrupt_instruction(i: &Instruction) -> Result<String> {
    ensure!(
        matches!(i.mnemonic.as_str(), "cli" | "sti"),
        "not an interrupt instruction"
    );
    let op = u32::from(i.mnemonic == "sti");
    ensure!(
        i.size == 1
            && i.bytes_hex == (if op == 0 { "fa" } else { "fb" })
            && i.operands.is_empty()
            && i.op_str.is_empty()
            && i.call_target.is_none()
            && i.jump_target.is_none()
            && i.memory_refs.is_empty()
            && i.imm_values.is_empty(),
        "unqualified controlled interrupt instruction"
    );
    Ok(format!(
        "if(gamepkg_x86_dispatch_interrupt({op}u,ebp))return;\n"
    ))
}
/// Exact direct far pointer forms in legacy32 code. Runtime code-cache
/// capability is mandatory; ordinary lowering never turns this into near JMP.
pub fn controlled_far_instruction(i: &Instruction) -> Result<String> {
    ensure!(i.mnemonic == "ljmp", "not a far JMP");
    let bytes = hex::decode(&i.bytes_hex)?;
    let small = bytes.first() == Some(&0x66);
    let start = usize::from(small);
    let offset_bytes = if small { 2 } else { 4 };
    ensure!(
        bytes.len() == start + 1 + offset_bytes + 2
            && usize::from(i.size) == bytes.len()
            && bytes.get(start) == Some(&0xea)
            && i.operands.len() == 2
            && i.call_target.is_none()
            && i.jump_target.is_none()
            && i.memory_refs.is_empty(),
        "unqualified far JMP encoding or metadata"
    );
    let mut offset = 0u32;
    for n in 0..offset_bytes {
        offset |= u32::from(bytes[start + 1 + n]) << (n * 8);
    }
    let selector_at = start + 1 + offset_bytes;
    let selector = u32::from(u16::from_le_bytes([
        bytes[selector_at],
        bytes[selector_at + 1],
    ]));
    ensure!(
        i.imm_values == [selector, offset],
        "far immediates mismatch"
    );
    for (operand, value) in i.operands.iter().zip([selector, offset]) {
        ensure!(
            *operand
                == Operand {
                    kind: "imm",
                    imm: Some(value),
                    ..Operand::default()
                },
            "forged far operand"
        );
    }
    let number = |n: u32| {
        if n < 10 {
            n.to_string()
        } else {
            format!("0x{n:x}")
        }
    };
    ensure!(
        i.op_str == format!("{}:{}", number(selector), number(offset)),
        "far text mismatch"
    );
    Ok(format!(
        "gamepkg_x86_dispatch_far_jump({selector}u,0x{offset:08x}u,{}u,ebp);return;\n",
        u32::from(small)
    ))
}
/// Legacy protected32 table stores require a scoped reviewed CPU provider.
/// Ordinary lowering rejects these operations; no native host table is read.
pub fn controlled_table_instruction(i: &Instruction) -> Result<String> {
    ensure!(
        matches!(i.mnemonic.as_str(), "sgdt" | "sidt"),
        "not a table store"
    );
    let p = parse_prefixes(i, false)?;
    ensure!(
        i.operands.len() == 1
            && i.call_target.is_none()
            && i.jump_target.is_none()
            && i.imm_values.is_empty(),
        "invalid table metadata"
    );
    let operand = &i.operands[0];
    ensure!(
        operand.kind == "mem"
            && operand.mem_size == 6
            && operand.expr.is_none()
            && operand.expr_src.is_none()
            && matches!(operand.mem_seg.as_deref(), None | Some("fs")),
        "unqualified table operand"
    );
    let bytes = hex::decode(&i.bytes_hex)?;
    let mut start = 0;
    let mut seen = 0u8;
    while start < bytes.len() {
        let bit = match bytes[start] {
            0x64 => 1,
            0x66 => 2,
            0x67 => 4,
            _ => break,
        };
        ensure!(seen & bit == 0, "repeated table prefix");
        seen |= bit;
        start += 1;
    }
    let op = u32::from(i.mnemonic == "sidt");
    ensure!(
        bytes.len() >= start + 3
            && bytes[start..start + 2] == [0x0f, 0x01]
            && bytes[start + 2] < 0xc0
            && u32::from((bytes[start + 2] >> 3) & 7) == op,
        "unqualified table encoding"
    );
    let modrm = bytes[start + 2];
    let mode = modrm >> 6;
    let rm = modrm & 7;
    let mut encoded = start + 3;
    let displacement = if p.address16 {
        match (mode, rm) {
            (0, 6) | (2, _) => 2,
            (1, _) => 1,
            _ => 0,
        }
    } else {
        let base = if rm == 4 {
            let sib = *bytes.get(encoded).context("missing table SIB")?;
            encoded += 1;
            sib & 7
        } else {
            rm
        };
        match (mode, base) {
            (0, 5) | (2, _) => 4,
            (1, _) => 1,
            _ => 0,
        }
    };
    ensure!(
        encoded + displacement == bytes.len(),
        "table encoding length mismatch"
    );
    ensure!(
        (seen & 1 != 0) == (operand.mem_seg.as_deref() == Some("fs")),
        "table segment/prefix mismatch"
    );
    let segment = if operand.mem_seg.as_deref() == Some("fs") {
        2
    } else if matches!(
        operand.mem_base.as_deref(),
        Some("ebp" | "esp" | "bp" | "sp")
    ) {
        1
    } else {
        0
    };
    Ok(format!(
        "if(gamepkg_x86_dispatch_store_table({op}u,{}u,{},{}u,ebp))return;\n",
        u32::from(p.operand16),
        address(operand, &p, true)?,
        segment
    ))
}
pub fn block(decoded: &Decoded, entry: u32, end: u32, budget: &Budget) -> Result<Block> {
    block_profile(decoded, entry, end, budget, false)
}
pub fn controlled_block(decoded: &Decoded, entry: u32, end: u32, budget: &Budget) -> Result<Block> {
    block_profile(decoded, entry, end, budget, true)
}
fn block_profile(
    decoded: &Decoded,
    entry: u32,
    end: u32,
    budget: &Budget,
    controlled: bool,
) -> Result<Block> {
    let insns = decoded.instructions();
    ensure!(
        entry < end
            && (end - entry) as usize <= MAX_FUNCTION
            && !insns.is_empty()
            && insns.len() <= MAX_FUNCTION,
        "invalid dispatch block"
    );
    let memory = budget.reserve(insns.len() as u64 * 4096 + 131072)?;
    let cap = insns
        .len()
        .checked_mul(4096)
        .and_then(|v| v.checked_add(65536))
        .context("dispatch source cap overflow")?;
    let mut lookup = BTreeMap::new();
    let mut previous = entry;
    for i in insns {
        ensure!(
            i.size > 0 && i.address >= previous && i.address >= entry && i.end() <= end,
            "unsorted or out-of-range dispatch instruction"
        );
        previous = i.address;
        ensure!(
            lookup.insert(i.address, i).is_none(),
            "duplicate dispatch instruction"
        );
    }
    ensure!(lookup.contains_key(&entry), "dispatch entry not decoded");
    let prefix = if controlled {
        "gamepkg_cpu_block"
    } else {
        "gamepkg_block"
    };
    let retire = if controlled {
        "if(gamepkg_x86_dispatch_retire(ebp))return;\n"
    } else {
        ""
    };
    let mut source=format!("void {prefix}_{entry:08x}(uint32_t dispatch_pc) {{\nuint32_t ebp=g_ebp;\nswitch(dispatch_pc){{\n");
    for &a in lookup.keys() {
        source.push_str(&format!("case 0x{a:08x}u:goto L_{a:08x};\n"));
    }
    source.push_str("default:gamepkg_x86_dispatch_invalid();return;\n}\n");
    for (&a, i) in &lookup {
        source.push_str(&format!(
            "L_{a:08x}:;gamepkg_x86_dispatch_pc=0x{a:08x}u;g_ebp=ebp;\n"
        ));
        if controlled {
            source.push_str(&format!(
                "if(gamepkg_x86_dispatch_instruction(0x{a:08x}u,{}u,ebp))return;\n",
                i.size
            ));
        }
        let p = if i.is_call()
            || i.is_jump()
            || i.is_conditional()
            || matches!(i.mnemonic.as_str(), "ret" | "retn")
        {
            prefixes(i)?
        } else {
            Prefixes::default()
        };
        if controlled && i.mnemonic == "ljmp" {
            source.push_str(&controlled_far_instruction(i)?);
        } else if i.is_call() {
            ensure!(!p.operand16, "16bit IP CALL not yet qualified");
            let t = target(i, &p)?;
            if controlled {
                source.push_str(&format!(
                    "{{uint32_t target={t};if(gamepkg_x86_dispatch_call_to(target,0x{:08x}u,0x{a:08x}u,ebp))return;{retire}{}}}\n",
                    i.end(), transfer("target")
                ));
            } else {
                source.push_str(&format!(
                    "{{uint32_t target={t};gamepkg_x86_dispatch_call(0x{:08x}u,0x{a:08x}u,ebp);{retire}{}}}\n",
                    i.end(), transfer("target")
                ));
            }
        } else if i.is_jump() || i.is_conditional() {
            ensure!(!p.operand16, "16bit IP branch not yet qualified");
            let t = target(i, &p)?;
            let jump = if let Some(destination) = i.jump_target.filter(|v| lookup.contains_key(v)) {
                format!("goto L_{destination:08x};\n")
            } else {
                transfer(if controlled { "target" } else { &t })
            };
            if let Some((decrement, condition)) = flow::branch(i)? {
                if controlled {
                    let _ = flow::direct_target(i)?;
                    let staged = decrement.replace("g_ecx", "next_ecx");
                    let condition = condition.replace("g_ecx", "next_ecx");
                    source.push_str(&format!(
                        "{{uint32_t target={t};uint32_t next_ecx=g_ecx;{staged}int taken=({condition});if(taken && gamepkg_x86_dispatch_check_target(target,0x{a:08x}u,ebp))return;g_ecx=next_ecx;{retire}if(taken){{{jump}}}}}\n"
                    ));
                } else {
                    source.push_str(&format!("{decrement}{retire}if({condition}){{{jump}}}\n"));
                }
                if lookup.contains_key(&i.end()) {
                    source.push_str(&format!("goto L_{:08x};\n", i.end()));
                } else {
                    source.push_str(&transfer(&format!("0x{:08x}u", i.end())));
                }
            } else if controlled {
                source.push_str(&format!(
                    "{{uint32_t target={t};if(gamepkg_x86_dispatch_check_target(target,0x{a:08x}u,ebp))return;{retire}{jump}}}\n"
                ));
            } else {
                source.push_str(retire);
                source.push_str(&jump);
            }
        } else if matches!(i.mnemonic.as_str(), "ret" | "retn") {
            let extra = match i.operands.as_slice() {
                [] => 0,
                [op] if op.kind == "imm" && op.expr.is_none() && op.expr_src.is_none() => op
                    .imm
                    .filter(|v| *v <= 65535)
                    .context("invalid RET cleanup")?,
                _ => bail!("invalid RET operands"),
            };
            source.push_str(&format!(
                "{{uint32_t target=gamepkg_x86_dispatch_ret({}u,{extra}u,0x{a:08x}u,ebp);{retire}{}}}\n",
                if p.operand16 { 2 } else { 4 },
                transfer("target")
            ));
        } else {
            if controlled && matches!(i.mnemonic.as_str(), "cli" | "sti") {
                source.push_str(&controlled_interrupt_instruction(i)?);
            } else if controlled && matches!(i.mnemonic.as_str(), "sgdt" | "sidt") {
                source.push_str(&controlled_table_instruction(i)?);
            } else {
                source.push_str(&if controlled && string::supports(&i.mnemonic) {
                    string::controlled_instruction(i)?
                } else {
                    instruction(i)?
                });
                source.push_str(retire);
            }
            if lookup.contains_key(&i.end()) {
                source.push_str(&format!("goto L_{:08x};\n", i.end()));
            } else {
                source.push_str(&transfer(&format!("0x{:08x}u", i.end())));
            }
        }
        ensure!(source.len() + 2 <= cap, "dispatch source byte cap");
    }
    source.push_str("}\n");
    Ok(Block {
        source,
        addresses: lookup.keys().copied().collect(),
        _memory: memory,
    })
}
