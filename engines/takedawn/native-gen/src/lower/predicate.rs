//! Eight legacy SSE1 compare predicates. AVX/reserved immediate values fail.
use super::*;
const NAMES: [&str; 8] = [
    "cmpeq", "cmplt", "cmple", "cmpunord", "cmpneq", "cmpnlt", "cmpnle", "cmpord",
];
pub(super) fn supports(name: &str) -> bool {
    matches!(name, "cmpss" | "cmpps")
        || name
            .strip_suffix("ss")
            .or_else(|| name.strip_suffix("ps"))
            .is_some_and(|base| NAMES.contains(&base))
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    let scalar = insn.mnemonic.ends_with("ss");
    ensure!(
        !p.operand16 && p.mandatory == if scalar { Some(0xf3) } else { None },
        "invalid SSE1 predicate prefix"
    );
    let raw = hex::decode(&insn.bytes_hex)?;
    let start = raw
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67 | 0xf3))
        .count();
    ensure!(
        raw.len() >= start + 4 && raw[start..start + 2] == [0x0f, 0xc2],
        "wrong compare opcode"
    );
    let predicate = *raw.last().unwrap();
    ensure!(predicate < 8, "reserved legacy comparison predicate");
    let suffix = if scalar { "ss" } else { "ps" };
    if insn.mnemonic == format!("cmp{suffix}") {
        ensure!(
            insn.operands.len() == 3
                && insn.operands[2].kind == "imm"
                && insn.operands[2].imm == Some(u32::from(predicate)),
            "wrong compare immediate metadata"
        );
    } else {
        ensure!(
            insn.operands.len() == 2
                && insn.mnemonic == format!("{}{suffix}", NAMES[usize::from(predicate)]),
            "compare predicate/alias mismatch"
        );
    }
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic comparison expression"
        );
    }
    let target = vector::xmm(&insn.operands[0])?;
    let lanes = if scalar { 1 } else { 4 };
    let read = vector::read_vector(&insn.operands[1], &p, lanes, !scalar, insn.address)?;
    Ok(format!("{{ extern void gamepkg_x86_predicate_f32(const uint32_t*,const uint32_t*,uint32_t*,uint32_t,uint32_t,uint32_t,uint32_t); RecompXmm a={target}; {read} RecompXmm result=a; gamepkg_x86_predicate_f32(a.u,b.u,result.u,{lanes}u,{predicate}u,0x{:08x}u,ebp); {target}=result; }}\n",insn.address))
}
