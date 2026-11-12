//! Borrowed XBE layout for native generation. Explicit LE/u32 guest addresses;
//! executable bytes remain in owned host memory, never extracted to disk.
use anyhow::{ensure, Context, Result};
use std::ops::Range;

const HEADER: usize = 0x178;
const SECTION_HEADER: usize = 56;
const MAX_SECTIONS: usize = 64;
const EXECUTABLE: u32 = 4;
pub const MAX_XBE: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Section {
    pub name: String,
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_address: u32,
    pub raw_size: u32,
    pub flags: u32,
    pub is_code: bool,
}
pub struct Image<'a> {
    bytes: &'a [u8],
    pub base: u32,
    pub image_size: u32,
    pub entry: u32,
    pub kernel_thunk: u32,
    pub sections: Vec<Section>,
}
/// Validated original-image import slot, never a host function pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernelImport {
    pub slot: u32,
    pub ordinal: u32,
}
/// Entire table is validated before this immutable, allocation-free iterator
/// is returned. The backing image cannot change while any iterator is borrowed.
#[derive(Clone)]
pub struct KernelImports<'image, 'bytes> {
    image: &'image Image<'bytes>,
    current: u32,
    end: u32,
}
impl Iterator for KernelImports<'_, '_> {
    type Item = KernelImport;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            return None;
        }
        let slot = self.current;
        let encoded = self
            .image
            .kernel_import_word(slot)
            .expect("validated immutable import slot");
        self.current += 4; // Constructor checked the complete address progression.
        Some(KernelImport {
            slot,
            ordinal: encoded & 0x7fff_ffff,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = ((self.end - self.current) / 4) as usize;
        (count, Some(count))
    }
}
impl ExactSizeIterator for KernelImports<'_, '_> {}
impl std::iter::FusedIterator for KernelImports<'_, '_> {}
fn word(bytes: &[u8], at: usize) -> Result<u32> {
    let value = bytes
        .get(at..at.checked_add(4).context("XBE field overflow")?)
        .context("truncated XBE field")?;
    Ok(u32::from_le_bytes(value.try_into().unwrap()))
}
fn region(offset: u32, size: u32, limit: usize) -> Result<Range<usize>> {
    let end = u64::from(offset) + u64::from(size);
    ensure!(end <= limit as u64, "XBE file extent is truncated");
    Ok(offset as usize..end as usize)
}
fn header_offset(address: u32, base: u32, headers: u32, size: u32) -> Result<usize> {
    let offset = address
        .checked_sub(base)
        .context("XBE header address below image base")?;
    ensure!(
        u64::from(offset) + u64::from(size) <= u64::from(headers),
        "XBE address outside header"
    );
    Ok(offset as usize)
}
pub fn code_section(name: &str, flags: u32) -> bool {
    // Same explicit code/data exceptions as the pinned original generator.
    if matches!(
        name,
        ".data"
            | ".data1"
            | ".rdata"
            | "EnglishXlate"
            | "JapaneseXlate"
            | "GermanXlate"
            | "FrenchXlate"
            | "SpanishXlate"
            | "ItalianXlate"
    ) {
        return false;
    }
    flags & EXECUTABLE != 0 || name == "XIPS"
}
impl<'a> Image<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        ensure!(
            bytes.len() >= HEADER && bytes.len() <= MAX_XBE,
            "invalid XBE length"
        );
        ensure!(&bytes[..4] == b"XBEH", "invalid XBE magic");
        let base = word(bytes, 0x104)?;
        let headers = word(bytes, 0x108)?;
        let image_size = word(bytes, 0x10c)?;
        let image_end = base
            .checked_add(image_size)
            .context("XBE image address overflow")?;
        ensure!(
            base >= 0x10000
                && image_size > 0
                && headers as usize >= HEADER
                && headers as usize <= bytes.len(),
            "invalid XBE image/header range"
        );
        let count = word(bytes, 0x11c)? as usize;
        ensure!(
            (1..=MAX_SECTIONS).contains(&count),
            "invalid XBE section count"
        );
        let table = header_offset(
            word(bytes, 0x120)?,
            base,
            headers,
            (count * SECTION_HEADER) as u32,
        )?;
        let encoded_entry = word(bytes, 0x128)?;
        let retail = encoded_entry ^ 0xa8fc57ab;
        let (entry, thunk_key) = if (base..image_end).contains(&retail) {
            (retail, 0x5b6d40b6)
        } else {
            let debug = encoded_entry ^ 0x94859d4b;
            ensure!(
                (base..image_end).contains(&debug),
                "XBE entry is outside its image"
            );
            (debug, 0xefb1f152)
        };
        let kernel_thunk = word(bytes, 0x158)? ^ thunk_key;
        let mut sections: Vec<Section> = Vec::with_capacity(count);
        for index in 0..count {
            let at = table + index * SECTION_HEADER;
            let flags = word(bytes, at)?;
            let virtual_address = word(bytes, at + 4)?;
            let virtual_size = word(bytes, at + 8)?;
            let virtual_end = virtual_address
                .checked_add(virtual_size)
                .context("XBE section address overflow")?;
            ensure!(
                virtual_address >= base && virtual_end <= image_end && virtual_size > 0,
                "XBE section is outside its image"
            );
            let raw_address = word(bytes, at + 12)?;
            let raw_size = word(bytes, at + 16)?;
            region(raw_address, raw_size, bytes.len())?;
            ensure!(
                raw_size == 0 || raw_address >= headers,
                "XBE raw section overlaps headers"
            );
            let name_at = header_offset(word(bytes, at + 20)?, base, headers, 1)?;
            let tail = &bytes[name_at..headers as usize];
            let length = tail
                .iter()
                .take(32)
                .position(|b| *b == 0)
                .context("invalid XBE section name")?;
            let name = std::str::from_utf8(&tail[..length])
                .context("non-UTF8 XBE section name")?
                .to_owned();
            ensure!(
                !name.is_empty() && !name.chars().any(char::is_control),
                "invalid XBE section name"
            );
            for old in &sections {
                let old_end = u64::from(old.virtual_address) + u64::from(old.virtual_size);
                ensure!(
                    u64::from(virtual_address) >= old_end
                        || u64::from(virtual_end) <= u64::from(old.virtual_address),
                    "overlapping XBE virtual sections"
                );
                if raw_size != 0 && old.raw_size != 0 {
                    ensure!(
                        u64::from(raw_address)
                            >= u64::from(old.raw_address) + u64::from(old.raw_size)
                            || u64::from(raw_address) + u64::from(raw_size)
                                <= u64::from(old.raw_address),
                        "overlapping XBE raw sections"
                    );
                }
            }
            let is_code = code_section(&name, flags);
            sections.push(Section {
                name,
                virtual_address,
                virtual_size,
                raw_address,
                raw_size,
                flags,
                is_code,
            });
        }
        let image = Self {
            bytes,
            base,
            image_size,
            entry,
            kernel_thunk,
            sections,
        };
        ensure!(
            image.code_bytes(entry, 1).is_ok(),
            "XBE entry does not name backed executable bytes"
        );
        image
            .kernel_import_word(kernel_thunk)
            .context("XBE kernel thunk has no backed bytes")?;
        Ok(image)
    }
    fn kernel_import_word(&self, address: u32) -> Result<u32> {
        let mut bytes = [0; 4];
        for (lane, byte) in bytes.iter_mut().enumerate() {
            let at = address
                .checked_add(lane as u32)
                .context("kernel import word wraps")?;
            *byte = self.bytes_at(at, 1)?[0];
        }
        Ok(u32::from_le_bytes(bytes))
    }
    /// Validate all original ordinal slots through the zero terminator before
    /// exposing any import. Unknown ordinals remain explicit unresolved records.
    /// Monotonic checked addresses and nonoverlapping raw-backed sections bound
    /// the scan by the admitted image; no extra import/instruction quota is used.
    pub fn kernel_imports(&self) -> Result<KernelImports<'_, 'a>> {
        let mut end = self.kernel_thunk;
        loop {
            let encoded = self
                .kernel_import_word(end)
                .context("kernel import table lacks backed terminator")?;
            if encoded == 0 {
                break;
            }
            ensure!(
                encoded & 0x8000_0000 != 0 && encoded & 0x7fff_ffff != 0,
                "kernel import is not a nonzero encoded ordinal"
            );
            end = end
                .checked_add(4)
                .context("kernel import table address wraps")?;
        }
        Ok(KernelImports {
            image: self,
            current: self.kernel_thunk,
            end,
        })
    }
    pub fn bytes_at(&self, address: u32, size: u32) -> Result<&'a [u8]> {
        let section = self
            .sections
            .iter()
            .find(|s| {
                address >= s.virtual_address
                    && u64::from(address) + u64::from(size)
                        <= u64::from(s.virtual_address) + u64::from(s.virtual_size.min(s.raw_size))
            })
            .context("XBE virtual span is not backed by a single raw section")?;
        let offset = section
            .raw_address
            .checked_add(address - section.virtual_address)
            .context("XBE raw address overflow")?;
        Ok(&self.bytes[region(offset, size, self.bytes.len())?])
    }
    pub fn code_bytes(&self, address: u32, size: u32) -> Result<&'a [u8]> {
        ensure!(
            self.sections.iter().any(|s| s.is_code
                && address >= s.virtual_address
                && u64::from(address) + u64::from(size)
                    <= u64::from(s.virtual_address) + u64::from(s.virtual_size.min(s.raw_size))),
            "XBE span is not executable code"
        );
        self.bytes_at(address, size)
    }
}
