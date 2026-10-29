//! SSE1 signed32/binary32 conversions through a host integer-only service.
use super::*;
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    ensure!(
        !p.operand16 && p.mandatory == Some(0xf3),
        "invalid SSE1 conversion prefix"
    );
    ensure!(insn.operands.len() == 2, "conversion requires two operands");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic conversion expression"
        );
    }
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let (source, commit, operation) = if insn.mnemonic == "cvtsi2ss" {
        let target = vector::xmm(dst)?;
        ensure!(
            (src.kind == "reg" && operand_register(src)?.width == 4)
                || (src.kind == "mem" && src.mem_size == 4),
            "CVTSI2SS requires r32/m32"
        );
        (read(src, &p)?, format!("{target}.u[0]=result;"), 0)
    } else {
        let target = operand_register(dst)?;
        ensure!(
            target.width == 4,
            "float conversion requires r32 destination"
        );
        let source = if src.kind == "reg" {
            format!("{}.u[0]", vector::xmm(src)?)
        } else {
            ensure!(
                src.kind == "mem" && src.mem_size == 4,
                "float conversion requires XMM/m32"
            );
            read(src, &p)?
        };
        let operation = match insn.mnemonic.as_str() {
            "cvtss2si" => 1,
            "cvttss2si" => 2,
            _ => bail!("unsupported SSE conversion"),
        };
        (source, write_register(target, "result"), operation)
    };
    Ok(format!("{{ extern uint32_t gamepkg_x86_convert_f32(uint32_t,uint32_t,uint32_t,uint32_t); uint32_t value={source}; uint32_t result=gamepkg_x86_convert_f32(value,{operation}u,0x{:08x}u,ebp); {commit} }}\n",insn.address))
}
