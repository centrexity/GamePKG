//! Typed data/stack instruction lowering into the pinned runtime's C ABI.
//! This is an instruction component, not a complete function generator. EBP
//! remains the caller's local `ebp`, as in the original runtime. Calls, SEH,
//! flags, fault recovery and atomic memory exchanges need their own lowering.
use crate::decode::{Instruction, Operand};
use anyhow::{bail, ensure, Context, Result};
mod alu;
mod approx;
mod arithmetic;
mod atomic;
mod conversion;
pub mod dispatch;
mod division;
mod flags;
mod floating;
pub mod flow;
mod mmx;
mod multiply;
mod predicate;
mod selection;
mod shift;
mod streaming;
mod string;
mod vector;
mod x87;
mod x87_compare;
mod x87_environment;
mod x87_load;
mod x87_stack;
mod x87_store;
mod x87_trans;

/// Supported scalar instructions plus exact-bit SSE1 data operations.
pub fn instruction(insn: &Instruction) -> Result<String> {
    if insn.mnemonic == "int3" {
        ensure!(
            insn.size == 1
                && hex::decode(&insn.bytes_hex)? == [0xcc]
                && insn.operands.is_empty()
                && insn.call_target.is_none()
                && insn.jump_target.is_none(),
            "unqualified INT3 encoding or metadata"
        );
        let next = insn
            .address
            .checked_add(1)
            .context("INT3 IP wrap not qualified")?;
        Ok(format!("{{extern _Noreturn void gamepkg_x86_breakpoint_trap(uint32_t,uint32_t);gamepkg_x86_breakpoint_trap(0x{next:08x}u,ebp);}}\n"))
    } else if approx::supports(&insn.mnemonic) {
        approx::instruction(insn)
    } else if string::supports(&insn.mnemonic) {
        string::instruction(insn)
    } else if predicate::supports(&insn.mnemonic) {
        predicate::instruction(insn)
    } else if selection::supports(&insn.mnemonic) {
        selection::instruction(insn)
    } else if streaming::supports(&insn.mnemonic) {
        streaming::instruction(insn)
    } else if x87_environment::supports(&insn.mnemonic) {
        x87_environment::instruction(insn)
    } else if x87_trans::supports(&insn.mnemonic) {
        x87_trans::instruction(insn)
    } else if x87_stack::supports(&insn.mnemonic) {
        x87_stack::instruction(insn)
    } else if x87_store::supports(&insn.mnemonic) {
        x87_store::instruction(insn)
    } else if x87_load::supports(&insn.mnemonic) {
        x87_load::instruction(insn)
    } else if x87_compare::supports(&insn.mnemonic) {
        x87_compare::instruction(insn)
    } else if x87::supports(&insn.mnemonic) {
        x87::instruction(insn)
    } else if mmx::supports(&insn.mnemonic) {
        mmx::instruction(insn)
    } else if matches!(
        insn.mnemonic.as_str(),
        "cvtsi2ss" | "cvtss2si" | "cvttss2si"
    ) {
        conversion::instruction(insn)
    } else if arithmetic::supports(&insn.mnemonic) {
        arithmetic::instruction(insn)
    } else if matches!(
        insn.mnemonic.as_str(),
        "comiss" | "ucomiss" | "ldmxcsr" | "stmxcsr"
    ) {
        floating::instruction(insn)
    } else if vector::supports(&insn.mnemonic) {
        vector::instruction(insn)
    } else {
        scalar_instruction(insn)
    }
}

/// Integer emission uses eager arithmetic bits in `g_eflags`. Existing lazy
/// flag functions must be bridged before mixing them at call/SEH boundaries.
/// DIV/IDIV also require runtime/gamepkg_x86_fault.c and a live gamepkg_x86_run
/// entry boundary. Its fault result must be dispatched, not treated as return.
pub fn integer_instruction(insn: &Instruction) -> Result<String> {
    if flags::supports(&insn.mnemonic) {
        return flags::instruction(insn);
    }
    match insn.mnemonic.as_str() {
        "add" | "adc" | "sub" | "sbb" | "cmp" | "test" | "and" | "or" | "xor" | "inc" | "dec"
        | "neg" | "not" => alu::instruction(insn),
        "shl" | "sal" | "shr" | "sar" | "rol" | "ror" | "rcl" | "rcr" => shift::instruction(insn),
        "shld" | "shrd" => shift::double_instruction(insn),
        "mul" | "imul" => multiply::instruction(insn),
        "div" | "idiv" => division::instruction(insn),
        _ => data_instruction(insn),
    }
}

