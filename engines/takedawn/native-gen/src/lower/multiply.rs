//! Defined IA-32 MUL/IMUL products and CF/OF. Undefined arithmetic flag bits
//! retain their prior image. All inputs, including aliased implicit operands,
//! are captured before any architectural destination is committed.
use super::*;

pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let prefix = prefixes(insn)?;
    let signed = insn.mnemonic == "imul";
    ensure!(
        matches!(insn.operands.len(), 1..=3),
        "invalid multiply operands"
    );
    ensure!(
        signed || insn.operands.len() == 1,
        "MUL requires one operand"
    );
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic multiply expression"
        );
    }
    let operands = insn.operands.as_slice();
    let implicit = operands.len() == 1;
    let size = width(&operands[0])?;
    ensure!(
        size == 1 || size == if prefix.operand16 { 2 } else { 4 },
        "multiply operand-size mismatch"
    );
    let mask = mask(size)?;
    let bits = size * 8;
    let sign = 1u32 << (bits - 1);
    let (left, right) = if implicit {
        (
            format!("g_eax & 0x{mask:08x}u"),
            read(&operands[0], &prefix)?,
        )
    } else {
        ensure!(
            matches!(size, 2 | 4) && operands[0].kind == "reg",
            "truncated IMUL requires word/dword register destination"
        );
        let source = &operands[1];
        ensure!(
            matches!(source.kind, "reg" | "mem") && width(source)? == size,
            "multiply source width mismatch"
        );
        if operands.len() == 2 {
            (read(&operands[0], &prefix)?, read(source, &prefix)?)
        } else {
            ensure!(
                operands[2].kind == "imm",
                "three-operand IMUL requires immediate"
            );
            (read(source, &prefix)?, read(&operands[2], &prefix)?)
        }
    };
    let mut code =
        format!("uint32_t a = ({left}) & 0x{mask:08x}u; uint32_t b = ({right}) & 0x{mask:08x}u; ");
    if signed {
        // Every u32 fits int64, and two signed32 operands multiply without
        // int64 overflow. Conversion of the product to uint64 is defined even
        // for negative products; no signed right shift or narrowing is used.
        let modulus = 1u64 << bits;
        code.push_str(&format!("int64_t sa = (int64_t)a - ((a & 0x{sign:08x}u) != 0u ? INT64_C({modulus}) : INT64_C(0)); int64_t sb = (int64_t)b - ((b & 0x{sign:08x}u) != 0u ? INT64_C({modulus}) : INT64_C(0)); int64_t product = sa * sb; uint64_t wide = (uint64_t)product; uint32_t overflow = (product < -INT64_C({sign}) || product > INT64_C({})); ", sign - 1));
    } else {
        code.push_str(&format!("uint64_t wide = (uint64_t)a * (uint64_t)b; uint32_t overflow = (wide >> {bits}u) != 0u; "));
    }
    code.push_str("uint32_t flags = (g_eflags & ~0x801u) | (overflow != 0u ? 0x801u : 0u); ");
    if implicit {
        if size == 1 {
            code.push_str(&write_register(register("ax")?, "(uint32_t)wide"));
        } else {
            code.push_str(&write_register(
                register(if size == 2 { "ax" } else { "eax" })?,
                "(uint32_t)wide",
            ));
            code.push_str(&write_register(
                register(if size == 2 { "dx" } else { "edx" })?,
                &format!("(uint32_t)(wide >> {bits}u)"),
            ));
        }
    } else {
        code.push_str(&write(&operands[0], "(uint32_t)wide", &prefix)?);
    }
    code.push_str("g_eflags = flags;");
    Ok(format!("{{ {code} }}\n"))
}
