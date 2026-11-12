//! Exact x87 loads through staged logical memory reads and shared stack state.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "fld"
            | "fild"
            | "fbld"
            | "fld1"
            | "fldl2t"
            | "fldl2e"
            | "fldpi"
            | "fldlg2"
            | "fldln2"
            | "fldz"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(!p.operand16, "unqualified x87 load operand prefix");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic x87 load"
        );
    }
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(bytes.len() >= start + 2, "truncated x87 load");
    let first = bytes[start];
    let second = bytes[start + 1];
    let opcode = ((u32::from(first) & 7) << 8) | u32::from(second);
    let mut read = String::new();
    let mut addr = "0u".to_string();
    let (kind, low, high, upper): (u32, String, String, String) = if first == 0xd9
        && (0xe8..=0xee).contains(&second)
    {
        let names = [
            "fld1", "fldl2t", "fldl2e", "fldpi", "fldlg2", "fldln2", "fldz",
        ];
        ensure!(
            insn.mnemonic == names[usize::from(second - 0xe8)]
                && insn.operands.is_empty()
                && bytes.len() == start + 2,
            "invalid x87 constant"
        );
        (
            u32::from(second - 0xe8) + 7,
            "0u".into(),
            "0u".into(),
            "0u".into(),
        )
    } else {
        ensure!(insn.operands.len() == 1, "load requires one source");
        let op = &insn.operands[0];
        if first == 0xd9 && (0xc0..=0xc7).contains(&second) {
            let index = second & 7;
            let name = format!("st({index})");
            ensure!(
                insn.mnemonic == "fld"
                    && op.kind == "reg"
                    && op.reg.as_deref() == Some(name.as_str())
                    && bytes.len() == start + 2,
                "invalid FLD register"
            );
            (6, format!("{index}u"), "0u".into(), "0u".into())
        } else {
            ensure!(op.kind == "mem" && second < 0xc0, "load requires memory");
            let field = (second >> 3) & 7;
            let (kind, width, name) = match (first, field) {
                (0xd9, 0) => (0, 4, "fld"),
                (0xdd, 0) => (1, 8, "fld"),
                (0xdb, 5) => (2, 10, "fld"),
                (0xdf, 0) => (3, 2, "fild"),
                (0xdb, 0) => (4, 4, "fild"),
                (0xdf, 5) => (5, 8, "fild"),
                (0xdf, 4) => (14, 10, "fbld"),
                _ => bail!("unqualified x87 memory load"),
            };
            ensure!(
                insn.mnemonic == name && op.mem_size == width,
                "wrong load width/opcode"
            );
            addr = "address".into();
            let metadata = x87_data_segment(op);
            read = if kind == 14 {
                ensure!(
                    matches!(op.mem_seg.as_deref(), None | Some("fs")),
                    "unqualified BCD load segment"
                );
                let base = if op.mem_seg.as_deref() == Some("fs") {
                    "g_fs_base"
                } else {
                    "0u"
                };
                format!("extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); uint32_t offset={}; if(offset>0xfffffff6u) gamepkg_x86_general_protection(0x{:08x}u,ebp); uint32_t address=(uint32_t)({base}+offset); uint32_t low=xbox_MmioLoad(address,4); ",address(op,&p,true)?,insn.address)
            } else {
                format!(
                    "uint32_t address={}; uint32_t low=xbox_MmioLoad(address,{}); ",
                    address(op, &p, false)?,
                    if width == 2 { 2 } else { 4 }
                )
            };
            if width >= 8 {
                read.push_str("uint32_t high=xbox_MmioLoad((uint32_t)(address+4u),4); ");
            }
            if width == 10 {
                read.push_str("uint32_t upper=xbox_MmioLoad((uint32_t)(address+8u),2); ");
            }
            read.push_str(&metadata);
            (
                kind,
                "low".into(),
                if width >= 8 {
                    "high".into()
                } else {
                    "0u".into()
                },
                if width == 10 {
                    "upper".into()
                } else {
                    "0u".into()
                },
            )
        }
    };
    Ok(format!("{{ extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern void gamepkg_x86_x87_load(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_wait(0x{:08x}u,ebp); {read}gamepkg_x86_x87_load({kind}u,{low},{high},{upper},0x{:08x}u,0x{opcode:03x}u,{addr},ebp); }}\n",insn.address,insn.address))
}
