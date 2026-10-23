//! IA-32 unsigned/signed division. Every host divide is unsigned and guarded;
//! faults are non-returning guest #DE events at the instruction address, not
//! host SIGFPE, saturated results or skipped instructions. The host must link
//! runtime/gamepkg_x86_fault.c and enter through its live thread-local boundary.
use super::*;

pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    ensure!(insn.operands.len() == 1, "division requires one operand");
    let source = &insn.operands[0];
    ensure!(
        source.expr.is_none() && source.expr_src.is_none(),
        "synthetic division expression"
    );
    let size = width(source)?;
    ensure!(
        size == 1 || size == if p.operand16 { 2 } else { 4 },
        "division operand-size mismatch"
    );
    let mask = mask(size)?;
    let bits = size * 8;
    let dividend = if size == 1 {
        "(uint64_t)(g_eax & 0xffffu)".into()
    } else {
        format!("((uint64_t)(g_edx & 0x{mask:08x}u) << {bits}u) | (g_eax & 0x{mask:08x}u)")
    };
    let trap = format!("gamepkg_x86_divide_error(0x{:08x}u, ebp);", insn.address);
    let mut code = format!("extern _Noreturn void gamepkg_x86_divide_error(uint32_t, uint32_t); uint32_t divisor = ({}) & 0x{mask:08x}u; uint64_t dividend = {dividend}; if (divisor == 0u) {{ {trap} }} ", read(source, &p)?);
    if insn.mnemonic == "idiv" {
        let full_mask = if size == 4 {
            u64::MAX
        } else {
            (1u64 << (bits * 2)) - 1
        };
        let dividend_sign = 1u64 << (bits * 2 - 1);
        let sign = 1u32 << (bits - 1);
        code.push_str(&format!("uint32_t negative = (dividend & UINT64_C({dividend_sign})) != 0u; uint32_t divisor_negative = (divisor & 0x{sign:08x}u) != 0u; uint64_t magnitude = negative ? (UINT64_C(0) - dividend) & UINT64_C({full_mask}) : dividend; uint32_t denominator = divisor_negative ? (0u - divisor) & 0x{mask:08x}u : divisor; uint64_t quotient = magnitude / denominator; uint32_t remainder = (uint32_t)(magnitude % denominator); uint32_t quotient_negative = negative ^ divisor_negative; if (quotient > (quotient_negative ? UINT64_C({sign}) : UINT64_C({}))) {{ {trap} }} uint32_t q = (uint32_t)(quotient_negative ? UINT64_C(0) - quotient : quotient); uint32_t r = negative ? 0u - remainder : remainder; ", sign - 1));
    } else {
        code.push_str(&format!("uint64_t quotient = dividend / divisor; if (quotient > UINT64_C({mask})) {{ {trap} }} uint32_t q = (uint32_t)quotient; uint32_t r = (uint32_t)(dividend % divisor); "));
    }
    if size == 1 {
        code.push_str(&write_register(
            register("ax")?,
            "(q & 0xffu) | ((r & 0xffu) << 8u)",
        ));
    } else {
        code.push_str(&write_register(
            register(if size == 2 { "ax" } else { "eax" })?,
            "q",
        ));
        code.push_str(&write_register(
            register(if size == 2 { "dx" } else { "edx" })?,
            "r",
        ));
    }
    // All arithmetic flags are undefined for DIV/IDIV. Preserve their virtual
    // image without claiming physical PIII behavior for undefined values.
    Ok(format!("{{ {code} }}\n"))
}
