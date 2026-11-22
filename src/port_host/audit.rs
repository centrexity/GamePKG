//! Build gate for game objects, before linking the audited host runtime.
//! This complements source validation and OS confinement; it cannot establish
//! the safety of arbitrary precompiled code or detect embedded raw syscalls.
use anyhow::{bail, ensure, Context, Result};
use std::{collections::BTreeSet, path::Path, time::Duration};

const I686_COFF_STACK_PROBE: &str = "__alloca";

fn reserved(symbol: &str) -> bool {
    if symbol.starts_with("__imp_") || symbol.starts_with("__IMPORT_DESCRIPTOR_") {
        return true;
    }
    let s = symbol.trim_start_matches(['_', '@']);
    if s.starts_with("imp_") || s.starts_with("IMPORT_DESCRIPTOR_") {
        return true;
    }
    let s = s.split('@').next().unwrap_or(s);
    s.starts_with("gamepkg_")
        || s.starts_with("SDL")
        || s.starts_with("vk")
        || s.starts_with("glX")
        || s.starts_with("wgl")
        || (s.starts_with("gl") && s.as_bytes().get(2).is_some_and(u8::is_ascii_uppercase))
        || matches!(
            s,
            "open"
                | "open64"
                | "openat"
                | "fopen"
                | "fopen64"
                | "freopen"
                | "read"
                | "write"
                | "pread"
                | "pwrite"
                | "socket"
                | "connect"
                | "bind"
                | "send"
                | "recv"
                | "syscall"
                | "system"
                | "execve"
                | "dlopen"
                | "dlsym"
                | "getenv"
                | "putenv"
                | "setenv"
                | "clock_gettime"
                | "gettimeofday"
                | "time"
                | "nanosleep"
                | "ioctl"
                | "mmap"
                | "mprotect"
                | "CreateFileA"
                | "CreateFileW"
                | "ReadFile"
                | "WriteFile"
                | "LoadLibraryA"
                | "LoadLibraryW"
                | "GetProcAddress"
                | "VirtualAlloc"
                | "VirtualProtect"
                | "XInputGetState"
        )
}

// Closed compiler ABI vocabulary for already source-checked Rust collections,
// Unicode, and implicit bounds/arithmetic traps. These helpers operate on
// in-memory values only. Never accept a crate or namespace prefix.
// The final builder pins panic=abort and the exact compiler/sysroot identity.
fn rust_runtime(symbol: &str) -> bool {
    let symbol = symbol
        .strip_prefix('_')
        .filter(|_| symbol.starts_with("__R") || symbol.starts_with("__ZN"))
        .unwrap_or(symbol);
    let Ok(name) = rustc_demangle::try_demangle(symbol) else {
        return false;
    };
    matches!(
        format!("{name:#}").as_str(),
        "__rustc::__rust_alloc"
            | "__rustc::__rust_dealloc"
            | "__rustc::__rust_realloc"
            | "__rustc::__rust_alloc_zeroed"
            | "__rustc::__rust_no_alloc_shim_is_unstable_v2"
            | "__rustc::__rust_alloc_error_handler"
            | "alloc::raw_vec::handle_error"
            | "alloc::alloc::handle_alloc_error"
            | "alloc::vec::Vec<T,A>::insert::assert_failed"
            | "alloc::vec::Vec<T,A>::remove::assert_failed"
            | "alloc::sync::arcinner_layout_for_value_layout"
            | "<alloc::string::String as core::clone::Clone>::clone"
            | "core::char::methods::encode_utf8_raw::do_panic::runtime"
            | "core::slice::<impl [T]>::copy_from_slice::len_mismatch_fail"
            | "core::slice::sort::shared::smallsort::panic_on_ord_violation"
            | "core::slice::sort::stable::drift::sqrt_approx"
            | "core::slice::memchr::memchr_aligned"
            | "core::slice::memchr::memrchr"
            | "core::unicode::unicode_data::white_space::WHITESPACE_MAP"
            | "core::unicode::unicode_data::cc::lookup"
            | "core::panicking::panic_bounds_check"
            | "core::slice::index::slice_end_index_len_fail"
            | "core::slice::index::slice_start_index_len_fail"
            | "core::slice::index::slice_index_order_fail"
            | "core::panicking::panic_fmt"
            | "core::panicking::panic_const::panic_const_add_overflow"
            | "core::panicking::panic_const::panic_const_sub_overflow"
            | "core::panicking::panic_const::panic_const_mul_overflow"
            | "core::panicking::panic_const::panic_const_div_by_zero"
            | "core::panicking::panic_const::panic_const_rem_by_zero"
            | "core::panicking::panic_const::panic_const_div_overflow"
            | "core::panicking::panic_const::panic_const_rem_overflow"
    )
}

