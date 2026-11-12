//! Eager flag images under the host's flat protected-mode CPL0 profile.
use super::*;
pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "lahf"
            | "sahf"
            | "clc"
            | "stc"
            | "cmc"
            | "cld"
            | "std"
            | "pushf"
            | "pushfd"
            | "popf"
            | "popfd"
    )
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = prefixes(insn)?;
    ensure!(
        insn.operands.is_empty(),
        "flag instruction takes no explicit operands"
    );
    let raw = hex::decode(&insn.bytes_hex)?;
    let start = raw.iter().take_while(|b| matches!(b, 0x66 | 0x67)).count();
    ensure!(raw.len() == start + 1, "unqualified flag opcode/prefix");
    let name = insn.mnemonic.as_str();
    let expected = match name {
        "lahf" => 0x9f,
        "sahf" => 0x9e,
        "clc" => 0xf8,
        "stc" => 0xf9,
        "cmc" => 0xf5,
        "cld" => 0xfc,
        "std" => 0xfd,
        "pushf" | "pushfd" => 0x9c,
        "popf" | "popfd" => 0x9d,
        _ => bail!("unsupported flag operation"),
    };
    ensure!(raw[start] == expected, "flag mnemonic/opcode mismatch");
    let slot = if p.operand16 { 2 } else { 4 };
    let code = match name {
        "lahf" => "g_eax=(g_eax & ~0xff00u)|(((g_eflags & 0xd5u)|2u)<<8u);".into(),
        "sahf" => "g_eflags=(g_eflags & ~0xffu)|((g_eax>>8u)&0xd5u)|2u;".into(),
        "clc" => "g_eflags=(g_eflags & ~1u)|2u;".into(),
        "stc" => "g_eflags |= 3u;".into(),
        "cmc" => "g_eflags=(g_eflags ^ 1u)|2u;".into(),
        "cld" => "g_df=0;g_eflags=(g_eflags & ~0x400u)|2u;".into(),
        "std" => "g_df=1;g_eflags |= 0x402u;".into(),
        "pushf" | "pushfd" => {
            ensure!(
                name == if p.operand16 { "pushf" } else { "pushfd" },
                "wrong flags stack width"
            );
            format!("extern uint32_t gamepkg_x86_flags_image(void); uint32_t value=gamepkg_x86_flags_image(); uint32_t flag_sp=g_esp-{slot}u; xbox_MmioStore(flag_sp,value & 0x{:08x}u,{slot}); g_esp=flag_sp;",mask(slot)?)
        }
        _ => {
            ensure!(
                name == if p.operand16 { "popf" } else { "popfd" },
                "wrong flags stack width"
            );
            format!("extern void gamepkg_x86_flags_load(uint32_t,uint32_t); uint32_t value=xbox_MmioLoad(g_esp,{slot}); gamepkg_x86_flags_load(value,{slot}u); g_esp+={slot}u;")
        }
    };
    Ok(format!("{{ {code} }}\n"))
}
