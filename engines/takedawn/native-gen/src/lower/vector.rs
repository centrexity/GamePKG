//! SSE1 data/lane operations on the pinned runtime's uint32 XMM lanes. No
//! floating conversion/arithmetic or relaxed alignment/fault substitutes.
use super::*;

pub(super) fn supports(name: &str) -> bool {
    matches!(
        name,
        "movss"
            | "movaps"
            | "movups"
            | "movlps"
            | "movhps"
            | "movlhps"
            | "movhlps"
            | "andps"
            | "orps"
            | "xorps"
            | "andnps"
            | "shufps"
            | "unpcklps"
            | "unpckhps"
            | "movmskps"
    )
}
pub(super) fn xmm(op: &Operand) -> Result<&'static str> {
    ensure!(op.kind == "reg", "expected XMM register");
    Ok(match op.reg.as_deref() {
        Some("xmm0") => "g_xmm0",
        Some("xmm1") => "g_xmm1",
        Some("xmm2") => "g_xmm2",
        Some("xmm3") => "g_xmm3",
        Some("xmm4") => "g_xmm4",
        Some("xmm5") => "g_xmm5",
        Some("xmm6") => "g_xmm6",
        Some("xmm7") => "g_xmm7",
        _ => bail!("unsupported SSE1 register"),
    })
}
fn memory(op: &Operand, p: &Prefixes, bytes: u8, aligned: bool, ip: u32) -> Result<String> {
    ensure!(
        op.kind == "mem" && op.mem_size == bytes,
        "invalid vector memory width"
    );
    let address = address(op, p, false)?;
    let mut code = format!("uint32_t address = {address}; ");
    // Check the final guest linear address (after address-size wrapping and FS
    // addition) before any memory callback. Only constant aligned addresses
    // can omit this guard. Guest alignment never depends on host pointers.
    let proven = op.mem_base.is_none()
        && op.mem_index.is_none()
        && op.mem_seg.is_none()
        && (op.mem_disp & 15) == 0;
    if aligned && !proven {
        code.push_str(&format!("extern _Noreturn void gamepkg_x86_general_protection(uint32_t,uint32_t); if (address & 15u) gamepkg_x86_general_protection(0x{ip:08x}u,ebp); "));
    }
    Ok(code)
}
pub(super) fn read_vector(
    op: &Operand,
    p: &Prefixes,
    lanes: u8,
    aligned: bool,
    ip: u32,
) -> Result<String> {
    if op.kind == "reg" {
        return Ok(format!("RecompXmm b = {}; ", xmm(op)?));
    }
    let mut code = memory(op, p, lanes * 4, aligned, ip)?;
    code.push_str("RecompXmm b; ");
    for i in 0..4 {
        code.push_str(&if i < lanes {
            format!(
                "b.u[{i}] = xbox_MmioLoad((uint32_t)(address + {}u), 4); ",
                i * 4
            )
        } else {
            format!("b.u[{i}] = 0u; ")
        });
    }
    Ok(code)
}
fn store_vector(
    op: &Operand,
    p: &Prefixes,
    start: u8,
    lanes: u8,
    aligned: bool,
    ip: u32,
) -> Result<String> {
    let mut code = memory(op, p, lanes * 4, aligned, ip)?;
    for i in 0..lanes {
        code.push_str(&format!(
            "xbox_MmioStore((uint32_t)(address + {}u), b.u[{}], 4); ",
            i * 4,
            i + start
        ));
    }
    Ok(code)
}
pub(super) fn instruction(insn: &Instruction) -> Result<String> {
    let p = parse_prefixes(insn, true)?;
    ensure!(
        !p.operand16
            && p.mandatory
                == if insn.mnemonic == "movss" {
                    Some(0xf3)
                } else {
                    None
                },
        "unsupported SSE1 mandatory prefix"
    );
    let name = insn.mnemonic.as_str();
    ensure!(
        insn.operands.len() == if name == "shufps" { 3 } else { 2 },
        "invalid vector operand count"
    );
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic vector expression"
        );
    }
    let dst = &insn.operands[0];
    let src = &insn.operands[1];
    let body = match name {
        "movss" => {
            if dst.kind == "mem" {
                let source = xmm(src)?;
                format!(
                    "RecompXmm b = {source}; {}",
                    store_vector(dst, &p, 0, 1, false, insn.address)?
                )
            } else {
                let target = xmm(dst)?;
                let code = read_vector(src, &p, 1, false, insn.address)?;
                if src.kind == "mem" {
                    format!("{code}{target} = b;")
                } else {
                    format!("{code}{target}.u[0] = b.u[0];")
                }
            }
        }
        "movaps" | "movups" => {
            let aligned = name == "movaps";
            if dst.kind == "mem" {
                format!(
                    "RecompXmm b = {}; {}",
                    xmm(src)?,
                    store_vector(dst, &p, 0, 4, aligned, insn.address)?
                )
            } else {
                format!(
                    "{}{} = b;",
                    read_vector(src, &p, 4, aligned, insn.address)?,
                    xmm(dst)?
                )
            }
        }
        "movlps" | "movhps" => {
            let start = if name == "movhps" { 2 } else { 0 };
            if dst.kind == "mem" {
                format!(
                    "RecompXmm b = {}; {}",
                    xmm(src)?,
                    store_vector(dst, &p, start, 2, false, insn.address)?
                )
            } else {
                ensure!(src.kind == "mem", "half-vector move requires memory");
                let target = xmm(dst)?;
                format!("{}RecompXmm a = {target}; a.u[{start}] = b.u[0]; a.u[{}] = b.u[1]; {target} = a;", read_vector(src,&p,2,false, insn.address)?, start+1)
            }
        }
        "movlhps" | "movhlps" => {
            let target = xmm(dst)?;
            let source = xmm(src)?;
            let (a, b) = if name == "movlhps" { (2, 0) } else { (0, 2) };
            format!("RecompXmm a = {target}; RecompXmm b = {source}; a.u[{a}] = b.u[{b}]; a.u[{}] = b.u[{}]; {target} = a;", a+1,b+1)
        }
        "movmskps" => {
            let target = operand_register(dst)?;
            ensure!(target.width == 4, "MOVMSKPS requires a 32bit destination");
            format!("RecompXmm b = {}; {}", xmm(src)?, write_register(target,"(b.u[0] >> 31u) | ((b.u[1] >> 31u) << 1u) | ((b.u[2] >> 31u) << 2u) | ((b.u[3] >> 31u) << 3u)"))
        }
        "andps" | "orps" | "xorps" | "andnps" => {
            let target = xmm(dst)?;
            let mut code = format!(
                "RecompXmm a = {target}; {}",
                read_vector(src, &p, 4, true, insn.address)?
            );
            let op = match name {
                "andps" => "&",
                "orps" => "|",
                "xorps" => "^",
                _ => "&",
            };
            for i in 0..4 {
                code.push_str(&format!(
                    "a.u[{i}] = {}a.u[{i}] {op} b.u[{i}]; ",
                    if name == "andnps" { "~" } else { "" }
                ));
            }
            format!("{code}{target} = a;")
        }
        "shufps" | "unpcklps" | "unpckhps" => {
            let target = xmm(dst)?;
            let lanes = if name == "shufps" {
                let op = &insn.operands[2];
                ensure!(op.kind == "imm", "SHUFPS requires immediate lane control");
                let imm = op
                    .imm
                    .filter(|i| *i <= 255)
                    .context("invalid shuffle immediate")?;
                [
                    (false, (imm & 3) as u8),
                    (false, ((imm >> 2) & 3) as u8),
                    (true, ((imm >> 4) & 3) as u8),
                    (true, ((imm >> 6) & 3) as u8),
                ]
            } else {
                let start = if name == "unpckhps" { 2 } else { 0 };
                [
                    (false, start),
                    (true, start),
                    (false, start + 1),
                    (true, start + 1),
                ]
            };
            let mut code = format!(
                "RecompXmm a = {target}; {}RecompXmm result; ",
                read_vector(src, &p, 4, true, insn.address)?
            );
            for (i, (source, lane)) in lanes.into_iter().enumerate() {
                code.push_str(&format!(
                    "result.u[{i}] = {}.u[{lane}]; ",
                    if source { "b" } else { "a" }
                ));
            }
            format!("{code}{target} = result;")
        }
        _ => bail!("unsupported SSE1 operation"),
    };
    Ok(format!("{{ {body} }}\n"))
}
