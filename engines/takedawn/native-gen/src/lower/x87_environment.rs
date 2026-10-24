//! Protected-mode x87 control/environment save images through whole RAM spans.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(name, "fldcw" | "fldenv" | "frstor" | "fnstenv" | "fnsave")
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(
        insn.operands.len() == 1,
        "environment operation needs one memory operand"
    );
    let operand = &insn.operands[0];
    ensure!(
        operand.kind == "mem"
            && operand.expr.is_none()
            && operand.expr_src.is_none()
            && matches!(operand.mem_seg.as_deref(), None | Some("fs")),
        "unqualified x87 environment operand"
    );
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x66 | 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 2 && bytes[start + 1] < 0xc0,
        "invalid x87 environment encoding"
    );
    let field = (bytes[start + 1] >> 3) & 7;
    let env = if p.operand16 { 14u32 } else { 28 };
    let (op, width, name) = match (bytes[start], field) {
        (0xd9, 5) => (0, 2, "fldcw"),
        (0xd9, 4) => (1, env, "fldenv"),
        (0xdd, 4) => (2, env + 80, "frstor"),
        (0xd9, 6) => (3, env, "fnstenv"),
        (0xdd, 6) => (4, env + 80, "fnsave"),
        _ => bail!("unqualified x87 environment operation"),
    };
    ensure!(
        insn.mnemonic == name,
        "environment mnemonic/encoding mismatch"
    );
    // Capstone labels FRSTOR/FNSAVE as dword operands. Their complete image
    // width comes from the validated opcode and operand-size prefix.
    ensure!(
        u32::from(operand.mem_size)
            == if op == 0 {
                2
            } else if op == 2 || op == 4 {
                4
            } else {
                28
            },
        "environment width mismatch: {} versus {width}",
        operand.mem_size
    );
    let offset = address(operand, &p, true)?;
    let base = if operand.mem_seg.as_deref() == Some("fs") {
        "g_fs_base"
    } else {
        "0u"
    };
    let waiting = if op <= 2 {
        format!("extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); gamepkg_x86_x87_wait(0x{:08x}u,ebp); ",insn.address)
    } else {
        String::new()
    };
    Ok(format!("{{ {waiting}extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); extern void gamepkg_x86_x87_environment(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); uint32_t offset={offset}; if(offset>0x{:08x}u) gamepkg_x86_general_protection(0x{:08x}u,ebp); uint32_t address=(uint32_t)({base}+offset); gamepkg_x86_x87_environment({op}u,{}u,address,0x{:08x}u,ebp); }}\n",u32::MAX-(width-1),insn.address,u8::from(p.operand16),insn.address))
}
