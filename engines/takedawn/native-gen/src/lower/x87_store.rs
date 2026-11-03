//! x87 register/raw80, IEEE and signed-integer stores through checked RAM.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(name, "fst" | "fstp" | "fist" | "fistp" | "fbstp")
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(
        !p.operand16 && insn.operands.len() == 1,
        "unqualified x87 store operands"
    );
    let operand = &insn.operands[0];
    ensure!(
        operand.expr.is_none() && operand.expr_src.is_none(),
        "synthetic x87 store"
    );
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(bytes.len() >= start + 2, "truncated x87 store");
    let first = bytes[start];
    let second = bytes[start + 1];
    let opcode = ((u32::from(first) & 7) << 8) | u32::from(second);
    let (kind, destination, pop, name, setup, addr) = if second >= 0xc0 {
        ensure!(
            start == 0 && bytes.len() == 2 && first == 0xdd && (0xd0..=0xdf).contains(&second),
            "unqualified x87 register store"
        );
        let dst = second & 7;
        let reg = format!("st({dst})");
        ensure!(
            operand.kind == "reg" && operand.reg.as_deref() == Some(reg.as_str()),
            "wrong store register metadata"
        );
        let pop = u32::from(second & 8 != 0);
        (
            6,
            u32::from(dst),
            pop,
            if pop == 0 { "fst" } else { "fstp" },
            String::new(),
            "0u",
        )
    } else {
        ensure!(
            operand.kind == "mem" && matches!(operand.mem_seg.as_deref(), None | Some("fs")),
            "unqualified x87 store segment"
        );
        let field = (second >> 3) & 7;
        let (kind, width, pop, name) = match (first, field) {
            (0xd9, 2 | 3) => (
                0,
                4u32,
                u32::from(field == 3),
                if field == 2 { "fst" } else { "fstp" },
            ),
            (0xdd, 2 | 3) => (
                1,
                8,
                u32::from(field == 3),
                if field == 2 { "fst" } else { "fstp" },
            ),
            (0xdb, 7) => (2, 10, 1, "fstp"),
            (0xdf, 2 | 3) => (
                3,
                2,
                u32::from(field == 3),
                if field == 2 { "fist" } else { "fistp" },
            ),
            (0xdb, 2 | 3) => (
                4,
                4,
                u32::from(field == 3),
                if field == 2 { "fist" } else { "fistp" },
            ),
            (0xdf, 7) => (5, 8, 1, "fistp"),
            (0xdf, 6) => (7, 10, 1, "fbstp"),
            _ => bail!("unqualified x87 memory store"),
        };
        ensure!(
            u32::from(operand.mem_size) == width,
            "wrong x87 store width"
        );
        let offset = address(operand, &p, true)?;
        let metadata = x87_data_segment(operand);
        let base = if operand.mem_seg.as_deref() == Some("fs") {
            "g_fs_base"
        } else {
            "0u"
        };
        let setup=format!("extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); uint32_t offset={offset}; if(offset>0x{:08x}u) gamepkg_x86_general_protection(0x{:08x}u,ebp); uint32_t address=(uint32_t)({base}+offset); {metadata}",u32::MAX-(width-1),insn.address);
        (kind, 0, pop, name, setup, "address")
    };
    ensure!(insn.mnemonic == name, "store mnemonic/encoding mismatch");
    Ok(format!("{{ extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern void gamepkg_x86_x87_store(uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_x87_wait(0x{:08x}u,ebp); {setup}gamepkg_x86_x87_store({kind}u,{destination}u,{pop}u,0x{:08x}u,0x{opcode:03x}u,{addr},ebp); }}\n",insn.address,insn.address))
}