// Return whether the validated object uses the i686 COFF ABI.
fn object_header(path: &Path) -> Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let mut header = [0u8; 64];
    let mut file =
        crate::files::open_regular(path, super::admission::MAX_GAME_OBJECT_BYTES, false)?;
    let length = file.read(&mut header)?;
    let elf = length >= 52
        && &header[..4] == b"\x7fELF"
        && matches!(header[4], 1 | 2)
        && header[6] == 1
        && match header[5] {
            1 => u16::from_le_bytes([header[16], header[17]]) == 1,
            2 => u16::from_be_bytes([header[16], header[17]]) == 1,
            _ => false,
        };
    let coff = length >= 20
        && matches!(
            u16::from_le_bytes([header[0], header[1]]),
            0x14c | 0x8664 | 0x1c4 | 0xaa64
        )
        && u16::from_le_bytes([header[2], header[3]]) > 0
        && header[16..18] == [0, 0];
    ensure!(elf||coff,"game audit requires individual relocatable ELF/COFF objects; archives, libraries, executables and bitcode are not permitted");
    let disallowed = |s: &str| {
        [
            ".init",
            ".fini",
            ".preinit_array",
            ".ctors",
            ".dtors",
            ".CRT",
            ".drectve",
            ".llvmbc",
            ".llvmcmd",
            ".gnu.lto",
        ]
        .iter()
        .any(|p| s.starts_with(p))
            || matches!(s, ".dynamic" | ".interp")
    };
    if coff {
        file.seek(SeekFrom::Start(20))?;
        let mut names = Vec::new();
        for _ in 0..u16::from_le_bytes([header[2], header[3]]) {
            let mut section = [0; 40];
            file.read_exact(&mut section)?;
            names.push(section);
        }
        let mut strings = Vec::new();
        for raw in names {
            let short = std::str::from_utf8(&raw[..8])?.trim_end_matches('\0');
            let name = if let Some(number) = short.strip_prefix('/') {
                if strings.is_empty() {
                    let at = u64::from(u32::from_le_bytes(header[8..12].try_into().unwrap()))
                        + u64::from(u32::from_le_bytes(header[12..16].try_into().unwrap())) * 18;
                    file.seek(SeekFrom::Start(at))?;
                    let mut size = [0; 4];
                    file.read_exact(&mut size)?;
                    let size = u32::from_le_bytes(size);
                    ensure!(
                        (4..=16 * 1024 * 1024).contains(&size)
                            && at.checked_add(u64::from(size)).is_some_and(
                                |end| end <= file.metadata().map(|m| m.len()).unwrap_or(0)
                            ),
                        "invalid COFF string table"
                    );
                    strings = vec![0; size as usize];
                    strings[..4].copy_from_slice(&size.to_le_bytes());
                    file.read_exact(&mut strings[4..])?;
                }
                let offset = number.parse::<usize>()?;
                ensure!(offset >= 4, "invalid COFF name offset");
                let name = strings
                    .get(offset..)
                    .context("COFF name outside string table")?;
                let end = name
                    .iter()
                    .take(4097)
                    .position(|b| *b == 0)
                    .context("unterminated COFF name")?;
                std::str::from_utf8(&name[..end])?
            } else {
                short
            };
            ensure!(
                !disallowed(name),
                "game object contains a linker directive or startup section: {name}"
            );
            if name == ".gpkmeta" {
                ensure!(
                    u32::from_le_bytes(raw[36..40].try_into().unwrap()) == 0x00100800
                        && raw[24..36].iter().all(|b| *b == 0),
                    "invalid inert compiler metadata flags or relocations"
                );
                let size = u32::from_le_bytes(raw[16..20].try_into().unwrap()) as usize;
                let offset = u32::from_le_bytes(raw[20..24].try_into().unwrap()) as u64;
                ensure!(size<=1024*1024 && offset>=20+40*u64::from(u16::from_le_bytes(header[2..4].try_into().unwrap())) && offset.checked_add(size as u64).is_some_and(|end|end<=file.metadata().map(|m|m.len()).unwrap_or(0)),"inert metadata outside object");
                file.seek(SeekFrom::Start(offset))?;
                let mut metadata = vec![0; size];
                file.read_exact(&mut metadata)?;
                ensure!(
                    metadata.iter().all(|b| *b == 0),
                    "inert compiler metadata contains data"
                );
            }
        }
    } else {
        let big = header[5] == 2;
        let u16_at = |b: &[u8], n| {
            let a = [b[n], b[n + 1]];
            if big {
                u16::from_be_bytes(a)
            } else {
                u16::from_le_bytes(a)
            }
        };
        let u32_at = |b: &[u8], n| {
            let a: [u8; 4] = b[n..n + 4].try_into().unwrap();
            if big {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            }
        };
        let u64_at = |b: &[u8], n| {
            let a: [u8; 8] = b[n..n + 8].try_into().unwrap();
            if big {
                u64::from_be_bytes(a)
            } else {
                u64::from_le_bytes(a)
            }
        };
        let wide = header[4] == 2;
        ensure!(!wide || length == 64, "truncated ELF64 header");
        let (offset, size, count, names) = if wide {
            (
                u64_at(&header, 40),
                u16_at(&header, 58),
                u16_at(&header, 60),
                u16_at(&header, 62),
            )
        } else {
            (
                u64::from(u32_at(&header, 32)),
                u16_at(&header, 46),
                u16_at(&header, 48),
                u16_at(&header, 50),
            )
        };
        ensure!(
            size == if wide { 64 } else { 40 } && count > 0 && names > 0 && names < count,
            "unsupported ELF section table"
        );
        ensure!(
            offset
                .checked_add(u64::from(size) * u64::from(count))
                .is_some_and(|n| n <= file.metadata().map(|m| m.len()).unwrap_or(0)),
            "ELF section table outside object"
        );
        let mut sections = Vec::new();
        file.seek(SeekFrom::Start(offset))?;
        for _ in 0..count {
            let mut s = vec![0; size as usize];
            file.read_exact(&mut s)?;
            sections.push(s);
        }
        let table = &sections[names as usize];
        let (at, len) = if wide {
            (u64_at(table, 24), u64_at(table, 32))
        } else {
            (u64::from(u32_at(table, 16)), u64::from(u32_at(table, 20)))
        };
        ensure!(
            len <= 16 * 1024 * 1024
                && at
                    .checked_add(len)
                    .is_some_and(|n| n <= file.metadata().map(|m| m.len()).unwrap_or(0)),
            "invalid ELF section names"
        );
        let mut strings = vec![0; len as usize];
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut strings)?;
        for s in sections {
            let name = strings
                .get(u32_at(&s, 0) as usize..)
                .context("invalid ELF section name")?;
            let end = name
                .iter()
                .take(4097)
                .position(|b| *b == 0)
                .context("unterminated ELF section name")?;
            let name = std::str::from_utf8(&name[..end])?;
            ensure!(
                !disallowed(name),
                "game object contains a startup or dynamic-link section: {name}"
            );
        }
    }
    Ok(coff && header[..2] == [0x4c, 0x01])
}