/// Data, arithmetic and flag-dependent register/memory conditions. Branch
/// destinations belong to the checked function emitter, not this API.
pub fn scalar_instruction(insn: &Instruction) -> Result<String> {
    if insn.mnemonic.starts_with("set") || insn.mnemonic.starts_with("cmov") {
        flow::conditional_data(insn)
    } else {
        integer_instruction(insn)
    }
}

#[derive(Clone, Copy)]
struct Register {
    name: &'static str,
    width: u8,
    shift: u8,
}
fn register(name: &str) -> Result<Register> {
    let (name, width, shift) = match name {
        "eax" => ("g_eax", 4, 0),
        "ax" => ("g_eax", 2, 0),
        "al" => ("g_eax", 1, 0),
        "ah" => ("g_eax", 1, 8),
        "ebx" => ("g_ebx", 4, 0),
        "bx" => ("g_ebx", 2, 0),
        "bl" => ("g_ebx", 1, 0),
        "bh" => ("g_ebx", 1, 8),
        "ecx" => ("g_ecx", 4, 0),
        "cx" => ("g_ecx", 2, 0),
        "cl" => ("g_ecx", 1, 0),
        "ch" => ("g_ecx", 1, 8),
        "edx" => ("g_edx", 4, 0),
        "dx" => ("g_edx", 2, 0),
        "dl" => ("g_edx", 1, 0),
        "dh" => ("g_edx", 1, 8),
        "esi" => ("g_esi", 4, 0),
        "si" => ("g_esi", 2, 0),
        "edi" => ("g_edi", 4, 0),
        "di" => ("g_edi", 2, 0),
        "esp" => ("g_esp", 4, 0),
        "sp" => ("g_esp", 2, 0),
        "ebp" => ("ebp", 4, 0),
        "bp" => ("ebp", 2, 0),
        _ => bail!("unsupported data register"),
    };
    Ok(Register { name, width, shift })
}
fn mask(width: u8) -> Result<u32> {
    match width {
        1 => Ok(0xff),
        2 => Ok(0xffff),
        4 => Ok(u32::MAX),
        _ => bail!("unsupported data width"),
    }
}
fn operand_register(op: &Operand) -> Result<Register> {
    ensure!(op.kind == "reg", "expected data register");
    register(op.reg.as_deref().context("missing data register")?)
}
fn width(op: &Operand) -> Result<u8> {
    match op.kind {
        "reg" => Ok(operand_register(op)?.width),
        "mem" => {
            mask(op.mem_size)?;
            Ok(op.mem_size)
        }
        _ => bail!("operand has no data width"),
    }
}
fn read_register(reg: Register) -> String {
    format!(
        "(({} >> {}u) & 0x{:08x}u)",
        reg.name,
        reg.shift,
        mask(reg.width).unwrap()
    )
}
fn write_register(reg: Register, value: &str) -> String {
    let field = mask(reg.width).unwrap();
    if reg.width == 4 {
        return format!("{} = (uint32_t)({value});", reg.name);
    }
    format!(
        "{} = ({} & 0x{:08x}u) | (((uint32_t)({value}) & 0x{field:08x}u) << {}u);",
        reg.name,
        reg.name,
        !(field << reg.shift),
        reg.shift
    )
}
#[derive(Default)]
struct Prefixes {
    operand16: bool,
    address16: bool,
    mandatory: Option<u8>,
}
fn prefixes(insn: &Instruction) -> Result<Prefixes> {
    parse_prefixes(insn, false)
}
fn parse_prefixes(insn: &Instruction, vector: bool) -> Result<Prefixes> {
    ensure!(
        (1..=15).contains(&insn.size) && insn.bytes_hex.len() == usize::from(insn.size) * 2,
        "invalid instruction byte metadata"
    );
    let bytes = hex::decode(&insn.bytes_hex).context("invalid instruction hex")?;
    let mut out = Prefixes::default();
    for byte in bytes {
        match byte {
            0x66 => out.operand16 = true,
            0x67 => out.address16 = true,
            0x26 | 0x2e | 0x36 | 0x3e | 0x64 | 0x65 => {}
            0xf2 | 0xf3 if vector => {
                ensure!(
                    out.mandatory.replace(byte).is_none(),
                    "conflicting mandatory vector prefixes"
                );
            }
            0xf0 | 0xf2 | 0xf3 => bail!("atomic/repeat prefix requires separate lowering"),
            _ => return Ok(out),
        }
    }
    bail!("instruction consists only of prefixes")
}
fn address(op: &Operand, prefix: &Prefixes, lea: bool) -> Result<String> {
    address_with_stack(op, prefix, lea, "g_esp")
}
// POP memory computes ESP-based destinations using the advanced value while
// retaining the old architectural ESP until the complete store succeeds.
fn address_with_stack(
    op: &Operand,
    prefix: &Prefixes,
    lea: bool,
    stack_pointer: &'static str,
) -> Result<String> {
    ensure!(op.kind == "mem", "expected memory address");
    ensure!(
        (i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&op.mem_disp),
        "invalid address displacement"
    );
    ensure!(
        matches!(op.mem_scale, 1 | 2 | 4 | 8),
        "invalid address scale"
    );
    let mut value = format!("0x{:08x}u", op.mem_disp as u32);
    for (name, scale) in [(&op.mem_base, 1), (&op.mem_index, op.mem_scale)] {
        if let Some(name) = name {
            let mut reg = register(name)?;
            ensure!(
                reg.width == if prefix.address16 { 2 } else { 4 },
                "address register width mismatch"
            );
            if reg.name == "g_esp" {
                reg.name = stack_pointer;
            }
            value = format!("({value} + {} * {scale}u)", read_register(reg));
        }
    }
    value = if prefix.address16 {
        format!("({value} & 0x0000ffffu)")
    } else {
        format!("(uint32_t)({value})")
    };
    if !lea {
        match op.mem_seg.as_deref() {
            None => {}
            Some("fs") => value = format!("(uint32_t)(g_fs_base + {value})"),
            _ => bail!("segment addressing requires separate lowering"),
        }
    }
    Ok(value)
}
fn x87_data_segment(op: &Operand) -> String {
    let (kind, base) = if op.mem_seg.as_deref() == Some("fs") {
        (2, "g_fs_base")
    } else if matches!(op.mem_base.as_deref(), Some("ebp" | "esp" | "bp" | "sp")) {
        (1, "0u")
    } else {
        (0, "0u")
    };
    format!("extern void gamepkg_x86_x87_data_segment(uint32_t,uint32_t); gamepkg_x86_x87_data_segment({kind}u,{base}); ")
}
fn read(op: &Operand, prefix: &Prefixes) -> Result<String> {
    Ok(match op.kind {
        "reg" => read_register(operand_register(op)?),
        "imm" => format!("0x{:08x}u", op.imm.context("missing immediate")?),
        "mem" => {
            mask(op.mem_size)?;
            format!(
                "xbox_MmioLoad({}, {})",
                address(op, prefix, false)?,
                op.mem_size
            )
        }
        _ => bail!("unsupported data operand"),
    })
}
fn write(op: &Operand, value: &str, prefix: &Prefixes) -> Result<String> {
    match op.kind {
        "reg" => Ok(write_register(operand_register(op)?, value)),
        "mem" => {
            let mask = mask(op.mem_size)?;
            Ok(format!(
                "xbox_MmioStore({}, ((uint32_t)({value}) & 0x{mask:08x}u), {});",
                address(op, prefix, false)?,
                op.mem_size
            ))
        }
        _ => bail!("unsupported data destination"),
    }
}

