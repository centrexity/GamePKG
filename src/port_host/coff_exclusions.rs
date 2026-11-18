//! Closed normalization for Rust/MinGW's compiler-generated symbol exclusions.
//! Never a general linker-directive allowlist. The ordinary object audit still
//! rejects every raw .drectve; only exact inert rewritten objects may be linked.
use anyhow::{ensure, Context, Result};
use std::collections::BTreeSet;

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .context("truncated COFF field")?
            .try_into()?,
    ))
}
fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .context("truncated COFF field")?
            .try_into()?,
    ))
}
fn range(bytes: &[u8], at: usize, length: usize) -> Result<std::ops::Range<usize>> {
    let end = at.checked_add(length).context("COFF range overflow")?;
    ensure!(end <= bytes.len(), "COFF range outside object");
    Ok(at..end)
}
fn overlap(a: &std::ops::Range<usize>, b: &std::ops::Range<usize>) -> bool {
    a.start < b.end && b.start < a.end
}
fn rust_symbol(symbol: &str) -> bool {
    let raw = if symbol.starts_with("__ZN") || symbol.starts_with("__R") {
        &symbol[1..]
    } else {
        symbol
    };
    !symbol.is_empty()
        && symbol.len() <= 4096
        && symbol.is_ascii()
        && !symbol
            .chars()
            .any(|c| c.is_ascii_control() || c.is_whitespace() || c == '"' || c == '\\')
        && (raw.starts_with("_ZN") || raw.starts_with("_R"))
        && rustc_demangle::try_demangle(raw).is_ok()
}
fn exclusions(data: &[u8], definitions: &BTreeSet<String>, i686: bool) -> Result<()> {
    ensure!(
        data.len() <= 1024 * 1024 && data.is_ascii(),
        "invalid compiler exclusion metadata"
    );
    let text = std::str::from_utf8(data)?;
    let mut seen = BTreeSet::new();
    for token in text.split_ascii_whitespace() {
        let value = token
            .strip_prefix("-exclude-symbols:")
            .context("unapproved linker directive")?;
        let symbol = if let Some(value) = value.strip_prefix('"') {
            value
                .strip_suffix('"')
                .context("unclosed compiler exclusion quote")?
        } else {
            value
        };
        ensure!(
            rust_symbol(symbol),
            "compiler exclusion is not a Rust symbol: {symbol}"
        );
        // MinGW i686 directives use the undecorated Rust name; COFF's
        // external definition has exactly one additional C ABI underscore.
        let definition = if i686 {
            format!("_{symbol}")
        } else {
            symbol.to_owned()
        };
        ensure!(
            definitions.contains(&definition),
            "compiler exclusion has no same-object definition: {symbol}"
        );
        ensure!(
            seen.insert(symbol),
            "duplicate compiler exclusion: {symbol}"
        );
        // A mangled compiler-runtime definition cannot be laundered through
        // metadata either. Full imports/definitions are audited after rewriting.
        super::audit::check_symbols(std::iter::empty::<&str>(), [definition.as_str()])?;
    }
    Ok(())
}

