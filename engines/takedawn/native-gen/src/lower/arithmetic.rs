//! Staged SSE1 binary32 arithmetic through the reviewed software host service.
//! Guest rounding, exception stages and FTZ never use host C float operations.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "addss"
            | "addps"
            | "subss"
            | "subps"
            | "mulss"
            | "mulps"
            | "divss"
            | "divps"
            | "sqrtss"
            | "sqrtps"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    let scalar = insn.mnemonic.ends_with("ss");
    ensure!(
        !p.operand16 && p.mandatory == if scalar { Some(0xf3) } else { None },
        "invalid SSE1 arithmetic prefix"
    );
    ensure!(
        insn.operands.len() == 2,
        "SSE arithmetic requires two operands"
    );
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic SSE arithmetic expression"
        );
    }
    let target = vector::xmm(&insn.operands[0])?;
    let lanes = if scalar { 1 } else { 4 };
    let read = vector::read_vector(&insn.operands[1], &p, lanes, !scalar, insn.address)?;
    let operation = match insn.mnemonic.as_str() {
        "addss" | "addps" => 0,
        "subss" | "subps" => 1,
        "mulss" | "mulps" => 2,
        "divss" | "divps" => 3,
        "sqrtss" | "sqrtps" => 4,
        _ => bail!("unsupported SSE operation"),
    };
    Ok(format!("{{ extern void gamepkg_x86_arithmetic_f32(const uint32_t*,const uint32_t*,uint32_t*,uint32_t,uint32_t,uint32_t,uint32_t); RecompXmm a={target}; {read} RecompXmm result=a; gamepkg_x86_arithmetic_f32(a.u,b.u,result.u,{lanes}u,{operation}u,0x{:08x}u,ebp); {target}=result; }}\n",insn.address))
}
