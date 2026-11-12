//! Implicit-register strings, routed to checked RAM with explicit fault sites.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    let base = name
        .strip_prefix("rep ")
        .or_else(|| name.strip_prefix("repe "))
        .or_else(|| name.strip_prefix("repne "))
        .unwrap_or(name);
    matches!(
        base,
        "movsb"
            | "movsw"
            | "movsd"
            | "stosb"
            | "stosw"
            | "stosd"
            | "lodsb"
            | "lodsw"
            | "lodsd"
            | "cmpsb"
            | "cmpsw"
            | "cmpsd"
            | "scasb"
            | "scasw"
            | "scasd"
    )
}
fn memory(op: &Operand, base: &str, segment: Option<&str>, width: u8) -> Result<()> {
    ensure!(
        op.kind == "mem"
            && op.mem_base.as_deref() == Some(base)
            && op.mem_index.is_none()
            && op.mem_scale == 1
            && op.mem_disp == 0
            && op.mem_size == width
            && op.mem_seg.as_deref() == segment,
        "wrong implicit string memory operand"
    );
    Ok(())
}
fn accumulator(op: &Operand, width: u8) -> Result<()> {
    let expected = match width {
        1 => "al",
        2 => "ax",
        _ => "eax",
    };
    ensure!(
        op.kind == "reg" && op.reg.as_deref() == Some(expected),
        "wrong string accumulator"
    );
    Ok(())
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    instruction_profile(insn, false)
}
pub(super) fn controlled_instruction(insn: &Instruction) -> Result<String> {
    instruction_profile(insn, true)
}
fn instruction_profile(insn: &Instruction, controlled: bool) -> Result<String> {
    ensure!(
        (1..=15).contains(&insn.size) && insn.bytes_hex.len() == usize::from(insn.size) * 2,
        "invalid string byte metadata"
    );
    let raw = hex::decode(&insn.bytes_hex)?;
    let (mut word, mut a16, mut fs, mut repeat) = (false, false, false, 0u8);
    let mut n = 0;
    for &b in &raw {
        match b {
            0x66 => {
                ensure!(!word, "duplicate operand prefix");
                word = true;
            }
            0x67 => {
                ensure!(!a16, "duplicate address prefix");
                a16 = true;
            }
            0x64 => {
                ensure!(!fs, "duplicate segment prefix");
                fs = true;
            }
            0xf3 | 0xf2 => {
                ensure!(repeat == 0, "duplicate repeat prefix");
                repeat = if b == 0xf3 { 1 } else { 2 };
            }
            _ => break,
        }
        n += 1;
    }
    ensure!(raw.len() == n + 1, "unqualified string encoding/prefix");
    let opcode = raw[n];
    let op = match opcode {
        0xa4 | 0xa5 => 0,
        0xaa | 0xab => 1,
        0xac | 0xad => 2,
        0xa6 | 0xa7 => 3,
        0xae | 0xaf => 4,
        _ => bail!("not a string opcode"),
    };
    ensure!(repeat != 2 || op >= 3, "REPNE requires a compare/scan");
    let width = if opcode & 1 == 0 {
        1
    } else if word {
        2
    } else {
        4
    };
    let stem = ["movs", "stos", "lods", "cmps", "scas"][op];
    let suffix = match width {
        1 => "b",
        2 => "w",
        _ => "d",
    };
    let prefix = match repeat {
        0 => "",
        1 if op >= 3 => "repe ",
        1 => "rep ",
        _ => "repne ",
    };
    ensure!(
        insn.mnemonic == format!("{prefix}{stem}{suffix}") && insn.operands.len() == 2,
        "string mnemonic/operand mismatch"
    );
    for p in &insn.operands {
        ensure!(
            p.expr.is_none() && p.expr_src.is_none(),
            "synthetic string expression"
        );
    }
    let si = if a16 { "si" } else { "esi" };
    let di = if a16 { "di" } else { "edi" };
    let srcseg = if fs { Some("fs") } else { None };
    let a = &insn.operands[0];
    let b = &insn.operands[1];
    match op {
        0 => {
            memory(a, di, Some("es"), width)?;
            memory(b, si, srcseg, width)?;
        }
        1 => {
            memory(a, di, Some("es"), width)?;
            accumulator(b, width)?;
        }
        2 => {
            accumulator(a, width)?;
            memory(b, si, srcseg, width)?;
        }
        3 => {
            memory(a, si, srcseg, width)?;
            memory(b, di, Some("es"), width)?;
        }
        _ => {
            accumulator(a, width)?;
            memory(b, di, Some("es"), width)?;
        }
    }
    if controlled {
        Ok(format!("{{ extern int gamepkg_x86_string_controlled(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,int(*)(uint32_t,uint32_t)); if(gamepkg_x86_string_controlled({op}u,{width}u,{}u,{repeat}u,{}u,0x{:08x}u,ebp,gamepkg_x86_dispatch_rep_checkpoint))return; }}\n",u8::from(a16),u8::from(fs),insn.address))
    } else {
        Ok(format!("{{ extern void gamepkg_x86_string(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_string({op}u,{width}u,{}u,{repeat}u,{}u,0x{:08x}u,ebp); }}\n",u8::from(a16),u8::from(fs),insn.address))
    }
}
