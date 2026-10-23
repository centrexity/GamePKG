//! Eager integer arithmetic flags. Unsigned intermediates avoid host signed
//! overflow and implementation-defined narrowing. Undefined logical AF keeps
//! its prior image; it is not claimed to match hardware's undefined value.
use super::*;

pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    let unary = matches!(insn.mnemonic.as_str(), "inc" | "dec" | "neg" | "not");
    ensure!(
        insn.operands.len() == if unary { 1 } else { 2 },
        "invalid ALU operand count"
    );
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic ALU expressions require typed lowering"
        );
    }
    let dst = &insn.operands[0];
    let width = width(dst)?;
    let mask = mask(width)?;
    let sign = 1u32 << (width * 8 - 1);
    let memory = dst.kind == "mem";
    let mut code = if memory {
        format!(
            "uint32_t address = {}; uint32_t a = xbox_MmioLoad(address, {width}) & 0x{mask:08x}u; ",
            address(dst, &p, false)?
        )
    } else {
        format!("uint32_t a = {}; ", read(dst, &p)?)
    };
    let op = insn.mnemonic.as_str();
    if unary {
        code.push_str(match op {
            "inc" | "dec" => "uint32_t b = 1u; ",
            "neg" => "uint32_t b = a; a = 0u; ",
            "not" => "",
            _ => unreachable!(),
        });
    } else {
        let src = &insn.operands[1];
        ensure!(
            src.kind == "imm" || super::width(src)? == width,
            "ALU width mismatch"
        );
        ensure!(!memory || src.kind != "mem", "memory-to-memory ALU");
        code.push_str(&format!(
            "uint32_t b = ({}) & 0x{mask:08x}u; ",
            read(src, &p)?
        ));
    }
    let arithmetic = matches!(
        op,
        "add" | "adc" | "sub" | "sbb" | "cmp" | "inc" | "dec" | "neg"
    );
    let addition = matches!(op, "add" | "adc" | "inc");
    if arithmetic {
        let carry = if matches!(op, "adc" | "sbb") {
            "(g_eflags & 1u)"
        } else {
            "0u"
        };
        let symbol = if addition { "+" } else { "-" };
        code.push_str(&format!("uint64_t wide = (uint64_t)a {symbol} (uint64_t)b {symbol} {carry}; uint32_t r = (uint32_t)wide & 0x{mask:08x}u; "));
    } else {
        let expression = match op {
            "not" => "~a",
            "and" | "test" => "a & b",
            "or" => "a | b",
            "xor" => "a ^ b",
            _ => unreachable!(),
        };
        code.push_str(&format!("uint32_t r = ({expression}) & 0x{mask:08x}u; "));
    }
    // Snapshot flags before a register destination (e.g. ADC AL, AH) changes.
    if op != "not" {
        let changed = if matches!(op, "inc" | "dec") {
            0x8d4u32
        } else if arithmetic {
            0x8d5
        } else {
            0x8c5
        };
        code.push_str(&format!("uint32_t flags = (g_eflags & 0x{:08x}u) | 2u | (((0x9669u >> ((r ^ (r >> 4u)) & 15u)) & 1u) << 2u) | (r == 0u ? 0x40u : 0u) | ((r & 0x{sign:08x}u) != 0u ? 0x80u : 0u); ", !changed));
        if arithmetic {
            code.push_str("flags |= (a ^ b ^ r) & 0x10u; ");
            let overflow = if addition { "~(a ^ b)" } else { "(a ^ b)" };
            code.push_str(&format!(
                "flags |= (({overflow} & (a ^ r) & 0x{sign:08x}u) != 0u ? 0x800u : 0u); "
            ));
            if !matches!(op, "inc" | "dec") {
                let carry = if addition {
                    format!("wide > 0x{mask:08x}u")
                } else {
                    let input = if op == "sbb" { "(g_eflags & 1u)" } else { "0u" };
                    format!("(uint64_t)a < (uint64_t)b + {input}")
                };
                code.push_str(&format!("flags |= ({carry}) ? 1u : 0u; "));
            }
        }
    }
    if !matches!(op, "cmp" | "test") {
        if memory {
            code.push_str(&format!("xbox_MmioStore(address, r, {width}); "));
        } else {
            code.push_str(&write(dst, "r", &p)?);
        }
    }
    // Commit flags after a memory store succeeds. General fault rollback,
    // thread scheduling and SEH still belong to the complete function/runtime.
    if op != "not" {
        code.push_str("g_eflags = flags;");
    }
    Ok(format!("{{ {code} }}\n"))
}
