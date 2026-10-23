//! Defined IA-32 shift/rotate results and eager flags. Undefined flag values
//! retain their prior image; they are not claimed as PIII-specific behavior.
use super::*;

fn count_operand(op: &Operand) -> Result<()> {
    ensure!(
        (op.kind == "imm" && op.imm.is_some_and(|v| v <= 255))
            || (op.kind == "reg" && op.reg.as_deref() == Some("cl")),
        "shift count requires imm8 or CL"
    );
    Ok(())
}
pub(super) fn double_instruction(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    ensure!(
        insn.operands.len() == 3,
        "double shift requires three operands"
    );
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic shift expression"
        );
    }
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let count = &insn.operands[2];
    let bytes = width(dst)?;
    let bits = bytes * 8;
    let mask = mask(bytes)?;
    let sign = 1u32 << (bits - 1);
    ensure!(
        matches!(bytes, 2 | 4) && operand_register(src)?.width == bytes,
        "invalid double-shift widths"
    );
    count_operand(count)?;
    // 16bit counts above16 have undefined results. Do not guess PIII behavior
    // for dynamic word counts; valid constant cases are supported explicitly.
    ensure!(
        bytes == 4 || (count.kind == "imm" && count.imm.is_some_and(|c| (c & 31) <= 16)),
        "dynamic/oversized 16bit double-shift needs defined range analysis"
    );
    let memory = dst.kind == "mem";
    let mut code = format!(
        "uint32_t c = ({}) & 31u; uint32_t b = {}; ",
        read(count, &p)?,
        read(src, &p)?
    );
    if memory {
        code.push_str(&format!(
            "uint32_t address = {}; uint32_t a = xbox_MmioLoad(address, {bytes}) & 0x{mask:08x}u; ",
            address(dst, &p, false)?
        ));
    } else {
        code.push_str(&format!("uint32_t a = {}; ", read(dst, &p)?));
    }
    code.push_str("uint32_t r = a; uint32_t flags = g_eflags; if (c != 0u) { ");
    let cf = if insn.mnemonic == "shld" {
        code.push_str(&format!("uint64_t pair = ((uint64_t)a << {bits}u) | b; r = (uint32_t)((pair << c) >> {bits}u) & 0x{mask:08x}u; "));
        format!("(a >> ({bits}u - c)) & 1u")
    } else {
        code.push_str(&format!("uint64_t pair = ((uint64_t)b << {bits}u) | a; r = (uint32_t)(pair >> c) & 0x{mask:08x}u; "));
        "(a >> (c - 1u)) & 1u".into()
    };
    code.push_str(&format!("flags = (flags & ~0xc5u) | ({cf}) | (((0x9669u >> ((r ^ (r >> 4u)) & 15u)) & 1u) << 2u) | (r == 0u ? 0x40u : 0u) | ((r & 0x{sign:08x}u) != 0u ? 0x80u : 0u); if (c == 1u) {{ flags = (flags & ~0x800u) | (((a ^ r) & 0x{sign:08x}u) != 0u ? 0x800u : 0u); }} }} "));
    if memory {
        code.push_str(&format!("xbox_MmioStore(address, r, {bytes}); "));
    } else {
        code.push_str(&write(dst, "r", &p)?);
    }
    code.push_str("g_eflags = flags;");
    Ok(format!("{{ {code} }}\n"))
}

pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    ensure!(
        insn.operands.len() == 2,
        "shift/rotate requires destination and count"
    );
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic shift expression"
        );
    }
    let dst = &insn.operands[0];
    let count = &insn.operands[1];
    count_operand(count)?;
    let bytes = width(dst)?;
    let bits = bytes * 8;
    let mask = mask(bytes)?;
    let sign = 1u32 << (bits - 1);
    let name = insn.mnemonic.as_str();
    let memory = dst.kind == "mem";
    let mut code = format!("uint32_t c = ({}) & 31u; ", read(count, &p)?);
    if memory {
        code.push_str(&format!(
            "uint32_t address = {}; uint32_t a = xbox_MmioLoad(address, {bytes}) & 0x{mask:08x}u; ",
            address(dst, &p, false)?
        ));
    } else {
        code.push_str(&format!("uint32_t a = {}; ", read(dst, &p)?));
    }
    code.push_str("uint32_t r = a; uint32_t flags = g_eflags; if (c != 0u) { ");
    let shift = matches!(name, "shl" | "sal" | "shr" | "sar");
    if shift {
        code.push_str(&match name {
            "shl"|"sal"=>format!("r = (uint32_t)((uint64_t)a << c) & 0x{mask:08x}u; "),
            "shr"=>"r = a >> c; ".into(),
            "sar"=>format!("r = (a >> c) | ((a & 0x{sign:08x}u) != 0u ? (0x{mask:08x}u ^ (0x{mask:08x}u >> c)) : 0u); "),
            _=>unreachable!(),
        });
        code.push_str(&format!("flags = (flags & ~0xc4u) | (((0x9669u >> ((r ^ (r >> 4u)) & 15u)) & 1u) << 2u) | (r == 0u ? 0x40u : 0u) | ((r & 0x{sign:08x}u) != 0u ? 0x80u : 0u); "));
        if name == "sar" {
            code.push_str(&format!(
                "flags = (flags & ~1u) | (c >= {bits}u ? (a >> {}u) : ((a >> (c - 1u)) & 1u)); ",
                bits - 1
            ));
        } else {
            let cf = if name == "shr" {
                "(a >> (c - 1u)) & 1u".into()
            } else {
                format!("(a >> ({bits}u - c)) & 1u")
            };
            code.push_str(&format!(
                "if (c < {bits}u) {{ flags = (flags & ~1u) | ({cf}); }} "
            ));
        }
        let of = match name {
            "shl" | "sal" => format!("((r >> {}u) ^ flags) & 1u", bits - 1),
            "shr" => format!("a >> {}u", bits - 1),
            _ => "0u".into(),
        };
        code.push_str(&format!(
            "if (c == 1u) {{ flags = (flags & ~0x800u) | (({of}) << 11u); }} "
        ));
    } else {
        let left = matches!(name, "rol" | "rcl");
        if matches!(name, "rcl" | "rcr") {
            let ring_bits = bits + 1;
            let ring_mask = (1u64 << ring_bits) - 1;
            code.push_str(&format!("uint32_t k = c % {ring_bits}u; uint64_t ring = ((uint64_t)a << 1u) | (flags & 1u); "));
            let rotate = if left {
                format!("(ring << k) | (ring >> ({ring_bits}u - k))")
            } else {
                format!("(ring >> k) | (ring << ({ring_bits}u - k))")
            };
            code.push_str(&format!("ring = ({rotate}) & 0x{ring_mask:x}ull; r = (uint32_t)(ring >> 1u); flags = (flags & ~1u) | (uint32_t)(ring & 1u); "));
        } else {
            code.push_str(&format!("uint32_t k = c % {bits}u; "));
            let rotate = if left {
                format!("(a << k) | (a >> (({bits}u - k) % {bits}u))")
            } else {
                format!("(a >> k) | (a << (({bits}u - k) % {bits}u))")
            };
            let cf = if left {
                "r & 1u".into()
            } else {
                format!("r >> {}u", bits - 1)
            };
            code.push_str(&format!(
                "r = ({rotate}) & 0x{mask:08x}u; flags = (flags & ~1u) | ({cf}); "
            ));
        }
        let of = if left {
            format!("((r >> {}u) ^ flags) & 1u", bits - 1)
        } else {
            format!("((r >> {}u) ^ (r >> {}u)) & 1u", bits - 1, bits - 2)
        };
        code.push_str(&format!(
            "if (c == 1u) {{ flags = (flags & ~0x800u) | (({of}) << 11u); }} "
        ));
    }
    code.push_str("} ");
    // A memory RMW still has a writable destination when its count masks to
    // zero. Capture its address once, store, then commit the virtual flags.
    if memory {
        code.push_str(&format!("xbox_MmioStore(address, r, {bytes}); "));
    } else {
        code.push_str(&write(dst, "r", &p)?);
    }
    code.push_str("g_eflags = flags;");
    Ok(format!("{{ {code} }}\n"))
}