const RUNTIME: &[&str] = &[
    "__rust_alloc",
    "__rust_dealloc",
    "__rust_realloc",
    "__rust_alloc_zeroed",
    "__rust_no_alloc_shim_is_unstable",
    "__rust_alloc_error_handler",
    "sinf",
    "sin",
    "cosf",
    "cos",
    "sincos",
    "sincosf",
    "tanf",
    "tan",
    "asinf",
    "asin",
    "acosf",
    "acos",
    "atanf",
    "atan",
    "atan2f",
    "atan2",
    "sinhf",
    "sinh",
    "coshf",
    "cosh",
    "tanhf",
    "tanh",
    "sqrtf",
    "sqrt",
    "powf",
    "pow",
    "expf",
    "exp",
    "exp2f",
    "exp2",
    "logf",
    "log",
    "log2f",
    "log2",
    "log10f",
    "log10",
    "floorf",
    "floor",
    "ceilf",
    "ceil",
    "roundf",
    "round",
    "truncf",
    "trunc",
    "fmodf",
    "fmod",
    "copysignf",
    "copysign",
    "fmaf",
    "fma",
    "_GLOBAL_OFFSET_TABLE_",
    "memcpy",
    "memmove",
    "memset",
    "memcmp",
    "bcmp",
    "malloc",
    "calloc",
    "realloc",
    "free",
    "__stack_chk_fail",
    "__stack_chk_fail_local",
    "__stack_chk_guard",
    "__chkstk",
    "___chkstk_ms",
    "__divdi3",
    "__udivdi3",
    "__moddi3",
    "__umoddi3",
    "__divti3",
    "__udivti3",
    "__modti3",
    "__umodti3",
    "__udivmoddi4",
    "__udivmodti4",
    "__muldi3",
    "__multi3",
];

