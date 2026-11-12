//! Register-stack arithmetic through the shared architectural host service.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "fadd"
            | "faddp"
            | "fmul"
            | "fmulp"
            | "fsub"
            | "fsubp"
            | "fsubr"
            | "fsubrp"
            | "fdiv"
            | "fdivp"
            | "fdivr"
            | "fdivrp"
            | "fsqrt"
            | "fscale"
            | "fprem"
            | "fprem1"
            | "fiadd"
            | "fimul"
            | "fisub"
            | "fisubr"
            | "fidiv"
            | "fidivr"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    if matches!(insn.mnemonic.as_str(), "fscale" | "fprem" | "fprem1") {
        let (bytes, operation, opcode) = match insn.mnemonic.as_str() {
            "fscale" => ("d9fd", 5, 0x1fd),
            "fprem" => ("d9f8", 6, 0x1f8),
            _ => ("d9f5", 7, 0x1f5),
        };
        let _ = parse_prefixes(insn, false)?;
        ensure!(
            insn.bytes_hex == bytes && insn.operands.is_empty(),
            "unqualified fixed-operand x87 encoding/operands"
        );
        return Ok(format!("{{ extern void gamepkg_x86_x87_arithmetic(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_arithmetic({operation}u,0u,1u,0u,0u,0x{:08x}u,0x{opcode:03x}u,ebp); }}\n",insn.address));
    }
    if insn.operands.iter().any(|op| op.kind == "mem") {
        return memory(insn);
    }
    let p = parse_prefixes(insn, false)?;
    ensure!(!p.operand16 && !p.address16, "unqualified x87 prefix");
    ensure!(
        insn.operands.len() <= 2,
        "invalid x87 register operand count"
    );
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic x87 operand"
        );
        ensure!(
            operand.kind == "reg"
                && matches!(
                    operand.reg.as_deref(),
                    Some(
                        "st(0)"
                            | "st(1)"
                            | "st(2)"
                            | "st(3)"
                            | "st(4)"
                            | "st(5)"
                            | "st(6)"
                            | "st(7)"
                    )
                ),
            "register arithmetic requires x87 stack operands"
        );
    }
    let bytes = hex::decode(&insn.bytes_hex)?;
    ensure!(bytes.len() == 2, "unqualified x87 encoding");
    let (op, dst, src, reverse, pop, name) = if bytes == [0xd9, 0xfa] {
        (4, 0, 0, false, false, "fsqrt")
    } else {
        ensure!(
            matches!(bytes[0], 0xd8 | 0xdc | 0xde) && bytes[1] >= 0xc0,
            "unqualified x87 register operation"
        );
        let field = (bytes[1] >> 3) & 7;
        let index = bytes[1] & 7;
        let stack_dst = bytes[0] != 0xd8;
        let pop = bytes[0] == 0xde;
        let (op, reverse, plain, popping) = match field {
            0 => (0, false, "fadd", "faddp"),
            1 => (2, false, "fmul", "fmulp"),
            4 => (
                1,
                stack_dst,
                if stack_dst { "fsubr" } else { "fsub" },
                if stack_dst { "fsubrp" } else { "fsubp" },
            ),
            5 => (
                1,
                !stack_dst,
                if stack_dst { "fsub" } else { "fsubr" },
                if stack_dst { "fsubp" } else { "fsubrp" },
            ),
            6 => (
                3,
                stack_dst,
                if stack_dst { "fdivr" } else { "fdiv" },
                if stack_dst { "fdivrp" } else { "fdivp" },
            ),
            7 => (
                3,
                !stack_dst,
                if stack_dst { "fdiv" } else { "fdivr" },
                if stack_dst { "fdivp" } else { "fdivrp" },
            ),
            _ => bail!("unqualified x87 opcode"),
        };
        (
            op,
            if stack_dst { index } else { 0 },
            if stack_dst { 0 } else { index },
            reverse,
            pop,
            if pop { popping } else { plain },
        )
    };
    ensure!(insn.mnemonic == name, "x87 mnemonic/encoding mismatch");
    let opcode = ((u32::from(bytes[0]) & 7) << 8) | u32::from(bytes[1]);
    Ok(format!("{{ extern void gamepkg_x86_x87_arithmetic(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_arithmetic({op}u,{dst}u,{src}u,{}u,{}u,0x{:08x}u,0x{opcode:03x}u,ebp); }}\n",u8::from(reverse),u8::from(pop),insn.address))
}

fn memory(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(
        !p.operand16 && insn.operands.len() == 1,
        "invalid x87 memory operands"
    );
    let operand = &insn.operands[0];
    ensure!(
        operand.kind == "mem" && operand.expr.is_none() && operand.expr_src.is_none(),
        "synthetic x87 memory operand"
    );
    ensure!(
        matches!(operand.mem_seg.as_deref(), None | Some("fs")),
        "unqualified x87 memory segment"
    );
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 2 && bytes[start + 1] < 0xc0,
        "invalid x87 memory encoding"
    );
    let (kind, width, integer) = match bytes[start] {
        0xd8 => (0, 4u32, false),
        0xdc => (1, 8, false),
        0xde => (2, 2, true),
        0xda => (3, 4, true),
        _ => bail!("unqualified x87 memory opcode"),
    };
    let (op, reverse, name) = match (bytes[start + 1] >> 3) & 7 {
        0 => (0, 0, if integer { "fiadd" } else { "fadd" }),
        1 => (2, 0, if integer { "fimul" } else { "fmul" }),
        4 => (1, 0, if integer { "fisub" } else { "fsub" }),
        5 => (1, 1, if integer { "fisubr" } else { "fsubr" }),
        6 => (3, 0, if integer { "fidiv" } else { "fdiv" }),
        7 => (3, 1, if integer { "fidivr" } else { "fdivr" }),
        _ => bail!("unqualified x87 memory operation"),
    };
    ensure!(
        insn.mnemonic == name && u32::from(operand.mem_size) == width,
        "x87 memory mnemonic/width mismatch"
    );
    let opcode = ((u32::from(bytes[start]) & 7) << 8) | u32::from(bytes[start + 1]);
    let offset = address(operand, &p, true)?;
    let metadata = x87_data_segment(operand);
    let base = if operand.mem_seg.as_deref() == Some("fs") {
        "g_fs_base"
    } else {
        "0u"
    };
    let site = format!("0x{:08x}u,ebp", insn.address);
    Ok(format!("{{ extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); extern void gamepkg_x86_memory_read(uint32_t,uint32_t[4],uint32_t,uint32_t,uint32_t); extern void gamepkg_x86_x87_memory_arithmetic(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_wait({site}); uint32_t offset={offset}; if(offset>0x{:08x}u) gamepkg_x86_general_protection({site}); uint32_t address=(uint32_t)({base}+offset),words[4]={{0,0,0,0}}; gamepkg_x86_memory_read(address,words,{width}u,{site}); {metadata}gamepkg_x86_x87_memory_arithmetic({op}u,{reverse}u,{kind}u,words[0],words[1],0x{:08x}u,0x{opcode:03x}u,address,ebp); }}\n", u32::MAX-(width-1), insn.address))
}
