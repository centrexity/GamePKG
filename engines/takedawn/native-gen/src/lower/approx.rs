//! SSE1 reciprocal approximations with fixed portable numerical profile.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(name, "rcpss" | "rcpps" | "rsqrtss" | "rsqrtps")
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    let scalar = insn.mnemonic.ends_with("ss");
    ensure!(
        !p.operand16 && p.mandatory == if scalar { Some(0xf3) } else { None },
        "invalid SSE approximation prefix"
    );
    let raw = hex::decode(&insn.bytes_hex)?;
    let mut n = 0;
    let (mut address16, mut fs, mut scalar_prefix) = (false, false, false);
    while let Some(&v) = raw.get(n) {
        let slot = match v {
            0x64 => &mut fs,
            0x67 => &mut address16,
            0xf3 => &mut scalar_prefix,
            _ => break,
        };
        ensure!(!*slot, "duplicate approximation prefix");
        *slot = true;
        n += 1;
    }
    let operation = u32::from(insn.mnemonic.starts_with("rsqrt"));
    ensure!(
        raw.get(n) == Some(&0x0f)
            && raw.get(n + 1) == Some(&(if operation == 0 { 0x53 } else { 0x52 })),
        "wrong approximation opcode"
    );
    ensure!(
        insn.operands.len() == 2,
        "approximation requires two operands"
    );
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic approximation operand"
        );
    }
    let target = vector::xmm(&insn.operands[0])?;
    let modrm = *raw.get(n + 2).context("missing approximation ModRM")?;
    ensure!(
        target == format!("g_xmm{}", (modrm >> 3) & 7),
        "approximation destination encoding mismatch"
    );
    if modrm >> 6 == 3 {
        ensure!(
            vector::xmm(&insn.operands[1])? == format!("g_xmm{}", modrm & 7),
            "approximation source encoding mismatch"
        );
        ensure!(raw.len() == n + 3, "trailing approximation bytes");
    } else {
        ensure!(
            insn.operands[1].kind == "mem",
            "approximation memory encoding mismatch"
        );
    }
    let lanes = if scalar { 1 } else { 4 };
    let read = vector::read_vector(&insn.operands[1], &p, lanes, !scalar, insn.address)?;
    Ok(format!("{{ extern void gamepkg_x86_approx_f32(const uint32_t*,uint32_t*,uint32_t,uint32_t); {read} RecompXmm result={target}; gamepkg_x86_approx_f32(b.u,result.u,{lanes}u,{operation}u); {target}=result; }}\n"))
}