/// `definitions` must be external-linkage definitions from the exact game objects being checked, never
/// from an arbitrary extra library. All remaining imports must be the broker
/// entry points (private transport, local services and text validation) or the
/// small audited memory/compiler runtime above.
pub fn check_symbols<'a>(
    undefined: impl IntoIterator<Item = &'a str>,
    definitions: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    check_symbols_for_abi(undefined, definitions, false)
}

fn check_symbols_for_abi<'a>(
    undefined: impl IntoIterator<Item = &'a str>,
    definitions: impl IntoIterator<Item = &'a str>,
    i686_coff: bool,
) -> Result<()> {
    let definitions: BTreeSet<_> = definitions.into_iter().collect();
    for symbol in &definitions {
        ensure!(
            symbol.len() <= 4096 && !symbol.chars().any(char::is_whitespace),
            "invalid object definition symbol"
        );
        ensure!(
            !reserved(symbol)
                && !rust_runtime(symbol)
                && !RUNTIME.contains(symbol)
                && *symbol != I686_COFF_STACK_PROBE
                && !symbol
                    .strip_prefix('_')
                    .is_some_and(|s| RUNTIME.contains(&s)),
            "game object defines reserved host/runtime symbol: {symbol}"
        );
    }
    for symbol in undefined {
        ensure!(
            !symbol.is_empty() && symbol.len() <= 4096 && !symbol.chars().any(char::is_whitespace),
            "invalid object symbol"
        );
        ensure!(
            matches!(
                symbol,
                "gamepkg_port_call"
                    | "_gamepkg_port_call"
                    | "gamepkg_local_call"
                    | "_gamepkg_local_call"
                    | "gamepkg_port_utf8_validate"
                    | "_gamepkg_port_utf8_validate"
            ) || !reserved(symbol),
            "game object calls a system/host-private function: {symbol}"
        );
        if definitions.contains(symbol)
            || symbol == "gamepkg_port_call"
            || symbol == "gamepkg_local_call"
            || symbol == "gamepkg_port_utf8_validate"
            // The MinGW libgcc stack probe is an alias of its i686 chkstk.
            || (i686_coff && symbol == I686_COFF_STACK_PROBE)
            || RUNTIME.contains(&symbol)
            || rust_runtime(symbol)
        {
            continue;
        }
        // i686 COFF C symbols have one leading underscore. Import-table thunk
        // names (__imp_...) and stdcall suffixes are deliberately not accepted.
        if let Some(c) = symbol.strip_prefix('_') {
            if matches!(
                c,
                "gamepkg_port_call" | "gamepkg_local_call" | "gamepkg_port_utf8_validate"
            ) || RUNTIME.contains(&c)
            {
                continue;
            }
        }
        bail!("game object imports unapproved function: {symbol}");
    }
    Ok(())
}

