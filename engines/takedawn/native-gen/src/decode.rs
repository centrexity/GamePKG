//! Native equivalent of the pinned tools.recomp.disasm model and CFG decoder.
//! The upstream notice is in engines/takedawn/Xboxrecomp-LICENSE.txt.
use anyhow::{ensure, Context, Result};
use capstone::{arch::x86::X86OperandType, prelude::*, RegId};
use gamepkg_takedawn_media::{Budget, Lease};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FUNCTION: usize = 16384;
const MAX_WORK: usize = 65536;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Operand {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub reg: Option<String>,
    pub imm: Option<u32>,
    pub mem_base: Option<String>,
    pub mem_index: Option<String>,
    pub mem_scale: i32,
    pub mem_disp: i64,
    pub mem_size: u8,
    pub mem_seg: Option<String>,
    pub expr: Option<String>,
    pub expr_src: Option<String>,
}
impl Default for Operand {
    fn default() -> Self {
        Self {
            kind: "reg",
            reg: None,
            imm: None,
            mem_base: None,
            mem_index: None,
            mem_scale: 1,
            mem_disp: 0,
            mem_size: 0,
            mem_seg: None,
            expr: None,
            expr_src: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Instruction {
    pub address: u32,
    pub size: u8,
    pub mnemonic: String,
    pub op_str: String,
    pub bytes_hex: String,
    pub operands: Vec<Operand>,
    pub call_target: Option<u32>,
    pub jump_target: Option<u32>,
    pub memory_refs: Vec<u32>,
    pub imm_values: Vec<u32>,
    pub regs_written: Vec<String>,
}
impl Instruction {
    pub fn end(&self) -> u32 {
        self.address + u32::from(self.size)
    }
    pub fn is_call(&self) -> bool {
        self.mnemonic == "call"
    }
    pub fn is_ret(&self) -> bool {
        matches!(self.mnemonic.as_str(), "ret" | "retn" | "retf")
    }
    pub fn is_jump(&self) -> bool {
        self.mnemonic == "jmp"
    }
    pub fn is_conditional(&self) -> bool {
        matches!(
            self.mnemonic.as_str(),
            "jo" | "jno"
                | "jb"
                | "jnb"
                | "jae"
                | "je"
                | "jz"
                | "jne"
                | "jnz"
                | "jbe"
                | "ja"
                | "jnae"
                | "jna"
                | "js"
                | "jns"
                | "jp"
                | "jnp"
                | "jl"
                | "jge"
                | "jle"
                | "jg"
                | "jcxz"
                | "jecxz"
                | "loop"
                | "loope"
                | "loopne"
        )
    }
}
pub struct Decoded {
    instructions: Vec<Instruction>,
    _memory: Lease,
}
impl Decoded {
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }
}
#[derive(Debug, serde::Serialize)]
pub struct Block<'a> {
    pub start: u32,
    pub instructions: Vec<&'a Instruction>,
    pub successors: Vec<u32>,
}
pub struct Decoder {
    cs: Capstone,
    budget: Budget,
    _context: Lease,
}
impl Decoder {
    pub fn new(budget: &Budget) -> Result<Self> {
        let context = budget.reserve(4 * 1024 * 1024)?;
        let cs = Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode32)
            .syntax(arch::x86::ArchSyntax::Intel)
            .detail(true)
            .build()?;
        Ok(Self {
            cs,
            budget: budget.clone(),
            _context: context,
        })
    }
    fn reserve(&self, bytes: &[u8], start: u32) -> Result<(u32, Lease)> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_FUNCTION,
            "native function byte limit exceeded"
        );
        let end = start
            .checked_add(bytes.len() as u32)
            .context("native function address overflow")?;
        // Includes Capstone's bounded C batch/detail allocations, owned strings,
        // instruction maps, final vector, block indices and serialization work.
        let memory = self.budget.reserve(bytes.len() as u64 * 4096 + 65536)?;
        Ok((end, memory))
    }
    fn register(&self, reg: RegId) -> Option<String> {
        (reg.0 != 0).then(|| {
            self.cs
                .reg_name(reg)
                .unwrap_or_else(|| format!("r{}", reg.0))
        })
    }
    fn instruction(&self, insn: &capstone::Insn<'_>) -> Result<Instruction> {
        let address = u32::try_from(insn.address())?;
        let size = u8::try_from(insn.bytes().len())?;
        ensure!(
            (1..=15).contains(&size) && address.checked_add(u32::from(size)).is_some(),
            "invalid x86-32 instruction extent"
        );
        let detail = self.cs.insn_detail(insn)?;
        let arch = detail.arch_detail();
        let x86 = arch
            .x86()
            .context("decoder returned a non-x86 instruction")?;
        let mut decoded = Instruction {
            address,
            size,
            mnemonic: insn.mnemonic().context("missing mnemonic")?.to_owned(),
            op_str: insn.op_str().context("missing operand text")?.to_owned(),
            bytes_hex: hex::encode(insn.bytes()),
            operands: Vec::new(),
            call_target: None,
            jump_target: None,
            memory_refs: Vec::new(),
            imm_values: Vec::new(),
            regs_written: detail
                .regs_write()
                .iter()
                .filter_map(|&r| self.register(r))
                .collect(),
        };
        for operand in x86.operands() {
            let value = match operand.op_type {
                X86OperandType::Imm(imm) => {
                    let imm = imm as u32;
                    decoded.imm_values.push(imm);
                    if decoded.is_call() {
                        decoded.call_target = Some(imm);
                    } else if decoded.is_jump() || decoded.is_conditional() {
                        decoded.jump_target = Some(imm);
                    }
                    Operand {
                        kind: "imm",
                        imm: Some(imm),
                        ..Operand::default()
                    }
                }
                X86OperandType::Reg(reg) => Operand {
                    reg: self.register(reg),
                    ..Operand::default()
                },
                X86OperandType::Mem(mem) => {
                    if mem.base().0 == 0 && mem.index().0 == 0 && mem.disp() != 0 {
                        decoded.memory_refs.push(mem.disp() as u32);
                    }
                    Operand {
                        kind: "mem",
                        mem_base: self.register(mem.base()),
                        mem_index: self.register(mem.index()),
                        mem_scale: mem.scale(),
                        mem_disp: if mem.disp() >= 0 {
                            i64::from(mem.disp() as u32)
                        } else {
                            mem.disp()
                        },
                        mem_size: operand.size,
                        mem_seg: self.register(mem.segment()),
                        ..Operand::default()
                    }
                }
                X86OperandType::Invalid => anyhow::bail!("invalid operand at {address:#x}"),
            };
            decoded.operands.push(value);
        }
        Ok(decoded)
    }
    // Capstone5 rejects valid F3,67 ordering for scalar reciprocal opcodes.
    // Canonicalize only these qualified SSE1 forms for the decoder, retaining
    // the actual bytes and address in the typed instruction consumed downstream.
    fn approximation_prefix_order(
        &self,
        bytes: &[u8],
        address: u32,
    ) -> Result<Option<Instruction>> {
        let mut seen = [false; 3];
        let mut n = 0;
        while let Some(&b) = bytes.get(n) {
            let group = match b {
                0x64 => 0,
                0x67 => 1,
                0xf3 => 2,
                _ => break,
            };
            if seen[group] {
                return Ok(None);
            }
            seen[group] = true;
            n += 1;
        }
        if !seen[2]
            || n < 2
            || bytes.get(n) != Some(&0x0f)
            || !matches!(bytes.get(n + 1), Some(0x52 | 0x53))
        {
            return Ok(None);
        }
        let mut normalized = [0u8; 15];
        let size = bytes.len().min(15);
        normalized[..size].copy_from_slice(&bytes[..size]);
        let mut at = 0;
        for b in &bytes[..n] {
            if *b != 0xf3 {
                normalized[at] = *b;
                at += 1;
            }
        }
        normalized[at] = 0xf3;
        if normalized[..size] == bytes[..size] {
            return Ok(None);
        }
        let decoded = self
            .cs
            .disasm_count(&normalized[..size], u64::from(address), 1)?;
        let Some(insn) = decoded.iter().next() else {
            return Ok(None);
        };
        let mut out = self.instruction(insn)?;
        ensure!(
            usize::from(out.size) <= size,
            "prefix normalization exceeded input"
        );
        out.bytes_hex = hex::encode(&bytes[..usize::from(out.size)]);
        Ok(Some(out))
    }
    fn walk(
        &self,
        bytes: &[u8],
        start: u32,
        entry: u32,
        work: &mut usize,
        batch_limit: usize,
        mut emit: impl FnMut(Instruction) -> bool,
    ) -> Result<()> {
        let mut offset = (entry - start) as usize;
        while offset < bytes.len() {
            ensure!(*work < MAX_WORK, "native decode work limit exceeded");
            let batch = self.cs.disasm_count(
                &bytes[offset..],
                u64::from(start) + offset as u64,
                (MAX_WORK - *work).min(batch_limit),
            )?;
            if batch.is_empty() {
                // Intel defines every 0F AE F8..FF encoding as SFENCE: the
                // r/m field is ignored. Capstone5 rejects seven of them.
                // Admit only these exact encodings and the already qualified
                // ignored FS/address prefixes, retaining the original bytes.
                if let Some(instruction) = sfence_alias(&bytes[offset..], start + offset as u32) {
                    *work += 1;
                    offset = (instruction.end() - start) as usize;
                    if !emit(instruction) {
                        return Ok(());
                    }
                    continue;
                }
                if let Some(instruction) =
                    self.approximation_prefix_order(&bytes[offset..], start + offset as u32)?
                {
                    *work += 1;
                    offset = (instruction.end() - start) as usize;
                    if !emit(instruction) {
                        return Ok(());
                    }
                    continue;
                }
                break;
            }
            *work += batch.len();
            for insn in batch.iter() {
                let instruction = self.instruction(insn)?;
                offset = (instruction.end() - start) as usize;
                if !emit(instruction) {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
    pub fn linear(&self, bytes: &[u8], start: u32, resync: &[u32]) -> Result<Decoded> {
        let (end, memory) = self.reserve(bytes, start)?;
        ensure!(
            resync.len() <= MAX_FUNCTION,
            "too many native resync points"
        );
        let mut decoded = BTreeMap::new();
        let mut work = 0;
        self.walk(bytes, start, start, &mut work, 4096, |insn| {
            decoded.insert(insn.address, insn);
            true
        })?;
        for point in resync
            .iter()
            .copied()
            .filter(|p| (start..end).contains(p))
            .collect::<BTreeSet<_>>()
        {
            if !(start..end).contains(&point) || decoded.contains_key(&point) {
                continue;
            }
            decoded.retain(|&address, insn| !(address < point && point < insn.end()));
            self.walk(bytes, start, point, &mut work, 1, |insn| {
                if decoded.contains_key(&insn.address) {
                    return false;
                }
                decoded.insert(insn.address, insn);
                true
            })?;
        }
        Ok(Decoded {
            instructions: decoded.into_values().collect(),
            _memory: memory,
        })
    }
    pub fn cfg(
        &self,
        bytes: &[u8],
        start: u32,
        entries: &[u32],
        stops: &BTreeSet<u32>,
        stop_mnemonics: &BTreeSet<String>,
    ) -> Result<Decoded> {
        let (end, memory) = self.reserve(bytes, start)?;
        ensure!(
            entries.len() <= MAX_FUNCTION
                && stops.len() <= MAX_FUNCTION
                && stop_mnemonics.len() <= 64,
            "native CFG metadata limit exceeded"
        );
        let mut work = 0;
        let mut pending = entries.to_vec();
        let mut decoded = BTreeMap::new();
        while let Some(entry) = pending.pop() {
            if !(start..end).contains(&entry) || decoded.contains_key(&entry) {
                continue;
            }
            self.walk(bytes, start, entry, &mut work, 1, |insn| {
                if (insn.address != entry && stops.contains(&insn.address))
                    || decoded.contains_key(&insn.address)
                {
                    return false;
                }
                let jump = insn.is_jump();
                let stop = insn.is_ret() || jump || stop_mnemonics.contains(&insn.mnemonic);
                if jump || insn.is_conditional() {
                    if let Some(target) = insn.jump_target.filter(|t| (start..end).contains(t)) {
                        pending.push(target);
                    }
                }
                decoded.insert(insn.address, insn);
                !stop
            })?;
        }
        Ok(Decoded {
            instructions: decoded.into_values().collect(),
            _memory: memory,
        })
    }
}
fn sfence_alias(bytes: &[u8], address: u32) -> Option<Instruction> {
    let prefix = bytes
        .iter()
        .take_while(|b| matches!(b, 0x64 | 0x67))
        .count();
    let size = prefix + 3;
    if size > 15
        || bytes.len() < size
        || bytes[prefix..prefix + 2] != [0x0f, 0xae]
        || bytes[prefix + 2] < 0xf8
    {
        return None;
    }
    Some(Instruction {
        address,
        size: size as u8,
        mnemonic: "sfence".into(),
        op_str: String::new(),
        bytes_hex: hex::encode(&bytes[..size]),
        operands: Vec::new(),
        call_target: None,
        jump_target: None,
        memory_refs: Vec::new(),
        imm_values: Vec::new(),
        regs_written: Vec::new(),
    })
}
pub fn blocks<'a>(
    decoded: &'a Decoded,
    start: u32,
    end: u32,
    extra: &[u32],
    stop_mnemonics: &BTreeSet<String>,
) -> Result<Vec<Block<'a>>> {
    ensure!(
        extra.len() <= MAX_FUNCTION && stop_mnemonics.len() <= 64,
        "native block metadata limit exceeded"
    );
    let instructions = decoded.instructions();
    let lookup: BTreeMap<_, _> = instructions
        .iter()
        .enumerate()
        .map(|(i, insn)| (insn.address, i))
        .collect();
    let mut leaders = BTreeSet::from([start]);
    leaders.extend(extra);
    for insn in instructions {
        if insn.is_jump() || insn.is_conditional() {
            if let Some(target) = insn.jump_target.filter(|t| (start..end).contains(t)) {
                leaders.insert(target);
            }
            leaders.insert(insn.end());
        } else if insn.is_call() || stop_mnemonics.contains(&insn.mnemonic) {
            leaders.insert(insn.end());
        }
    }
    let leaders: Vec<_> = leaders
        .into_iter()
        .filter(|l| lookup.contains_key(l))
        .collect();
    let mut blocks = Vec::new();
    for (index, &leader) in leaders.iter().enumerate() {
        let next = leaders.get(index + 1).copied().unwrap_or(end);
        let mut block = Block {
            start: leader,
            instructions: Vec::new(),
            successors: Vec::new(),
        };
        for insn in &instructions[lookup[&leader]..] {
            if insn.address >= next && insn.address != leader {
                break;
            }
            block.instructions.push(insn);
            if insn.is_ret()
                || insn.is_jump()
                || insn.is_conditional()
                || stop_mnemonics.contains(&insn.mnemonic)
            {
                break;
            }
        }
        if let Some(last) = block.instructions.last() {
            if !last.is_ret() && !stop_mnemonics.contains(&last.mnemonic) {
                if last.is_jump() || last.is_conditional() {
                    if last.is_conditional() && last.end() < end {
                        block.successors.push(last.end());
                    }
                    if let Some(target) = last.jump_target.filter(|t| (start..end).contains(t)) {
                        block.successors.push(target);
                    }
                } else if last.end() < end && lookup.contains_key(&last.end()) {
                    block.successors.push(last.end());
                }
            }
        }
        blocks.push(block);
    }
    Ok(blocks)
}
