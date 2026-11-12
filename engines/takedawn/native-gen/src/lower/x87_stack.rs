//! x87 stack motion, classification, sign, conditional move and exact rounding.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "fabs"
            | "fchs"
            | "fxch"
            | "ffree"
            | "fincstp"
            | "fdecstp"
            | "fxam"
            | "fnop"
            | "frndint"
            | "fxtract"
            | "fcmovb"
            | "fcmove"
            | "fcmovbe"
            | "fcmovu"
            | "fcmovnb"
            | "fcmovne"
            | "fcmovnbe"
            | "fcmovnu"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    let bytes = hex::decode(&insn.bytes_hex)?;
    ensure!(
        !p.operand16 && !p.address16 && bytes.len() == 2,
        "unqualified x87 stack prefix"
    );
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic x87 stack operand"
        );
    }
    let (first, second) = (bytes[0], bytes[1]);
    let (op, index, name) = match (first, second) {
        (0xd9, 0xe1) => (0, 0, "fabs"),
        (0xd9, 0xe0) => (1, 0, "fchs"),
        (0xd9, 0xc8..=0xcf) => (2, u32::from(second & 7), "fxch"),
        (0xdd, 0xc0..=0xc7) => (3, u32::from(second & 7), "ffree"),
        (0xd9, 0xf7) => (4, 0, "fincstp"),
        (0xd9, 0xf6) => (5, 0, "fdecstp"),
        (0xd9, 0xe5) => (6, 0, "fxam"),
        (0xd9, 0xd0) => (7, 0, "fnop"),
        (0xd9, 0xfc) => (8, 0, "frndint"),
        (0xd9, 0xf4) => (9, 0, "fxtract"),
        (0xda | 0xdb, 0xc0..=0xdf) => {
            let selector = ((second >> 3) & 3) + if first == 0xdb { 4 } else { 0 };
            let names = [
                "fcmovb", "fcmove", "fcmovbe", "fcmovu", "fcmovnb", "fcmovne", "fcmovnbe",
                "fcmovnu",
            ];
            (
                10 + u32::from(selector),
                u32::from(second & 7),
                names[usize::from(selector)],
            )
        }
        _ => bail!("unqualified x87 stack operation"),
    };
    ensure!(insn.mnemonic == name, "x87 stack mnemonic mismatch");
    let reg = format!("st({index})");
    let valid = if op == 3 {
        insn.operands.len() == 1
            && insn.operands[0].kind == "reg"
            && insn.operands[0].reg.as_deref() == Some(reg.as_str())
    } else if op == 2 || op >= 10 {
        insn.operands.len() == 2
            && insn.operands[0].kind == "reg"
            && insn.operands[0].reg.as_deref() == Some("st(0)")
            && insn.operands[1].kind == "reg"
            && insn.operands[1].reg.as_deref() == Some(reg.as_str())
    } else {
        insn.operands.is_empty()
    };
    ensure!(
        valid,
        "x87 stack operand/encoding mismatch for {name}: {:?}",
        insn.operands
    );
    let opcode = ((u32::from(first) & 7) << 8) | u32::from(second);
    Ok(format!("{{ extern void gamepkg_x86_x87_stack(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_stack({op}u,{index}u,g_eflags,0x{:08x}u,0x{opcode:03x}u,ebp); }}\n",insn.address))
}