/// Return a copy with only validated exclusion metadata made inert. No code,
/// relocations, symbol records or other section bytes are changed. The caller
/// records the raw hash and seals/audits the exact returned bytes before linking.
pub fn normalize(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    ensure!(bytes.len() >= 20, "truncated COFF header");
    ensure!(
        matches!(u16_at(bytes, 0)?, 0x14c | 0x8664) && u16_at(bytes, 16)? == 0,
        "not a supported relocatable COFF object"
    );
    let count = usize::from(u16_at(bytes, 2)?);
    ensure!((1..=4096).contains(&count), "invalid COFF section count");
    let headers = range(bytes, 0, 20 + count * 40)?;
    let mut target = None;
    for index in 0..count {
        let at = 20 + index * 40;
        if &bytes[at..at + 8] == b".drectve" {
            ensure!(
                target.replace((index, at)).is_none(),
                "duplicate directive sections"
            );
        }
    }
    let Some((section, at)) = target else {
        return Ok(None);
    };
    ensure!(
        u32_at(bytes, at + 36)? == 0x00100a00,
        "unapproved directive section flags"
    );
    ensure!(
        u32_at(bytes, at + 24)? == 0
            && u32_at(bytes, at + 28)? == 0
            && u16_at(bytes, at + 32)? == 0
            && u16_at(bytes, at + 34)? == 0,
        "directive section has relocations or line records"
    );
    let size = u32_at(bytes, at + 16)? as usize;
    let offset = u32_at(bytes, at + 20)? as usize;
    ensure!(
        size <= 1024 * 1024 && offset >= headers.end,
        "invalid directive section range"
    );
    let data = range(bytes, offset, size)?;
    let symbols = u32_at(bytes, 8)? as usize;
    let records = u32_at(bytes, 12)? as usize;
    ensure!(
        records <= 1000000 && symbols >= headers.end,
        "invalid COFF symbol table"
    );
    let table = range(
        bytes,
        symbols,
        records.checked_mul(18).context("symbol count overflow")?,
    )?;
    let strings_size = u32_at(bytes, table.end)? as usize;
    ensure!(
        (4..=16 * 1024 * 1024).contains(&strings_size),
        "invalid COFF string table"
    );
    let strings = range(bytes, table.end, strings_size)?;
    ensure!(
        !overlap(&data, &table) && !overlap(&data, &strings),
        "directive bytes overlap symbol records"
    );
    for index in 0..count {
        if index == section {
            continue;
        }
        let base = 20 + index * 40;
        let size = u32_at(bytes, base + 16)? as usize;
        let start = u32_at(bytes, base + 20)? as usize;
        if start == 0 {
            ensure!(
                size == 0 || u32_at(bytes, base + 36)? & 0x80 != 0,
                "unbacked non-BSS section"
            );
        } else {
            let other = range(bytes, start, size)?;
            ensure!(
                start >= headers.end && !overlap(&data, &other),
                "directive bytes overlap another section"
            );
        }
        for (pointer, number, width) in [
            (base + 24, base + 32, 10usize),
            (base + 28, base + 34, 6usize),
        ] {
            let length = usize::from(u16_at(bytes, number)?) * width;
            if length != 0 {
                let other = range(bytes, u32_at(bytes, pointer)? as usize, length)?;
                ensure!(
                    other.start >= headers.end && !overlap(&data, &other),
                    "directive bytes overlap relocations"
                );
            }
        }
    }
    let mut definitions = BTreeSet::new();
    let mut index = 0usize;
    while index < records {
        let record = symbols + index * 18;
        let name = if u32_at(bytes, record)? == 0 {
            let start = u32_at(bytes, record + 4)? as usize;
            ensure!(
                (4..strings_size).contains(&start),
                "COFF symbol name outside string table"
            );
            let tail = &bytes[strings.start + start..strings.end];
            let end = tail
                .iter()
                .take(4097)
                .position(|byte| *byte == 0)
                .context("unbounded COFF symbol name")?;
            std::str::from_utf8(&tail[..end])?
        } else {
            let raw = &bytes[record..record + 8];
            let end = raw.iter().position(|byte| *byte == 0).unwrap_or(8);
            std::str::from_utf8(&raw[..end])?
        };
        let owner = u16_at(bytes, record + 12)? as i16;
        let storage = bytes[record + 16];
        let auxiliary = usize::from(bytes[record + 17]);
        ensure!(
            index + 1 + auxiliary <= records,
            "COFF auxiliary records outside table"
        );
        if owner == section as i16 + 1 {
            ensure!(
                storage == 3
                    && name == ".drectve"
                    && u32_at(bytes, record + 8)? == 0
                    && auxiliary == 1,
                "runtime symbol refers to directive data"
            );
        } else if owner > 0 && storage == 2 {
            ensure!(owner as usize <= count, "definition has invalid section");
            definitions.insert(name.to_owned());
        }
        index += 1 + auxiliary;
    }
    exclusions(
        &bytes[data.clone()],
        &definitions,
        u16_at(bytes, 0)? == 0x14c,
    )?;
    let mut normalized = bytes.to_vec();
    normalized[at..at + 8].copy_from_slice(b".gpkmeta");
    normalized[at + 36..at + 40].copy_from_slice(&0x00100800u32.to_le_bytes());
    normalized[data].fill(0);
    Ok(Some(normalized))
}

