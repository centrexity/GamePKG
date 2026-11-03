//! Shared physical x87/MMX register state. No independent MMX shadow file.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "fninit"
            | "fnclex"
            | "fnstcw"
            | "fnstsw"
            | "wait"
            | "fwait"
            | "emms"
            | "movd"
            | "movq"
            | "pand"
            | "pandn"
            | "por"
            | "pxor"
            | "paddb"
            | "paddw"
            | "paddd"
            | "psubb"
            | "psubw"
            | "psubd"
            | "pcmpeqb"
            | "pcmpeqw"
            | "pcmpeqd"
            | "pcmpgtb"
            | "pcmpgtw"
            | "pcmpgtd"
            | "paddsb"
            | "paddsw"
            | "paddusb"
            | "paddusw"
            | "psubsb"
            | "psubsw"
            | "psubusb"
            | "psubusw"
            | "pmullw"
            | "pmulhw"
            | "pmulhuw"
            | "pmaddwd"
            | "packsswb"
            | "packssdw"
            | "packuswb"
            | "punpcklbw"
            | "punpcklwd"
            | "punpckldq"
            | "punpckhbw"
            | "punpckhwd"
            | "punpckhdq"
            | "psllw"
            | "pslld"
            | "psllq"
            | "psrlw"
            | "psrld"
            | "psrlq"
            | "psraw"
            | "psrad"
            | "pavgb"
            | "pavgw"
            | "pminub"
            | "pmaxub"
            | "pminsw"
            | "pmaxsw"
            | "psadbw"
            | "pshufw"
            | "pinsrw"
            | "pextrw"
            | "pmovmskb"
            | "cvtpi2ps"
            | "cvtps2pi"
            | "cvttps2pi"
    )
}
pub(super) fn mm(op: &Operand) -> Result<u32> {
    ensure!(op.kind == "reg", "expected MMX register");
    match op.reg.as_deref() {
        Some("mm0") => Ok(0),
        Some("mm1") => Ok(1),
        Some("mm2") => Ok(2),
        Some("mm3") => Ok(3),
        Some("mm4") => Ok(4),
        Some("mm5") => Ok(5),
        Some("mm6") => Ok(6),
        Some("mm7") => Ok(7),
        _ => bail!("unsupported MMX register"),
    }
}
fn mm_read(op: &Operand, p: &Prefixes, width: u8) -> Result<String> {
    if op.kind == "reg" {
        return Ok(format!(
            "uint64_t right=gamepkg_x86_mmx_read({}u); ",
            mm(op)?
        ));
    }
    ensure!(
        op.kind == "mem" && op.mem_size == width,
        "wrong MMX source width"
    );
    let mut code = format!(
        "uint32_t address={}; uint32_t lo=xbox_MmioLoad(address,4); ",
        address(op, p, false)?
    );
    if width == 4 {
        code.push_str("uint64_t right=lo; ");
    } else {
        code.push_str("uint32_t hi=xbox_MmioLoad((uint32_t)(address+4u),4); uint64_t right=(uint64_t)lo|((uint64_t)hi<<32u); ");
    }
    Ok(code)
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(!p.operand16, "unsupported MMX/x87 operand prefix");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic MMX/x87 expression"
        );
    }
    let site = format!("0x{:08x}u,ebp", insn.address);
    let name = insn.mnemonic.as_str();
    let declarations="extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern uint64_t gamepkg_x86_mmx_read(uint32_t); extern void gamepkg_x86_mmx_commit(uint32_t,uint64_t); extern void gamepkg_x86_mmx_read_commit(void); ";
    let body = match name {
        "fninit" | "fnclex" | "emms" | "wait" | "fwait" => {
            ensure!(insn.operands.is_empty(), "invalid FP control operands");
            match name {
                "fninit"=>"extern void gamepkg_x86_x87_init(void); gamepkg_x86_x87_init();".into(),
                "fnclex"=>"extern void gamepkg_x86_x87_clear(void); gamepkg_x86_x87_clear();".into(),
                "emms"=>format!("gamepkg_x86_x87_wait({site}); extern void gamepkg_x86_mmx_empty(void); gamepkg_x86_mmx_empty();"),
                _=>format!("gamepkg_x86_x87_wait({site});")
            }
        }
        "fnstcw" | "fnstsw" => {
            ensure!(insn.operands.len() == 1, "invalid FP store operands");
            let op = &insn.operands[0];
            let function = if name == "fnstcw" {
                "gamepkg_x86_x87_control"
            } else {
                "gamepkg_x86_x87_status"
            };
            let store = if op.kind == "mem" {
                ensure!(op.mem_size == 2, "FP control/status requires m16");
                format!(
                    "uint32_t address={}; xbox_MmioStore(address,{function}(),2);",
                    address(op, &p, false)?
                )
            } else {
                ensure!(
                    name == "fnstsw" && op.reg.as_deref() == Some("ax"),
                    "FNSTSW requires AX/m16"
                );
                write_register(operand_register(op)?, &format!("{function}()"))
            };
            format!("extern uint32_t {function}(void); {store}")
        }
        "psllw" | "pslld" | "psllq" | "psrlw" | "psrld" | "psrlq" | "psraw" | "psrad" => {
            shift(insn, &p, &site)?
        }
        "pavgb" | "pavgw" | "pminub" | "pmaxub" | "pminsw" | "pmaxsw" | "psadbw" | "pshufw"
        | "pinsrw" | "pextrw" | "pmovmskb" => lanes(insn, &p, &site)?,
        "cvtpi2ps" | "cvtps2pi" | "cvttps2pi" => conversion(insn, &p, &site)?,
        "movd" => {
            ensure!(insn.operands.len() == 2, "MOVD requires two operands");
            let dst = &insn.operands[0];
            let src = &insn.operands[1];
            let body = if let Ok(reg) = mm(dst) {
                ensure!(
                    (src.kind == "reg" && operand_register(src)?.width == 4)
                        || (src.kind == "mem" && src.mem_size == 4),
                    "MOVD requires r32/m32"
                );
                format!(
                    "uint32_t value={}; gamepkg_x86_mmx_commit({reg}u,(uint64_t)value);",
                    read(src, &p)?
                )
            } else {
                let reg = mm(src)?;
                ensure!(
                    (dst.kind == "reg" && operand_register(dst)?.width == 4)
                        || (dst.kind == "mem" && dst.mem_size == 4),
                    "MOVD requires r32/m32"
                );
                format!("uint32_t value=(uint32_t)gamepkg_x86_mmx_read({reg}u); {} gamepkg_x86_mmx_read_commit();",write(dst,"value",&p)?)
            };
            format!("gamepkg_x86_x87_wait({site}); {body}")
        }
        "movq" => {
            ensure!(insn.operands.len() == 2, "MOVQ requires two operands");
            let dst = &insn.operands[0];
            let src = &insn.operands[1];
            let body = if dst.kind == "mem" {
                ensure!(dst.mem_size == 8, "MOVQ requires m64");
                let reg = mm(src)?;
                format!("uint64_t value=gamepkg_x86_mmx_read({reg}u); uint32_t address={}; xbox_MmioStore(address,(uint32_t)value,4); xbox_MmioStore((uint32_t)(address+4u),(uint32_t)(value>>32u),4); gamepkg_x86_mmx_read_commit();",address(dst,&p,false)?)
            } else {
                format!(
                    "{}gamepkg_x86_mmx_commit({}u,right);",
                    mm_read(src, &p, 8)?,
                    mm(dst)?
                )
            };
            format!("gamepkg_x86_x87_wait({site}); {body}")
        }
        _ => {
            ensure!(insn.operands.len() == 2, "MMX binary requires two operands");
            let reg = mm(&insn.operands[0])?;
            let (operation, width) = match name {
                "pand" => (0, 64),
                "pandn" => (1, 64),
                "por" => (2, 64),
                "pxor" => (3, 64),
                "paddb" => (4, 8),
                "paddw" => (4, 16),
                "paddd" => (4, 32),
                "psubb" => (5, 8),
                "psubw" => (5, 16),
                "psubd" => (5, 32),
                "pcmpeqb" => (6, 8),
                "pcmpeqw" => (6, 16),
                "pcmpeqd" => (6, 32),
                "pcmpgtb" => (7, 8),
                "pcmpgtw" => (7, 16),
                "pcmpgtd" => (7, 32),
                "paddsb" => (8, 8),
                "paddsw" => (8, 16),
                "paddusb" => (9, 8),
                "paddusw" => (9, 16),
                "psubsb" => (10, 8),
                "psubsw" => (10, 16),
                "psubusb" => (11, 8),
                "psubusw" => (11, 16),
                "pmullw" => (12, 16),
                "pmulhw" => (13, 16),
                "pmulhuw" => (14, 16),
                "pmaddwd" => (15, 16),
                "packsswb" => (16, 16),
                "packssdw" => (16, 32),
                "packuswb" => (17, 16),
                "punpcklbw" => (18, 8),
                "punpcklwd" => (18, 16),
                "punpckldq" => (18, 32),
                "punpckhbw" => (19, 8),
                "punpckhwd" => (19, 16),
                "punpckhdq" => (19, 32),
                _ => bail!("unsupported MMX operation"),
            };
            format!("extern uint64_t gamepkg_x86_mmx_binary(uint64_t,uint64_t,uint32_t,uint32_t); gamepkg_x86_x87_wait({site}); uint64_t left=gamepkg_x86_mmx_read({reg}u); {}uint64_t result=gamepkg_x86_mmx_binary(left,right,{operation}u,{width}u); gamepkg_x86_mmx_commit({reg}u,result);",mm_read(&insn.operands[1],&p,if operation==18 {4} else {8})?)
        }
    };
    Ok(format!("{{ {declarations}{body} }}\n"))
}

