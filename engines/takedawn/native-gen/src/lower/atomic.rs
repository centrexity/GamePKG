//! Typed exchange operations. Memory RMW uses one checked RAM transaction.
use super::*;
pub(super) fn supports(s: &str) -> bool {
    matches!(
        s.strip_prefix("lock ").unwrap_or(s),
        "xchg" | "xadd" | "cmpxchg"
    )
}
fn encoded_reg(index: u8, bytes: u8) -> &'static str {
    let names = match bytes {
        1 => ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"],
        2 => ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"],
        _ => ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"],
    };
    names[usize::from(index)]
}
fn reg_is(op: &Operand, index: u8, bytes: u8) -> Result<()> {
    ensure!(
        op.kind == "reg" && op.reg.as_deref() == Some(encoded_reg(index, bytes)),
        "exchange register encoding mismatch"
    );
    Ok(())
}
fn encoded_memory(
    raw: &[u8],
    pos: &mut usize,
    mode: u8,
    rm: u8,
    a16: bool,
    fs: bool,
    op: &Operand,
) -> Result<()> {
    let mut base = None;
    let mut index = None;
    let mut scale = 1;
    let mut absolute = false;
    let mut disp_bytes = if mode == 1 {
        1
    } else if mode == 2 {
        if a16 {
            2
        } else {
            4
        }
    } else {
        0
    };
    if a16 {
        let pair = match rm {
            0 => (Some("bx"), Some("si")),
            1 => (Some("bx"), Some("di")),
            2 => (Some("bp"), Some("si")),
            3 => (Some("bp"), Some("di")),
            4 => (Some("si"), None),
            5 => (Some("di"), None),
            6 if mode == 0 => {
                absolute = true;
                disp_bytes = 2;
                (None, None)
            }
            6 => (Some("bp"), None),
            _ => (Some("bx"), None),
        };
        (base, index) = pair;
    } else if rm == 4 {
        let sib = *raw.get(*pos).context("missing exchange SIB")?;
        *pos += 1;
        let sindex = (sib >> 3) & 7;
        let sbase = sib & 7;
        if sindex != 4 {
            index = Some(encoded_reg(sindex, 4));
            scale = 1 << (sib >> 6);
        }
        if sbase == 5 && mode == 0 {
            absolute = true;
            disp_bytes = 4;
        } else {
            base = Some(encoded_reg(sbase, 4));
        }
    } else if rm == 5 && mode == 0 {
        absolute = true;
        disp_bytes = 4;
    } else {
        base = Some(encoded_reg(rm, 4));
    }
    let d = raw
        .get(*pos..*pos + disp_bytes)
        .context("missing exchange displacement")?;
    *pos += disp_bytes;
    let disp = match d.len() {
        0 => 0,
        1 => i64::from(d[0] as i8),
        2 => {
            let v = u16::from_le_bytes([d[0], d[1]]);
            if absolute {
                i64::from(v)
            } else {
                i64::from(v as i16)
            }
        }
        _ => i64::from(i32::from_le_bytes([d[0], d[1], d[2], d[3]])),
    };
    ensure!(
        op.kind == "mem"
            && op.mem_base.as_deref() == base
            && op.mem_index.as_deref() == index
            && op.mem_scale == scale
            && op.mem_disp as u32 == disp as u32
            && op.mem_seg.as_deref() == if fs { Some("fs") } else { None },
        "exchange memory encoding mismatch"
    );
    Ok(())
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    ensure!(
        (1..=15).contains(&insn.size) && insn.bytes_hex.len() == usize::from(insn.size) * 2,
        "invalid exchange byte metadata"
    );
    ensure!(insn.operands.len() == 2, "exchange needs two operands");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic exchange expression"
        );
    }
    let raw = hex::decode(&insn.bytes_hex)?;
    let (mut word, mut a16, mut fs, mut locked) = (false, false, false, false);
    let mut n = 0;
    while let Some(&b) = raw.get(n) {
        let slot = match b {
            0x66 => &mut word,
            0x67 => &mut a16,
            0x64 => &mut fs,
            0xf0 => &mut locked,
            _ => break,
        };
        ensure!(!*slot, "duplicate exchange prefix");
        *slot = true;
        n += 1;
    }
    let opcode = *raw.get(n).context("missing exchange opcode")?;
    n += 1;
    let (kind, bytes, short) = match opcode {
        0x86 | 0x87 => (
            0,
            if opcode == 0x86 {
                1
            } else if word {
                2
            } else {
                4
            },
            false,
        ),
        0x90..=0x97 => (0, if word { 2 } else { 4 }, true),
        0x0f => {
            let op = *raw.get(n).context("missing exchange second opcode")?;
            n += 1;
            let k = match op {
                0xc0 | 0xc1 => 1,
                0xb0 | 0xb1 => 2,
                _ => bail!("not an exchange opcode"),
            };
            (
                k,
                if op & 1 == 0 {
                    1
                } else if word {
                    2
                } else {
                    4
                },
                false,
            )
        }
        _ => bail!("not an exchange opcode"),
    };
    ensure!(
        insn.mnemonic
            == format!(
                "{}{}",
                if locked { "lock " } else { "" },
                ["xchg", "xadd", "cmpxchg"][kind]
            ),
        "exchange mnemonic mismatch"
    );
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let memory = dst.kind == "mem";
    ensure!(!locked || memory, "LOCK requires memory");
    let source = operand_register(src)?;
    ensure!(
        source.width == bytes && width(dst)? == bytes,
        "exchange width mismatch"
    );
    if short {
        ensure!(!fs && !locked, "segment/LOCK on short register exchange");
        // Capstone prints the accumulator as the second operand in short forms.
        reg_is(src, 0, bytes)?;
        reg_is(dst, opcode & 7, bytes)?;
    } else {
        let modrm = *raw.get(n).context("missing exchange ModRM")?;
        n += 1;
        reg_is(src, (modrm >> 3) & 7, bytes)?;
        if modrm >> 6 == 3 {
            ensure!(!fs, "segment on register exchange");
            reg_is(dst, modrm & 7, bytes)?;
        } else {
            encoded_memory(&raw, &mut n, modrm >> 6, modrm & 7, a16, fs, dst)?;
        }
    }
    ensure!(n == raw.len(), "trailing exchange bytes");
    let p = Prefixes {
        operand16: word,
        address16: a16,
        mandatory: None,
    };
    let mask = mask(bytes)?;
    let sign = 1u32 << (bytes * 8 - 1);
    let mut code = format!(
        "uint32_t b = {}; uint32_t accumulator = g_eax & 0x{mask:08x}u; (void)accumulator; ",
        read_register(source)
    );
    if memory {
        let offset = address(dst, &p, true)?;
        code.push_str(&format!("uint32_t offset = {offset}; if (offset > 0x{:08x}u) {{ extern void gamepkg_x86_general_protection(uint32_t,uint32_t); gamepkg_x86_general_protection(0x{:08x}u,ebp); }} extern uint32_t gamepkg_x86_memory_rmw(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); uint32_t a = gamepkg_x86_memory_rmw({}, {bytes}u, {kind}u, b, accumulator, 0x{:08x}u, ebp); ",u32::MAX-u32::from(bytes)+1,insn.address,address(dst,&p,false)?,insn.address));
    } else {
        code.push_str(&format!("uint32_t a = {}; ", read(dst, &p)?));
    }
    if kind == 0 {
        // Snapshots precede all writes, including overlapping AL/AH and ESP.
        code.push_str(&write_register(source, "a"));
        if !memory {
            code.push_str(&write(dst, "b", &p)?);
        }
        code.push_str("(void)accumulator; ");
    } else {
        let (lhs, rhs, symbol, overflow, carry) = if kind == 1 {
            (
                "a",
                "b",
                "+",
                "~(lhs ^ rhs)",
                format!("wide > 0x{mask:08x}u"),
            )
        } else {
            ("accumulator", "a", "-", "(lhs ^ rhs)", "lhs < rhs".into())
        };
        code.push_str(&format!("uint32_t lhs = {lhs}, rhs = {rhs}; uint64_t wide = (uint64_t)lhs {symbol} (uint64_t)rhs; uint32_t r = (uint32_t)wide & 0x{mask:08x}u; uint32_t flags = (g_eflags & ~0x8d5u) | 2u | (({carry}) ? 1u : 0u) | (((0x9669u >> ((r ^ (r >> 4u)) & 15u)) & 1u) << 2u) | ((lhs ^ rhs ^ r) & 0x10u) | (r ? 0u : 0x40u) | ((r & 0x{sign:08x}u) ? 0x80u : 0u) | (({overflow} & (lhs ^ r) & 0x{sign:08x}u) ? 0x800u : 0u); "));
        if kind == 1 {
            code.push_str(&write_register(source, "a"));
            if !memory {
                code.push_str(&write(dst, "r", &p)?);
            }
        } else {
            code.push_str("if (accumulator == a) { ");
            if !memory {
                code.push_str(&write(dst, "b", &p)?);
            }
            code.push_str("} else { ");
            code.push_str(&write_register(
                Register {
                    name: "g_eax",
                    width: bytes,
                    shift: 0,
                },
                "a",
            ));
            code.push_str("} ");
        }
        code.push_str("g_eflags = flags;");
    }
    Ok(format!("{{ {code} }}\n"))
}
