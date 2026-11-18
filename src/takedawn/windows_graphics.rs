//! Match a verified native executable's compiled renderer, never GPU availability.
use anyhow::{ensure, Context, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Backend {
    Vulkan,
    D3d11,
    LegacyVulkan,
}
impl Backend {
    pub(crate) fn ready_marker(self) -> Option<&'static str> {
        match self {
            Self::Vulkan => Some("[vulkan] backend up on "),
            Self::D3d11 => Some("[d3d11] backend up on "),
            Self::LegacyVulkan => None,
        }
    }
    pub(crate) fn environment(self) -> &'static str {
        match self {
            Self::Vulkan | Self::LegacyVulkan => "vulkan",
            Self::D3d11 => "d3d11",
        }
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(2).context("PE offset overflow")?)
            .context("truncated PE field")?
            .try_into()?,
    ))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset.checked_add(4).context("PE offset overflow")?)
            .context("truncated PE field")?
            .try_into()?,
    ))
}
struct Section {
    va: u64,
    raw: usize,
    size: usize,
}
fn mapped<'a>(
    bytes: &'a [u8],
    address: u32,
    headers: usize,
    sections: &[Section],
) -> Result<&'a [u8]> {
    let address = u64::from(address);
    let mut result = None;
    if address < headers as u64 {
        result = Some(&bytes[address as usize..headers]);
    }
    for section in sections {
        if address >= section.va && address - section.va < section.size as u64 {
            ensure!(result.is_none(), "ambiguous PE virtual address");
            let start = section.raw + (address - section.va) as usize;
            result = Some(&bytes[start..section.raw + section.size]);
        }
    }
    result.context("PE import address has no file data")
}

