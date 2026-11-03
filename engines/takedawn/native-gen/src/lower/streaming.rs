//! Checked streaming stores, store ordering and non-faulting cache hints.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "movntq"
            | "movntps"
            | "maskmovq"
            | "sfence"
            | "prefetchnta"
            | "prefetcht0"
            | "prefetcht1"
            | "prefetcht2"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, false)?;
    ensure!(!p.operand16, "unqualified streaming operand prefix");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic streaming expression"
        );
    }
    let bytes = hex::decode(&insn.bytes_hex)?;
    let start = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    ensure!(
        bytes.len() >= start + 3 && bytes[start] == 0x0f,
        "invalid streaming encoding"
    );
    let field = u32::from((bytes[start + 2] >> 3) & 7);
    let site = format!("0x{:08x}u,ebp", insn.address);
    let name = insn.mnemonic.as_str();
    let code = match name {
        "sfence" => {
            ensure!(
                insn.operands.is_empty()
                    && bytes.len() == start + 3
                    && bytes[start + 1] == 0xae
                    && bytes[start + 2] >= 0xf8,
                "invalid SFENCE"
            );
            "extern void gamepkg_x86_store_fence(void); gamepkg_x86_store_fence();".into()
        }
        "prefetchnta" | "prefetcht0" | "prefetcht1" | "prefetcht2" => {
            let hint = match name {
                "prefetchnta" => 0,
                "prefetcht0" => 1,
                "prefetcht1" => 2,
                _ => 3,
            };
            ensure!(
                insn.operands.len() == 1
                    && bytes[start + 1] == 0x18
                    && bytes[start + 2] < 0xc0
                    && field == hint,
                "invalid prefetch"
            );
            let src = &insn.operands[0];
            ensure!(
                src.kind == "mem" && src.mem_size == 1,
                "prefetch requires m8"
            );
            format!("extern void gamepkg_x86_prefetch(uint32_t,uint32_t); uint32_t address={}; gamepkg_x86_prefetch(address,{hint}u);",address(src,&p,false)?)
        }
        "movntq" | "movntps" => {
            let size = if name == "movntq" { 8 } else { 16 };
            let opcode = if size == 8 { 0xe7 } else { 0x2b };
            ensure!(
                insn.operands.len() == 2 && bytes[start + 1] == opcode && bytes[start + 2] < 0xc0,
                "invalid streaming store"
            );
            let dst = &insn.operands[0];
            let src = &insn.operands[1];
            ensure!(
                dst.kind == "mem"
                    && dst.mem_size == size
                    && matches!(dst.mem_seg.as_deref(), None | Some("fs")),
                "wrong streaming destination"
            );
            let (snapshot, commit) = if size == 8 {
                let reg = mmx::mm(src)?;
                ensure!(field == reg, "wrong MOVNTQ source");
                (format!("extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern uint64_t gamepkg_x86_mmx_read(uint32_t); gamepkg_x86_x87_wait({site}); uint64_t value=gamepkg_x86_mmx_read({reg}u); uint32_t words[4]={{(uint32_t)value,(uint32_t)(value>>32u),0,0}}; "),"extern void gamepkg_x86_mmx_read_commit(void); gamepkg_x86_mmx_read_commit();".into())
            } else {
                let xmm = vector::xmm(src)?;
                ensure!(
                    field == u32::from(xmm.as_bytes()[5] - b'0'),
                    "wrong MOVNTPS source"
                );
                (
                    format!("uint32_t words[4]={{{xmm}.u[0],{xmm}.u[1],{xmm}.u[2],{xmm}.u[3]}}; "),
                    String::new(),
                )
            };
            let offset = address(dst, &p, true)?;
            let base = if dst.mem_seg.as_deref() == Some("fs") {
                "g_fs_base"
            } else {
                "0u"
            };
            let alignment = if size == 16 {
                format!("if(address&15u) gamepkg_x86_general_protection({site}); ")
            } else {
                String::new()
            };
            let mask = if size == 8 { "0xffu" } else { "0xffffu" };
            format!("{snapshot}extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); uint32_t offset={offset}; if(offset>0x{:08x}u) gamepkg_x86_general_protection({site}); uint32_t address=(uint32_t)({base}+offset); {alignment}extern void gamepkg_x86_memory_write(uint32_t,const uint32_t[4],uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_memory_write(address,words,{size}u,{mask},1u,{site}); {commit}",u32::MAX-(u32::from(size)-1))
        }
        "maskmovq" => {
            ensure!(
                insn.operands.len() == 2
                    && bytes.len() == start + 3
                    && bytes[start + 1] == 0xf7
                    && bytes[start + 2] >= 0xc0,
                "invalid MASKMOVQ"
            );
            let src = mmx::mm(&insn.operands[0])?;
            let mask = mmx::mm(&insn.operands[1])?;
            ensure!(
                field == src && u32::from(bytes[start + 2] & 7) == mask,
                "wrong MASKMOVQ register metadata"
            );
            let offset = if p.address16 {
                "(g_edi&0xffffu)"
            } else {
                "g_edi"
            };
            let base = if bytes[..start].contains(&0x64) {
                "g_fs_base"
            } else {
                "0u"
            };
            format!("extern void gamepkg_x86_x87_wait(uint32_t,uint32_t); extern uint64_t gamepkg_x86_mmx_read(uint32_t); extern void gamepkg_x86_mmx_read_commit(void); extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); gamepkg_x86_x87_wait({site}); uint64_t value=gamepkg_x86_mmx_read({src}u),selected=gamepkg_x86_mmx_read({mask}u); uint32_t words[4]={{(uint32_t)value,(uint32_t)(value>>32u),0,0}},write_mask=0; for(unsigned lane=0;lane<8;++lane) write_mask|=(uint32_t)((selected>>(8*lane+7))&1u)<<lane; gamepkg_x86_mmx_read_commit(); uint32_t offset={offset}; if(offset>0xfffffff8u) gamepkg_x86_general_protection({site}); uint32_t address=(uint32_t)({base}+offset); extern void gamepkg_x86_memory_write(uint32_t,const uint32_t[4],uint32_t,uint32_t,uint32_t,uint32_t,uint32_t); gamepkg_x86_memory_write(address,words,8u,write_mask,1u,{site});")
        }
        _ => bail!("unsupported streaming instruction"),
    };
    Ok(format!("{{ {code} }}\n"))
}