fn shift(insn: &Instruction, p: &Prefixes, site: &str) -> Result<String> {
    ensure!(insn.operands.len() == 2, "MMX shift requires two operands");
    let dst = mm(&insn.operands[0])?;
    let src = &insn.operands[1];
    let (op, width, variable, immediate, field) = match insn.mnemonic.as_str() {
        "psllw" => (0, 16, 0xf1, 0x71, 6),
        "pslld" => (0, 32, 0xf2, 0x72, 6),
        "psllq" => (0, 64, 0xf3, 0x73, 6),
        "psrlw" => (1, 16, 0xd1, 0x71, 2),
        "psrld" => (1, 32, 0xd2, 0x72, 2),
        "psrlq" => (1, 64, 0xd3, 0x73, 2),
        "psraw" => (2, 16, 0xe1, 0x71, 4),
        "psrad" => (2, 32, 0xe2, 0x72, 4),
        _ => bail!("invalid MMX shift"),
    };
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 3 && bytes[start] == 0x0f,
        "invalid MMX shift encoding"
    );
    let count = if src.kind == "imm" {
        let value = src.imm.context("missing MMX shift count")?;
        ensure!(
            value <= 255
                && bytes.len() == start + 4
                && bytes[start + 1] == immediate
                && u32::from(bytes[start + 2]) == 0xc0 | (field << 3) | dst
                && u32::from(bytes[start + 3]) == value,
            "invalid MMX immediate shift"
        );
        format!("uint64_t right={value}u; ")
    } else {
        ensure!(
            bytes[start + 1] == variable && u32::from((bytes[start + 2] >> 3) & 7) == dst,
            "invalid MMX variable shift"
        );
        if src.kind == "reg" {
            ensure!(
                bytes.len() == start + 3
                    && bytes[start + 2] >= 0xc0
                    && u32::from(bytes[start + 2] & 7) == mm(src)?,
                "invalid MMX shift register"
            );
        } else {
            ensure!(
                src.kind == "mem" && bytes[start + 2] < 0xc0,
                "invalid MMX shift memory"
            );
        }
        mm_read(src, p, 8)?
    };
    Ok(format!("extern uint64_t gamepkg_x86_mmx_shift(uint64_t,uint64_t,uint32_t,uint32_t); gamepkg_x86_x87_wait({site}); uint64_t left=gamepkg_x86_mmx_read({dst}u); {count}uint64_t result=gamepkg_x86_mmx_shift(left,right,{op}u,{width}u); gamepkg_x86_mmx_commit({dst}u,result);"))
}

