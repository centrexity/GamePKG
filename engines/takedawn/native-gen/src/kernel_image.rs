//! GamePKG's own small virtual-kernel module, not a retail kernel or BIOS dump.
//! Its PE metadata describes backed pages, host trap entries and actual INIT
//! code. Unknown services remain unresolved; an export is not an implemented
//! kernel-service claim. Format: Microsoft PE/COFF. Ordinals: XboxDev/nxdk CC0.
use crate::{
    kernel_data::{HardwareInfo, KernelVersion},
    xbe::Image,
};
use anyhow::{ensure, Context, Result};
use gamepkg_takedawn_media::{Budget, Lease};

pub const BASE: u32 = 0x8001_0000;
pub const PAGE: usize = 0x1000;
pub const IMAGE_BYTES: usize = 6 * PAGE;
pub const INIT_RVA: u32 = 0x5000;
pub const GDT_RVA: u32 = 0x4000;
pub const HARDWARE_RVA: u32 = 0x2f00;
pub const VERSION_RVA: u32 = 0x2f10;
// Append entries to preserve existing trap PCs across profile extensions.
pub const FUNCTION_ORDINALS: [u32; 32] = [
    255, 277, 291, 294, 47, 107, 129, 160, 161, 113, 112, 108, 24, 301, 149, 150, 97, 119, 137,
    184, 199, 202, 187, 218, 67, 69, 190, 289, 279, 211, 236, 219,
];
pub const SHUTDOWN_HEAD_RVA: u32 = GDT_RVA + 16;
pub const TIMER_HEAD_RVA: u32 = GDT_RVA + 24;
pub const CALLBACK_RETURN_RVA: u32 = 0x1100;
pub const DPC_RETURN_RVA: u32 = 0x1108;
const EXPORT_RVA: usize = 0x2000;
const EAT_RVA: usize = 0x2080;
const SECTIONS: usize = 0x80 + 24 + 224;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    HostTrap,
    Data,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Export {
    pub ordinal: u32,
    pub address: u32,
    pub kind: ExportKind,
}
pub struct KernelImage {
    bytes: Vec<u8>,
    _lease: Lease,
}
fn put16(b: &mut [u8], at: usize, n: u16) {
    b[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn put32(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn export(ordinal: u32) -> Option<Export> {
    let (rva, kind) = match ordinal {
        322 => (HARDWARE_RVA, ExportKind::Data),
        324 => (VERSION_RVA, ExportKind::Data),
        _ => (
            0x1000 + u32::try_from(FUNCTION_ORDINALS.iter().position(|&n| n == ordinal)?).ok()? * 8,
            ExportKind::HostTrap,
        ),
    };
    Some(Export {
        ordinal,
        address: BASE + rva,
        kind,
    })
}
impl KernelImage {
    pub fn build(hardware: HardwareInfo, version: KernelVersion, budget: &Budget) -> Result<Self> {
        let lease = budget.reserve(IMAGE_BYTES as u64)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(IMAGE_BYTES)?;
        bytes.resize(IMAGE_BYTES, 0);
        bytes[..2].copy_from_slice(b"MZ");
        put32(&mut bytes, 0x3c, 0x80);
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        put16(&mut bytes, 0x84, 0x14c);
        put16(&mut bytes, 0x86, 4);
        put16(&mut bytes, 0x94, 224);
        put16(&mut bytes, 0x96, 0x103);
        let optional = 0x98;
        put16(&mut bytes, optional, 0x10b);
        for (offset, value) in [
            (4, 0x2000),
            (8, 0x3000),
            (16, INIT_RVA),
            (20, 0x1000),
            (24, 0x2000),
            (28, BASE),
            (32, 0x1000),
            (36, 0x1000),
            (56, IMAGE_BYTES as u32),
            (60, 0x1000),
            (72, 0x10000),
            (76, 0x1000),
            (80, 0x10000),
            (84, 0x1000),
            (92, 16),
            (96, EXPORT_RVA as u32),
            (100, 0x600),
        ] {
            put32(&mut bytes, optional + offset, value);
        }
        put16(&mut bytes, optional + 40, 1);
        put16(&mut bytes, optional + 48, 1);
        put16(&mut bytes, optional + 68, 1); // Native subsystem; no host loader execution.
        for (index, (name, rva, size, flags)) in [
            (b".text\0\0\0", 0x1000, 0x1000, 0x60000020),
            (b".rdata\0\0", 0x2000, 0x2000, 0x40000040),
            (b".data\0\0\0", GDT_RVA, 0x1000, 0xc0000040),
            (b"INIT\0\0\0\0", INIT_RVA, 0x1000, 0x62000020),
        ]
        .into_iter()
        .enumerate()
        {
            let at = SECTIONS + index * 40;
            bytes[at..at + 8].copy_from_slice(name);
            for (offset, value) in [(8, size), (12, rva), (16, size), (20, rva), (36, flags)] {
                put32(&mut bytes, at + offset, value);
            }
        }
        // Ordinal-only exports; holes are absent entries, never generic stubs.
        put32(&mut bytes, EXPORT_RVA + 12, 0x2040);
        put32(&mut bytes, EXPORT_RVA + 16, 1);
        put32(&mut bytes, EXPORT_RVA + 20, 324);
        put32(&mut bytes, EXPORT_RVA + 28, EAT_RVA as u32);
        // Back even the empty name/ordinal tables. Some PE consumers resolve
        // their RVAs before checking NumberOfNames, which is explicitly zero.
        put32(&mut bytes, EXPORT_RVA + 32, 0x2590);
        put32(&mut bytes, EXPORT_RVA + 36, 0x2590);
        bytes[0x2040..0x204d].copy_from_slice(b"xboxkrnl.exe\0");
        for ordinal in FUNCTION_ORDINALS.into_iter().chain([322, 324]) {
            let e = export(ordinal).unwrap();
            let rva = e.address - BASE;
            put32(&mut bytes, EAT_RVA + (ordinal as usize - 1) * 4, rva);
            if e.kind == ExportKind::HostTrap {
                // Guarded host boundary, not a successful no-op API or RET.
                bytes[rva as usize..rva as usize + 4].copy_from_slice(&[0x0f, 0x0b, 0x47, 0x50]);
                put32(&mut bytes, rva as usize + 4, ordinal);
            }
        }
        bytes[CALLBACK_RETURN_RVA as usize..CALLBACK_RETURN_RVA as usize + 8]
            .copy_from_slice(&[0x0f, 0x0b, 0x47, 0x50, 0x52, 0x45, 0x54, 0]);
        bytes[DPC_RETURN_RVA as usize..DPC_RETURN_RVA as usize + 8]
            .copy_from_slice(&[0x0f, 0x0b, 0x47, 0x50, 0x44, 0x50, 0x43, 0]);
        bytes[HARDWARE_RVA as usize..HARDWARE_RVA as usize + 8]
            .copy_from_slice(&hardware.to_le_bytes());
        bytes[VERSION_RVA as usize..VERSION_RVA as usize + 8]
            .copy_from_slice(&version.to_le_bytes());
        // Actual initialization writes the null and flat accessed code
        // descriptors into this module's writable .data. The boot CPU cache
        // must be explicitly supplied by the loader; this code does not fake it.
        let mut at = INIT_RVA as usize;
        for (offset, value) in [(0, 0), (4, 0), (8, 0x0000ffff), (12, 0x00cf9b00)] {
            bytes[at..at + 2].copy_from_slice(&[0xc7, 0x05]);
            put32(&mut bytes, at + 2, BASE + GDT_RVA + offset);
            put32(&mut bytes, at + 6, value);
            at += 10;
        }
        bytes[at] = 0xc3;
        Ok(Self {
            bytes,
            _lease: lease,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn init_code(&self) -> &[u8] {
        &self.bytes[INIT_RVA as usize..INIT_RVA as usize + 41]
    }
    pub fn export(&self, ordinal: u32) -> Option<Export> {
        export(ordinal)
    }
    /// Only exact trap entry PCs qualify; data/interior/INIT PCs cannot invoke
    /// a host service. No x86 UD2 instruction is admitted by this method.
    pub fn host_ordinal(&self, address: u32) -> Option<u32> {
        FUNCTION_ORDINALS
            .into_iter()
            .find(|&n| export(n).unwrap().address == address)
    }
    /// Validate all selected original/loaded import slots before atomic commit.
    /// Unknown imports stay explicit unresolved ordinals. Caller must map this
    /// exact module with read-only headers/code and writable data before title
    /// entry. Translated code registration is separate from these RAM rights.
    pub fn bind_imports(&self, image: &Image<'_>, ram_base: u32, ram: &mut [u8]) -> Result<usize> {
        let arena_end = u64::from(ram_base) + ram.len() as u64;
        ensure!(arena_end <= u64::from(u32::MAX) + 1, "loaded arena wraps");
        ensure!(
            u64::from(image.base) + u64::from(image.image_size) <= u64::from(BASE)
                || u64::from(image.base) >= u64::from(BASE) + IMAGE_BYTES as u64,
            "kernel module overlaps title image"
        );
        let imports = image.kernel_imports()?;
        let mut count = 0;
        for i in imports.clone() {
            if export(i.ordinal).is_some() {
                let at = i
                    .slot
                    .checked_sub(ram_base)
                    .context("import before arena")? as usize;
                let end = at.checked_add(4).context("import offset wraps")?;
                let slot = ram.get(at..end).context("unbacked loaded import")?;
                ensure!(
                    u32::from_le_bytes(slot.try_into().unwrap()) == (0x80000000 | i.ordinal),
                    "loaded import differs from original"
                );
                count += 1;
            }
        }
        for i in imports {
            if let Some(e) = export(i.ordinal) {
                let at = (i.slot - ram_base) as usize;
                ram[at..at + 4].copy_from_slice(&e.address.to_le_bytes());
            }
        }
        Ok(count)
    }
}
