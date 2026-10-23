//! SSE1 MIN/MAX select exact source bits, including NaNs and signed zeros.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(name, "minss" | "minps" | "maxss" | "maxps")
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    let scalar = insn.mnemonic.ends_with("ss");
    ensure!(
        !p.operand16 && p.mandatory == if scalar { Some(0xf3) } else { None },
        "invalid SSE1 selection prefix"
    );
    let raw = hex::decode(&insn.bytes_hex)?;
    let start = raw
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67 | 0xf3))
        .count();
    let operation = u32::from(insn.mnemonic.starts_with("max"));
    ensure!(
        raw.get(start) == Some(&0x0f)
            && raw.get(start + 1) == Some(&(if operation == 0 { 0x5d } else { 0x5f })),
        "wrong selection opcode"
    );
    ensure!(insn.operands.len() == 2, "selection requires two operands");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic selection expression"
        );
    }
    let target = vector::xmm(&insn.operands[0])?;
    let lanes = if scalar { 1 } else { 4 };
    let read = vector::read_vector(&insn.operands[1], &p, lanes, !scalar, insn.address)?;
    Ok(format!("{{ extern void gamepkg_x86_select_f32(const uint32_t*,const uint32_t*,uint32_t*,uint32_t,uint32_t,uint32_t,uint32_t); RecompXmm a={target}; {read} RecompXmm result=a; gamepkg_x86_select_f32(a.u,b.u,result.u,{lanes}u,{operation}u,0x{:08x}u,ebp); {target}=result; }}\n",insn.address))
}
