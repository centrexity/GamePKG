//! Ordered/unordered x87 comparisons with staged memory and deferred faults.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "fcom"
            | "fcomp"
            | "fcompp"
            | "fucom"
            | "fucomp"
            | "fucompp"
            | "fcomi"
            | "fcompi"
            | "fucomi"
            | "fucompi"
            | "ficom"
            | "ficomp"
            | "ftst"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(!p.operand16, "unqualified x87 comparison prefix");
    for operand in &insn.operands {
        ensure!(
            operand.expr.is_none() && operand.expr_src.is_none(),
            "synthetic x87 comparison"
        );
    }
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(bytes.len() >= start + 2, "truncated x87 comparison");
    let first = bytes[start];
    let second = bytes[start + 1];
    let opcode = ((u32::from(first) & 7) << 8) | u32::from(second);
    let (kind, source, pop, quiet, integer_flags, name, read, low, high, addr) = if second < 0xc0 {
        ensure!(
            insn.operands.len() == 1,
            "comparison requires one memory source"
        );
        let operand = &insn.operands[0];
        ensure!(
            operand.kind == "mem" && matches!(operand.mem_seg.as_deref(), None | Some("fs")),
            "unqualified x87 comparison segment"
        );
        let (kind, width, integer) = match first {
            0xd8 => (2, 4u32, false),
            0xdc => (3, 8, false),
            0xde => (4, 2, true),
            0xda => (5, 4, true),
            _ => bail!("invalid memory comparison opcode"),
        };
        let field = (second >> 3) & 7;
        ensure!(field == 2 || field == 3, "invalid comparison field");
        ensure!(
            u32::from(operand.mem_size) == width,
            "wrong comparison width"
        );
        let name = match (integer, field == 3) {
            (false, false) => "fcom",
            (false, true) => "fcomp",
            (true, false) => "ficom",
            (true, true) => "ficomp",
        };
        let offset = address(operand, &p, true)?;
        let metadata = x87_data_segment(operand);
        let base = if operand.mem_seg.as_deref() == Some("fs") {
            "g_fs_base"
        } else {
            "0u"
        };
        let read=format!("extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); extern void gamepkg_x86_memory_read(uint32_t,uint32_t[4],uint32_t,uint32_t,uint32_t); uint32_t offset={offset}; if(offset>0x{:08x}u) gamepkg_x86_general_protection(0x{:08x}u,ebp); uint32_t address=(uint32_t)({base}+offset),words[4]={{0,0,0,0}}; gamepkg_x86_memory_read(address,words,{width}u,0x{:08x}u,ebp); {metadata}",u32::MAX-(width-1),insn.address,insn.address);
        (
            kind,
            0,
            u32::from(field == 3),
            0,
            0,
            name,
            read,
            "words[0]",
            "words[1]",
            "address",
        )
    } else {
        ensure!(
            start == 0 && bytes.len() == 2 && !p.address16,
            "unqualified register comparison prefix"
        );
        let (kind, source, pop, quiet, integer_flags, name) = match (first, second) {
            (0xd9, 0xe4) => (1, 0, 0, 0, 0, "ftst"),
            (0xde, 0xd9) => (0, 1, 2, 0, 0, "fcompp"),
            (0xda, 0xe9) => (0, 1, 2, 1, 0, "fucompp"),
            (0xd8, 0xd0..=0xd7) => (0, u32::from(second & 7), 0, 0, 0, "fcom"),
            (0xd8, 0xd8..=0xdf) => (0, u32::from(second & 7), 1, 0, 0, "fcomp"),
            (0xdd, 0xe0..=0xe7) => (0, u32::from(second & 7), 0, 1, 0, "fucom"),
            (0xdd, 0xe8..=0xef) => (0, u32::from(second & 7), 1, 1, 0, "fucomp"),
            (0xdb, 0xf0..=0xf7) => (0, u32::from(second & 7), 0, 0, 1, "fcomi"),
            (0xdf, 0xf0..=0xf7) => (0, u32::from(second & 7), 1, 0, 1, "fcompi"),
            (0xdb, 0xe8..=0xef) => (0, u32::from(second & 7), 0, 1, 1, "fucomi"),
            (0xdf, 0xe8..=0xef) => (0, u32::from(second & 7), 1, 1, 1, "fucompi"),
            _ => bail!("unqualified x87 comparison encoding"),
        };
        let source_name = format!("st({source})");
        let valid = if kind == 1 || pop == 2 {
            insn.operands.is_empty()
        } else {
            insn.operands.len() == 1
                && insn.operands[0].kind == "reg"
                && insn.operands[0].reg.as_deref() == Some(source_name.as_str())
        };
        ensure!(valid, "wrong x87 comparison register metadata");
        (
            kind,
            source,
            pop,
            quiet,
            integer_flags,
            name,
            String::new(),
            "0u",
            "0u",
            "0u",
        )
    };
    ensure!(
        insn.mnemonic == name,
        "comparison mnemonic/encoding mismatch"
    );
    Ok(format!("{{ extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern uint32_t gamepkg_x86_x87_compare(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_wait(0x{:08x}u,ebp); {read}g_eflags=gamepkg_x86_x87_compare({kind}u,{source}u,{pop}u,{quiet}u,{integer_flags}u,g_eflags,{low},{high},0x{:08x}u,0x{opcode:03x}u,{addr},ebp); }}\n",insn.address,insn.address))
}