#[cfg(test)]
pub(super) fn fixture(directive: &str) -> Vec<u8> {
    let symbol = "_ZN9game_unit4test17h0123456789abcdefE";
    let symbols = 101 + directive.len();
    let mut bytes = vec![0; symbols + 54 + 4 + symbol.len() + 1];
    bytes[..2].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[2..4].copy_from_slice(&2u16.to_le_bytes());
    bytes[8..12].copy_from_slice(&(symbols as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&3u32.to_le_bytes());
    bytes[20..25].copy_from_slice(b".text");
    bytes[36..40].copy_from_slice(&1u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&100u32.to_le_bytes());
    bytes[56..60].copy_from_slice(&0x60500020u32.to_le_bytes());
    bytes[60..68].copy_from_slice(b".drectve");
    bytes[76..80].copy_from_slice(&(directive.len() as u32).to_le_bytes());
    bytes[80..84].copy_from_slice(&101u32.to_le_bytes());
    bytes[96..100].copy_from_slice(&0x00100a00u32.to_le_bytes());
    bytes[100] = 0xc3;
    bytes[101..symbols].copy_from_slice(directive.as_bytes());
    bytes[symbols + 4..symbols + 8].copy_from_slice(&4u32.to_le_bytes());
    bytes[symbols + 12..symbols + 14].copy_from_slice(&1u16.to_le_bytes());
    bytes[symbols + 16] = 2;
    bytes[symbols + 18..symbols + 26].copy_from_slice(b".drectve");
    bytes[symbols + 30..symbols + 32].copy_from_slice(&2u16.to_le_bytes());
    bytes[symbols + 34] = 3;
    bytes[symbols + 35] = 1;
    bytes[symbols + 54..symbols + 58].copy_from_slice(&((symbol.len() + 5) as u32).to_le_bytes());
    bytes[symbols + 58..symbols + 58 + symbol.len()].copy_from_slice(symbol.as_bytes());
    bytes
}
#[cfg(test)]
mod tests {
    use super::*;
    const SYMBOL: &str = "_ZN9game_unit4test17h0123456789abcdefE";
    #[test]
    fn only_local_rust_exclusions_are_made_inert_without_changing_code_or_symbols() {
        for directive in [
            format!(" -exclude-symbols:{SYMBOL}"),
            format!(" -exclude-symbols:\"{SYMBOL}\" "),
        ] {
            let raw = fixture(&directive);
            let normalized = normalize(&raw).unwrap().unwrap();
            assert_eq!(raw.len(), normalized.len());
            for index in 0..raw.len() {
                if !(60..68).contains(&index)
                    && !(96..100).contains(&index)
                    && !(101..101 + directive.len()).contains(&index)
                {
                    assert_eq!(raw[index], normalized[index], "changed byte {index}");
                }
            }
            assert!(normalized[101..101 + directive.len()]
                .iter()
                .all(|b| *b == 0));
            assert!(normalize(&normalized).unwrap().is_none());
        }
    }
    #[test]
    fn i686_requires_exactly_one_target_decoration_and_x64_does_not_accept_it() {
        let directive = format!(" -exclude-symbols:{SYMBOL}");
        let mut raw = fixture(&directive);
        raw[..2].copy_from_slice(&0x14cu16.to_le_bytes());
        assert!(normalize(&raw).is_err());
        let symbols = u32_at(&raw, 8).unwrap() as usize;
        let strings = symbols + 54;
        raw.insert(strings + 4, b'_');
        raw[strings..strings + 4].copy_from_slice(&((SYMBOL.len() + 6) as u32).to_le_bytes());
        normalize(&raw).unwrap().unwrap();
        raw[..2].copy_from_slice(&0x8664u16.to_le_bytes());
        assert!(normalize(&raw).is_err());
    }
    #[test]
    fn arbitrary_directives_nonlocal_symbols_and_malformed_quotes_are_rejected() {
        for directive in [
            " /DEFAULTLIB:evil.lib".into(),
            " -include:evil".into(),
            " -export:evil".into(),
            " -exclude-symbols:CreateFileW".into(),
            " -exclude-symbols:_ZN9game_unit5other17h0123456789abcdefE".into(),
            format!(" -exclude-symbols:\"{SYMBOL}"),
            format!(" -exclude-symbols:{SYMBOL}\""),
            format!(" -exclude-symbols:{SYMBOL} -exclude-symbols:{SYMBOL}"),
            format!(" -exclude-symbols:{SYMBOL} /DEFAULTLIB:evil.lib"),
            format!(" -exclude-symbols:{SYMBOL}\0"),
        ] {
            assert!(normalize(&fixture(&directive)).is_err(), "{directive:?}");
        }
    }
    #[test]
    fn malformed_overlapping_relocated_and_executable_metadata_is_rejected() {
        let raw = fixture(&format!(" -exclude-symbols:{SYMBOL}"));
        for (at, value) in [
            (96, 0x60100a00u32),
            (84, 100),
            (88, 100),
            (92, 1),
            (80, 20),
            (80, 100),
            (80, u32::MAX),
            (40, 101),
            (8, 101),
            (12, u32::MAX),
        ] {
            let mut bad = raw.clone();
            bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
            assert!(normalize(&bad).is_err(), "field {at}, value {value}");
        }
        let symbols = u32_at(&raw, 8).unwrap() as usize;
        for (offset, value) in [
            (symbols + 16, 3),
            (symbols + 34, 2),
            (symbols + 35, 0),
            (symbols + 35, 255),
        ] {
            let mut bad = raw.clone();
            bad[offset] = value;
            assert!(normalize(&bad).is_err());
        }
        for length in [0, 19, 99, 100, raw.len() - 1] {
            assert!(normalize(&raw[..length]).is_err());
        }
    }
}