/// Emit one bounded C scope. All identifiers and expressions are constructed
/// from typed whitelists; neither disassembly text nor synthetic expressions
/// enter C. An unsupported instruction/form returns an error, never a stub.
/// The caller must supply an instruction from Decoder, not unverified metadata.
pub fn data_instruction(insn: &Instruction) -> Result<String> {
    if atomic::supports(&insn.mnemonic) {
        return atomic::instruction(insn);
    }
    ensure!(insn.operands.len() <= 2, "too many data operands");
    for op in &insn.operands {
        ensure!(
            op.expr.is_none() && op.expr_src.is_none(),
            "synthetic expressions require typed lowering"
        );
    }
    let p = prefixes(insn)?;
    let slot = if p.operand16 { 2 } else { 4 };
    let ops = insn.operands.as_slice();
    let body = match (insn.mnemonic.as_str(), ops) {
        ("mov", [dst, src]) => {
            let w = width(dst)?;
            ensure!(src.kind == "imm" || width(src)? == w, "MOV width mismatch");
            ensure!(
                dst.kind != "mem" || src.kind != "mem",
                "memory-to-memory MOV"
            );
            write(dst, &read(src, &p)?, &p)?
        }
        ("movzx" | "movsx", [dst, src]) => {
            let d = operand_register(dst)?;
            let s = width(src)?;
            ensure!(d.width > s && s <= 2, "invalid extension widths");
            let value = if insn.mnemonic == "movsx" {
                let sign = 1u32 << (s * 8 - 1);
                format!(
                    "(((v & 0x{:08x}u) ^ 0x{sign:08x}u) - 0x{sign:08x}u)",
                    mask(s)?
                )
            } else {
                format!("(v & 0x{:08x}u)", mask(s)?)
            };
            format!(
                "uint32_t v = {}; {}",
                read(src, &p)?,
                write_register(d, &value)
            )
        }
        ("lea", [dst, src]) => {
            let d = operand_register(dst)?;
            ensure!(d.width >= 2, "invalid LEA destination");
            write_register(d, &address(src, &p, true)?)
        }
        ("xchg", [a, b]) => {
            let a = operand_register(a)?;
            let b = operand_register(b)?;
            ensure!(a.width == b.width, "XCHG width mismatch");
            format!(
                "uint32_t a = {}; uint32_t b = {}; {} {}",
                read_register(a),
                read_register(b),
                write_register(a, "b"),
                write_register(b, "a")
            )
        }
        ("push", [src]) => {
            ensure!(
                src.kind == "imm" || width(src)? == slot,
                "PUSH width mismatch"
            );
            format!(
                "uint32_t v = {}; uint32_t push_sp = g_esp - {slot}u; xbox_MmioStore(push_sp, v & 0x{:08x}u, {slot}); g_esp = push_sp;",
                read(src, &p)?,
                mask(slot)?
            )
        }
        ("pop", [dst]) => {
            ensure!(width(dst)? == slot, "POP width mismatch");
            let commit = if dst.kind == "mem" {
                format!("uint32_t pop_address = {}; xbox_MmioStore(pop_address, v & 0x{:08x}u, {slot}); g_esp = pop_sp;",
                    address_with_stack(dst, &p, false, "pop_sp")?, mask(slot)?)
            } else {
                // POP ESP/SP replaces the advanced pointer with the loaded
                // register value. A 16-bit SP write keeps its advanced upper16.
                format!("g_esp = pop_sp; {}", write(dst, "v", &p)?)
            };
            format!("uint32_t v = xbox_MmioLoad(g_esp, {slot}); uint32_t pop_sp = g_esp + {slot}u; {commit}")
        }
        ("leave", []) => {
            let bp = Register {
                name: "ebp",
                width: slot,
                shift: 0,
            };
            format!(
                "uint32_t leave_sp = ebp; uint32_t v = xbox_MmioLoad(leave_sp, {slot}); {} g_esp = leave_sp + {slot}u;",
                write_register(bp, "v")
            )
        }
        ("bswap", [dst]) => {
            let d = operand_register(dst)?;
            ensure!(d.width == 4, "BSWAP requires 32 bits");
            format!("uint32_t v = {}; {}", read_register(d), write_register(d, "((v & 0xffu) << 24u) | ((v & 0xff00u) << 8u) | ((v >> 8u) & 0xff00u) | (v >> 24u)"))
        }
        ("cdq", []) => "g_edx = 0u - (g_eax >> 31u);".into(),
        ("cwd", []) => write_register(register("dx")?, "0u - ((g_eax >> 15u) & 1u)"),
        ("cwde", []) => "g_eax = ((g_eax & 0xffffu) ^ 0x8000u) - 0x8000u;".into(),
        ("cbw", []) => write_register(register("ax")?, "((g_eax & 0xffu) ^ 0x80u) - 0x80u"),
        ("nop", []) => String::new(),
        _ => bail!("unsupported native data instruction at {:#x}", insn.address),
    };
    Ok(format!("{{ {body} }}\n"))
}
