//! SSE1 comparison/control foundation. Arithmetic is intentionally separate;
//! no host float comparison, host MXCSR mutation, DAZ extension or NaN casts.
use super::*;
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    ensure!(
        !p.operand16 && p.mandatory.is_none(),
        "invalid SSE comparison/control prefix"
    );
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic floating expression"
        );
    }
    let site = format!("0x{:08x}u, ebp", insn.address);
    let body = if matches!(insn.mnemonic.as_str(), "ldmxcsr" | "stmxcsr") {
        ensure!(
            insn.operands.len() == 1,
            "MXCSR requires one memory operand"
        );
        let op = &insn.operands[0];
        ensure!(op.kind == "mem" && op.mem_size == 4, "MXCSR requires m32");
        let address = address(op, &p, false)?;
        if insn.mnemonic == "ldmxcsr" {
            format!("extern void gamepkg_x86_set_mxcsr(uint32_t,uint32_t,uint32_t); uint32_t value=xbox_MmioLoad({address},4); gamepkg_x86_set_mxcsr(value,{site});")
        } else {
            format!("extern uint32_t gamepkg_x86_get_mxcsr(void); uint32_t address={address}; xbox_MmioStore(address,gamepkg_x86_get_mxcsr(),4);")
        }
    } else {
        ensure!(
            insn.operands.len() == 2,
            "scalar compare requires two operands"
        );
        let target = vector::xmm(&insn.operands[0])?;
        let src = &insn.operands[1];
        let right = if src.kind == "reg" {
            format!("{}.u[0]", vector::xmm(src)?)
        } else {
            ensure!(
                src.kind == "mem" && src.mem_size == 4,
                "scalar compare requires XMM or m32"
            );
            format!("xbox_MmioLoad({},4)", address(src, &p, false)?)
        };
        let silent = u8::from(insn.mnemonic == "ucomiss");
        format!("extern uint32_t gamepkg_x86_compare_f32(uint32_t,uint32_t,int,uint32_t,uint32_t); uint32_t a={target}.u[0]; uint32_t b={right}; uint32_t flags=gamepkg_x86_compare_f32(a,b,{silent},{site}); g_eflags=(g_eflags & ~0x8d5u)|flags;")
    };
    Ok(format!("{{ {body} }}\n"))
}