fn gpr_code(op: &Operand) -> Result<u32> {
    ensure!(
        op.kind == "reg" && operand_register(op)?.width == 4,
        "requires r32"
    );
    ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi"]
        .iter()
        .position(|n| Some(*n) == op.reg.as_deref())
        .map(|n| n as u32)
        .context("invalid r32")
}
fn lanes(insn: &Instruction, p: &Prefixes, site: &str) -> Result<String> {
    let (op, opcode) = match insn.mnemonic.as_str() {
        "pavgb" => (0, 0xe0),
        "pavgw" => (1, 0xe3),
        "pminub" => (2, 0xda),
        "pmaxub" => (3, 0xde),
        "pminsw" => (4, 0xea),
        "pmaxsw" => (5, 0xee),
        "psadbw" => (6, 0xf6),
        "pshufw" => (7, 0x70),
        "pinsrw" => (8, 0xc4),
        "pextrw" => (9, 0xc5),
        "pmovmskb" => (10, 0xd7),
        _ => bail!("invalid MMX lane operation"),
    };
    ensure!(
        insn.operands.len() == if (7..=9).contains(&op) { 3 } else { 2 },
        "wrong MMX lane operand count"
    );
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 3 && bytes[start] == 0x0f && bytes[start + 1] == opcode,
        "invalid MMX lane encoding"
    );
    let control = if (7..=9).contains(&op) {
        let imm = &insn.operands[2];
        let value = imm.imm.context("missing MMX lane immediate")?;
        ensure!(
            imm.kind == "imm"
                && value <= 255
                && bytes.len() >= start + 4
                && u32::from(*bytes.last().unwrap()) == value,
            "wrong MMX lane immediate"
        );
        value
    } else {
        0
    };
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let field = u32::from((bytes[start + 2] >> 3) & 7);
    let rm = u32::from(bytes[start + 2] & 7);
    let register_length = start + if (7..=9).contains(&op) { 4 } else { 3 };
    let result = if op >= 9 {
        ensure!(
            field == gpr_code(dst)?
                && bytes[start + 2] >= 0xc0
                && rm == mm(src)?
                && bytes.len() == register_length,
            "invalid MMX extract/mask form"
        );
        format!("uint64_t left=gamepkg_x86_mmx_read({rm}u); uint32_t result=(uint32_t)gamepkg_x86_mmx_lanes(left,0u,{op}u,{control}u); {} gamepkg_x86_mmx_read_commit();",write_register(operand_register(dst)?,"result"))
    } else {
        let reg = mm(dst)?;
        ensure!(field == reg, "wrong MMX lane destination");
        let read = if op == 8 {
            if src.kind == "reg" {
                ensure!(
                    bytes[start + 2] >= 0xc0
                        && rm == gpr_code(src)?
                        && bytes.len() == register_length,
                    "invalid PINSRW source"
                );
            } else {
                ensure!(
                    src.kind == "mem" && src.mem_size == 2 && bytes[start + 2] < 0xc0,
                    "PINSRW requires r32/m16"
                );
            }
            format!("uint32_t right={}; ", read(src, p)?)
        } else {
            if src.kind == "reg" {
                ensure!(
                    bytes[start + 2] >= 0xc0 && rm == mm(src)? && bytes.len() == register_length,
                    "invalid MMX lane source"
                );
            } else {
                ensure!(
                    src.kind == "mem" && bytes[start + 2] < 0xc0,
                    "invalid MMX lane memory"
                );
            }
            mm_read(src, p, 8)?
        };
        let left = if op == 7 {
            "right".into()
        } else {
            format!("gamepkg_x86_mmx_read({reg}u)")
        };
        format!("{read}uint64_t left={left}; uint64_t result=gamepkg_x86_mmx_lanes(left,right,{op}u,{control}u); gamepkg_x86_mmx_commit({reg}u,result);")
    };
    Ok(format!("extern uint64_t gamepkg_x86_mmx_lanes(uint64_t,uint64_t,uint32_t,uint32_t); gamepkg_x86_x87_wait({site}); {result}"))
}

