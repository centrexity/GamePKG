//! Reviewed host loader data, not callable guest/native imports.
//! ABI: XboxDev/nxdk lib/xboxkrnl/xboxkrnl.h and xboxkrnl.exe.def (CC0-1.0).
//! Hardware values describe an explicit emulated profile, not the host PC.
use crate::xbe::Image;
use anyhow::{ensure, Context, Result};

pub const HARDWARE_INFO_ORDINAL: u32 = 322;
pub const HARDWARE_INFO_BYTES: u32 = 8;
pub const KERNEL_VERSION_ORDINAL: u32 = 324;
pub const DEVKIT_KERNEL_FLAG: u32 = 0x02;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HardwareInfo {
    pub flags: u32,
    pub gpu_revision: u8,
    pub mcp_revision: u8,
}
impl HardwareInfo {
    /// Explicit Xbox LE image. Reserved bytes are zero, independent of native
    /// struct packing, pointer width, byte order, or host hardware revisions.
    pub fn to_le_bytes(self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&self.flags.to_le_bytes());
        bytes[4] = self.gpu_revision;
        bytes[5] = self.mcp_revision;
        bytes
    }
}

/// Explicit emulated-kernel compatibility identity, not the host OS version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelVersion {
    pub major: u16,
    pub minor: u16,
    pub build: u16,
    pub qfe: u16,
}
impl KernelVersion {
    pub fn to_le_bytes(self) -> [u8; 8] {
        let mut bytes = [0; 8];
        for (chunk, value) in bytes
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip([self.major, self.minor, self.build, self.qfe])
        {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelDataAddresses {
    pub hardware_info: u32,
    pub kernel_version: u32,
}
/// Atomically bind the two reviewed static platform DATA exports. All target
/// ranges, original table entries and selected loaded slots are checked before
/// any write. Both eight-byte profiles must separately be registered read-only
/// by the caller before guest entry; this API does not create that capability.
pub fn bind_platform_data(
    image: &Image<'_>,
    ram_base: u32,
    ram: &mut [u8],
    targets: KernelDataAddresses,
) -> Result<usize> {
    bind_selected(
        image,
        ram_base,
        ram,
        &[
            (HARDWARE_INFO_ORDINAL, targets.hardware_info),
            (KERNEL_VERSION_ORDINAL, targets.kernel_version),
        ],
    )
}
/// Bind only the reviewed hardware-info DATA import, before guest entry.
/// `ram` is the loaded guest arena starting at `ram_base`; the original image
/// remains immutable. All table entries and all selected loaded slots are
/// validated before any mutation. Unrelated function/data imports are untouched.
/// Caller must separately register the eight-byte profile at `info_address`
/// as checked read-only RAM, with storage that outlives every guest worker.
/// This function neither creates that capability nor guesses hardware values.
/// Returns the number of patched slots (including duplicate imports). Rebinding
/// or corrupting a selected slot fails with the entire arena unchanged.
pub fn bind_hardware_info(
    image: &Image<'_>,
    ram_base: u32,
    ram: &mut [u8],
    info_address: u32,
) -> Result<usize> {
    bind_selected(
        image,
        ram_base,
        ram,
        &[(HARDWARE_INFO_ORDINAL, info_address)],
    )
}
fn bind_selected(
    image: &Image<'_>,
    ram_base: u32,
    ram: &mut [u8],
    targets: &[(u32, u32)],
) -> Result<usize> {
    for (index, &(ordinal, address)) in targets.iter().enumerate() {
        let end = address
            .checked_add(7)
            .context("kernel-data address wraps")?;
        ensure!(
            address != 0 && address != (0x8000_0000 | ordinal),
            "kernel-data address is null or unresolved"
        );
        ensure!(
            u64::from(end) < u64::from(image.base)
                || u64::from(address) >= u64::from(image.base) + u64::from(image.image_size),
            "kernel-data capability overlaps title image"
        );
        for &(_, old) in &targets[..index] {
            ensure!(
                u64::from(address) + 8 <= u64::from(old)
                    || u64::from(old) + 8 <= u64::from(address),
                "kernel-data capabilities overlap"
            );
        }
    }
    ensure!(
        u64::from(ram_base) + ram.len() as u64 <= u64::from(u32::MAX) + 1,
        "loaded guest arena wraps"
    );
    let imports = image.kernel_imports()?;
    let mut count = 0;
    for import in imports.clone() {
        if targets
            .iter()
            .any(|&(ordinal, _)| ordinal == import.ordinal)
        {
            let offset = import
                .slot
                .checked_sub(ram_base)
                .context("kernel import precedes loaded arena")? as usize;
            let end = offset
                .checked_add(4)
                .context("loaded import slot overflow")?;
            let bytes = ram
                .get(offset..end)
                .context("loaded import slot is unbacked")?;
            ensure!(
                u32::from_le_bytes(bytes.try_into().unwrap()) == (0x8000_0000 | import.ordinal),
                "loaded data import differs from original image"
            );
            count += 1;
        }
    }
    // All writes were validated against immutable source and exclusively
    // borrowed RAM. No allocation or fallible operation occurs during commit.
    for import in imports {
        if let Some(&(_, address)) = targets
            .iter()
            .find(|&&(ordinal, _)| ordinal == import.ordinal)
        {
            let offset = (import.slot - ram_base) as usize;
            ram[offset..offset + 4].copy_from_slice(&address.to_le_bytes());
        }
    }
    Ok(count)
}
