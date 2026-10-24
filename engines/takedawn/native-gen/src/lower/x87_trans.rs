//! Transcendental entry points with reviewed numerical and commit protocols.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "f2xm1" | "fyl2x" | "fpatan" | "fsin" | "fcos" | "fsincos" | "fptan" | "fyl2xp1"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let _ = parse_prefixes(insn, false)?;
    let (op, bytes, opcode) = if insn.mnemonic == "f2xm1" {
        (0, "d9f0", 0x1f0)
    } else if insn.mnemonic == "fyl2x" {
        (1, "d9f1", 0x1f1)
    } else if insn.mnemonic == "fpatan" {
        (2, "d9f3", 0x1f3)
    } else if insn.mnemonic == "fsin" {
        (3, "d9fe", 0x1fe)
    } else if insn.mnemonic == "fcos" {
        (4, "d9ff", 0x1ff)
    } else if insn.mnemonic == "fsincos" {
        (5, "d9fb", 0x1fb)
    } else if insn.mnemonic == "fptan" {
        (6, "d9f2", 0x1f2)
    } else {
        (7, "d9f9", 0x1f9)
    };
    ensure!(
        insn.bytes_hex == bytes && insn.operands.is_empty(),
        "unqualified transcendental encoding/operands"
    );
    Ok(format!("{{ extern void gamepkg_x86_x87_trans(uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_trans({op}u,0x{:08x}u,0x{opcode:x}u,ebp); }}\n",insn.address))
}