pub(crate) fn from_executable(bytes: &[u8]) -> Result<Backend> {
    ensure!(
        bytes.len() >= 64 && bytes.starts_with(b"MZ"),
        "invalid renderer executable"
    );
    let pe = u32_at(bytes, 60)? as usize;
    ensure!(
        pe >= 64
            && bytes.get(pe..pe.checked_add(4).context("PE offset overflow")?) == Some(b"PE\0\0"),
        "invalid renderer PE header"
    );
    let count = usize::from(u16_at(bytes, pe + 6)?);
    let optional_size = usize::from(u16_at(bytes, pe + 20)?);
    let optional = pe.checked_add(24).context("PE offset overflow")?;
    let table = optional
        .checked_add(optional_size)
        .context("PE offset overflow")?;
    let (directory_count, directories) = match u16_at(bytes, optional)? {
        0x20b => (108, 112),
        0x10b => (92, 96),
        _ => anyhow::bail!("unsupported renderer PE width"),
    };
    ensure!(
        count > 0 && count <= 96 && optional_size >= directories,
        "invalid renderer PE layout"
    );
    let section_end = table
        .checked_add(count * 40)
        .context("PE offset overflow")?;
    let headers = u32_at(bytes, optional + 60)? as usize;
    ensure!(
        headers >= section_end && headers <= bytes.len(),
        "invalid renderer PE headers"
    );
    let mut sections = Vec::with_capacity(count);
    for index in 0..count {
        let entry = table + index * 40;
        let section = Section {
            va: u64::from(u32_at(bytes, entry + 12)?),
            size: u32_at(bytes, entry + 16)? as usize,
            raw: u32_at(bytes, entry + 20)? as usize,
        };
        if section.size != 0 {
            ensure!(
                section.raw >= headers
                    && section
                        .raw
                        .checked_add(section.size)
                        .is_some_and(|end| end <= bytes.len()),
                "invalid renderer PE section"
            );
        }
        sections.push(section);
    }
    // Historical asset-free lifecycle fixtures have no graphics imports. Keep
    // their existing Vulkan environment without requiring a graphics loader.
    if u32_at(bytes, optional + directory_count)? < 2 {
        return Ok(Backend::LegacyVulkan);
    }
    ensure!(
        optional_size >= directories + 16,
        "truncated PE import directory"
    );
    let address = u32_at(bytes, optional + directories + 8)?;
    let size = u32_at(bytes, optional + directories + 12)? as usize;
    if address == 0 && size == 0 {
        return Ok(Backend::LegacyVulkan);
    }
    ensure!(
        address != 0 && (20..=1024 * 1024).contains(&size),
        "invalid PE import directory"
    );
    let table = mapped(bytes, address, headers, &sections)?
        .get(..size)
        .context("truncated PE import table")?;
    let mut vulkan = false;
    let mut d3d11 = false;
    let mut terminated = false;
    for descriptor in table.chunks_exact(20).take(4096) {
        if descriptor.iter().all(|x| *x == 0) {
            terminated = true;
            break;
        }
        let name = u32_at(descriptor, 12)?;
        ensure!(name != 0, "invalid PE import name");
        let tail = mapped(bytes, name, headers, &sections)?;
        let length = tail
            .iter()
            .take(256)
            .position(|x| *x == 0)
            .context("unterminated PE import name")?;
        let name = &tail[..length];
        ensure!(
            !name.is_empty()
                && name
                    .iter()
                    .all(|x| x.is_ascii_alphanumeric() || b"._-".contains(x)),
            "invalid PE import basename"
        );
        vulkan |= name.eq_ignore_ascii_case(b"vulkan-1.dll");
        d3d11 |= name.eq_ignore_ascii_case(b"d3d11.dll");
    }
    ensure!(
        terminated && !(vulkan && d3d11),
        "ambiguous or unterminated renderer imports"
    );
    Ok(if d3d11 {
        Backend::D3d11
    } else if vulkan {
        Backend::Vulkan
    } else {
        Backend::LegacyVulkan
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn image(names: &[&str], wide: bool) -> Vec<u8> {
        let mut b = vec![0; 0x800];
        b[..2].copy_from_slice(b"MZ");
        put(&mut b, 60, 64);
        b[64..68].copy_from_slice(b"PE\0\0");
        b[70..72].copy_from_slice(&2u16.to_le_bytes());
        let optional = 88;
        let (size, magic, directories, n) = if wide {
            (240u16, 0x20bu16, 112, 108)
        } else {
            (224, 0x10b, 96, 92)
        };
        b[84..86].copy_from_slice(&size.to_le_bytes());
        b[optional..optional + 2].copy_from_slice(&magic.to_le_bytes());
        put(&mut b, optional + 60, 512);
        put(&mut b, optional + n, 16);
        put(&mut b, optional + directories + 8, 0x1000);
        put(
            &mut b,
            optional + directories + 12,
            ((names.len() + 1) * 20) as u32,
        );
        for (index, (va, raw, length)) in [(0x1000, 0x200, 0x400), (0x2000, 0x600, 0x200)]
            .iter()
            .enumerate()
        {
            let s = optional + usize::from(size) + index * 40;
            put(&mut b, s + 12, *va);
            put(&mut b, s + 16, *length);
            put(&mut b, s + 20, *raw);
        }
        for (index, name) in names.iter().enumerate() {
            let position = 0x320 + index * 64;
            put(&mut b, 0x200 + index * 20, 0x2000);
            put(&mut b, 0x200 + index * 20 + 16, 0x2010);
            put(
                &mut b,
                0x200 + index * 20 + 12,
                0x1000 + (position - 0x200) as u32,
            );
            b[position..position + name.len()].copy_from_slice(name.as_bytes());
        }
        b
    }
    #[test]
    fn matches_only_actual_imports_on_both_widths() {
        for wide in [false, true] {
            assert_eq!(
                from_executable(&image(&["KERNEL32.dll"], wide)).unwrap(),
                Backend::LegacyVulkan
            );
            assert_eq!(
                from_executable(&image(&["vulkan-1.dll"], wide)).unwrap(),
                Backend::Vulkan
            );
            let mut b = image(&["D3D11.DLL", "dxgi.dll"], wide);
            b[0x600..0x60c].copy_from_slice(b"vulkan-1.dll");
            assert_eq!(from_executable(&b).unwrap(), Backend::D3d11);
            assert!(from_executable(&image(&["vulkan-1.dll", "d3d11.dll"], wide)).is_err());
        }
        assert_eq!(Backend::Vulkan.environment(), "vulkan");
        assert_eq!(Backend::D3d11.environment(), "d3d11");
        assert!(Backend::Vulkan.ready_marker().is_some());
        assert!(Backend::D3d11.ready_marker().is_some());
        assert!(Backend::LegacyVulkan.ready_marker().is_none());
    }
    #[test]
    #[ignore = "Requires current built Windows game artifacts, not an ISO"]
    fn current_built_renderer_imports() {
        for (variable, expected) in [
            ("GAMEPKG_TEST_VULKAN_EXE", Backend::Vulkan),
            ("GAMEPKG_TEST_D3D11_EXE", Backend::D3d11),
        ] {
            let path =
                std::env::var_os(variable).expect("set current Windows executable fixture path");
            let bytes = std::fs::read(path).unwrap();
            assert_eq!(from_executable(&bytes).unwrap(), expected);
        }
    }
    #[test]
    fn malformed_ranges_and_names_fail_closed() {
        let b = image(&["d3d11.dll"], true);
        for length in 0..b.len() {
            assert!(
                from_executable(&b[..length]).is_err(),
                "accepted truncation {length}"
            );
        }
        for (offset, value) in [
            (60, u32::MAX),
            (88 + 60, u32::MAX),
            (88 + 112 + 8, u32::MAX),
            (88 + 112 + 12, u32::MAX),
            (0x20c, 0),
            (0x20c, 0xfffffff0),
        ] {
            let mut bad = b.clone();
            put(&mut bad, offset, value);
            assert!(from_executable(&bad).is_err(), "offset {offset}");
        }
        let mut bad = b.clone();
        bad[0x320..0x420].fill(b'A');
        assert!(from_executable(&bad).is_err());
        assert!(from_executable(&image(&["../d3d11.dll"], true)).is_err());
        let mut bad = b.clone();
        put(&mut bad, 88 + 240 + 40 + 12, 0x1000);
        assert!(from_executable(&bad).is_err());
    }
}