/// Inspect actual object files with a host-selected pinned LLVM nm. Use the
/// complete set of game objects; host/backend objects are linked only after
/// this check. No shell, plugins, demangling or game-selected tool is used.
pub fn objects(nm: &Path, objects: &[std::path::PathBuf]) -> Result<()> {
    ensure!(
        !objects.is_empty() && objects.len() <= 4096,
        "invalid game object count"
    );
    let mut undefined = Vec::new();
    let mut defined = Vec::new();
    let mut i686_coff = true;
    for object in objects {
        i686_coff &= object_header(object)?;
        for (flag, output) in [
            ("--undefined-only", &mut undefined),
            ("--defined-only", &mut defined),
        ] {
            let mut command = std::process::Command::new(nm);
            command
                .args([
                    flag,
                    "--extern-only",
                    "--format=just-symbols",
                    "--no-demangle",
                ])
                .arg(object.canonicalize()?);
            let result = crate::compiler::run_timed(&mut command, Duration::from_secs(30))?;
            ensure!(
                result.status.success(),
                "cannot inspect game object: {}",
                result.stderr
            );
            ensure!(
                result.stdout_bytes.len() < crate::compiler::DIAGNOSTICS
                    && result.stderr.is_empty(),
                "object audit output is incomplete or contains tool diagnostics"
            );
            let text = std::str::from_utf8(&result.stdout_bytes)?;
            output.extend(text.lines().filter(|s| !s.is_empty()).map(str::to_owned));
        }
    }
    check_symbols_for_abi(
        undefined.iter().map(String::as_str),
        defined.iter().map(String::as_str),
        i686_coff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn i686_coff_stack_probe_is_only_allowed_as_an_exact_import() {
        check_symbols_for_abi([I686_COFF_STACK_PROBE], [], true).unwrap();
        assert!(check_symbols([I686_COFF_STACK_PROBE], []).is_err());
        for i686_coff in [false, true] {
            assert!(check_symbols_for_abi([], [I686_COFF_STACK_PROBE], i686_coff).is_err());
            for symbol in ["_alloca", "___alloca", "__imp___alloca", "__alloca@4"] {
                assert!(check_symbols_for_abi([symbol], [], i686_coff).is_err());
            }
        }
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn raw_directives_and_noninert_metadata_are_rejected() {
        let raw = super::super::coff_exclusions::fixture(
            " -exclude-symbols:_ZN9game_unit4test17h0123456789abcdefE",
        );
        let normalized = super::super::coff_exclusions::normalize(&raw)
            .unwrap()
            .unwrap();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), &raw).unwrap();
        assert!(object_header(file.path()).is_err());
        std::fs::write(file.path(), &normalized).unwrap();
        object_header(file.path()).unwrap();
        for (at, value) in [(101, 1), (96, 0x20), (84, 1), (80, 1)] {
            let mut bad = normalized.clone();
            bad[at] = value;
            std::fs::write(file.path(), bad).unwrap();
            assert!(object_header(file.path()).is_err(), "byte {at}");
        }
    }

    #[test]
    fn compiler_traps_are_an_exact_vocabulary_and_cannot_replace_runtime_definitions() {
        let bounds = "_ZN4core5slice5index24slice_end_index_len_fail17heb50b9bbe4663132E";
        let alloc = "_RNvCs73fAdSrgOJL_7___rustc12___rust_alloc";
        check_symbols(
            [
                bounds,
                alloc,
                "gamepkg_local_call",
                "gamepkg_port_utf8_validate",
                "_gamepkg_port_utf8_validate",
            ],
            [],
        )
        .unwrap();
        for symbol in [
            bounds,
            alloc,
            "gamepkg_local_call",
            "gamepkg_port_utf8_validate",
            "_gamepkg_port_utf8_validate",
        ] {
            assert!(check_symbols([], [symbol]).is_err());
        }
        for symbol in [
            "_ZN3std2fs4read17h0123456789abcdefE",
            "_ZN4core5slice5index24slice_end_index_len_fail_extra17h0123456789abcdefE",
            "__imp_gamepkg_local_call",
            "gamepkg_local_call@20",
            "gamepkg_port_utf8_validate_extra",
            "gamepkg_port_utf8_validate@8",
            "__imp_gamepkg_port_utf8_validate",
            "_ZN4core3str8converts9from_utf817h6773281adc79a5aeE",
            "memcpy extra",
            "SDL",
        ] {
            assert!(check_symbols([symbol], []).is_err(), "{symbol}");
        }
    }
    #[test]
    fn direct_platform_and_library_imports_are_rejected_including_thunks_and_mangling() {
        for symbol in [
            "SDL_PollEvent",
            "SDL_CreateWindow",
            "SDL_OpenAudioDevice",
            "vkCreateInstance",
            "glDrawElements",
            "XInputGetState",
            "fopen",
            "open",
            "read",
            "write",
            "socket",
            "connect",
            "CreateFileW",
            "__imp_CreateFileW",
            "_socket@12",
            "getenv",
            "system",
            "dlopen",
            "dlsym",
            "LoadLibraryW",
            "_ZN3std2fs4read",
            "gamepkg_input_connect",
            "gamepkg_host_begin_frame",
        ] {
            assert!(check_symbols([symbol], []).is_err(), "{symbol}");
        }
        check_symbols(
            [
                "gamepkg_port_call",
                "_gamepkg_port_call",
                "memcpy",
                "_memcpy",
                "__udivdi3",
                "game_local",
            ],
            ["game_local"],
        )
        .unwrap();
    }
}