fn conversion(insn: &Instruction, p: &Prefixes, site: &str) -> Result<String> {
    ensure!(
        insn.operands.len() == 2,
        "MMX conversion requires two operands"
    );
    let (op, opcode) = match insn.mnemonic.as_str() {
        "cvtpi2ps" => (0, 0x2a),
        "cvtps2pi" => (1, 0x2d),
        "cvttps2pi" => (2, 0x2c),
        _ => bail!("unsupported MMX conversion"),
    };
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 3 && bytes[start] == 0x0f && bytes[start + 1] == opcode,
        "invalid MMX conversion encoding"
    );
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let xmm_code = |operand: &Operand| -> Result<u32> {
        let name = vector::xmm(operand)?;
        Ok(u32::from(name.as_bytes()[5] - b'0'))
    };
    let target = if op == 0 { xmm_code(dst)? } else { mm(dst)? };
    ensure!(
        u32::from((bytes[start + 2] >> 3) & 7) == target,
        "wrong conversion destination"
    );
    let (read, enter) = if src.kind == "reg" {
        ensure!(
            bytes.len() == start + 3 && bytes[start + 2] >= 0xc0,
            "invalid conversion register encoding"
        );
        let index = if op == 0 { mm(src)? } else { xmm_code(src)? };
        ensure!(
            u32::from(bytes[start + 2] & 7) == index,
            "wrong conversion source"
        );
        (
            if op == 0 {
                format!("uint64_t value=gamepkg_x86_mmx_read({index}u); ")
            } else {
                let source = vector::xmm(src)?;
                format!("uint32_t lo={source}.u[0],hi={source}.u[1]; uint64_t value=(uint64_t)lo|((uint64_t)hi<<32u); ")
            },
            1,
        )
    } else {
        ensure!(
            src.kind == "mem" && src.mem_size == 8 && bytes[start + 2] < 0xc0,
            "conversion requires m64"
        );
        (format!("uint32_t address={}; uint32_t lo=xbox_MmioLoad(address,4); uint32_t hi=xbox_MmioLoad((uint32_t)(address+4u),4); uint64_t value=(uint64_t)lo|((uint64_t)hi<<32u); ",address(src,p,false)?),if op==0{0}else{1})
    };
    let wait = if enter != 0 {
        format!("gamepkg_x86_x87_wait({site}); ")
    } else {
        String::new()
    };
    let commit = if op == 0 {
        let target = vector::xmm(dst)?;
        format!("{target}.u[0]=(uint32_t)result; {target}.u[1]=(uint32_t)(result>>32u);")
    } else {
        format!("gamepkg_x86_mmx_commit({target}u,result);")
    };
    Ok(format!("extern uint64_t gamepkg_x86_convert_mmx(uint64_t,uint32_t,uint32_t,uint32_t,uint32_t); {wait}{read}uint64_t result=gamepkg_x86_convert_mmx(value,{op}u,{enter}u,{site}); {commit}"))
}
