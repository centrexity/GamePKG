//! Host compiler wrapper for resumable prepared game-unit builds. This is
//! compiled by GamePKG, outside the game source kit. No player Python is used.
#[cfg(not(windows))]
use std::process::Stdio;
use std::{
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
const MAX_OBJECT: u64 = 128 * 1048576;

#[cfg(not(windows))]
pub(crate) fn digest(bytes: &[u8]) -> io::Result<String> {
    let mut child = Command::new("/usr/bin/sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child.stdin.take().unwrap().write_all(bytes)?;
    let output = child.wait_with_output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let hash = text.split_whitespace().next().unwrap_or_default();
    if !output.status.success() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(io::Error::other("unable to hash native build cache"));
    }
    Ok(hash.to_owned())
}

#[cfg(windows)]
pub(crate) fn digest(bytes: &[u8]) -> io::Result<String> {
    use std::{
        ffi::c_void,
        ptr::{null, null_mut},
    };
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptOpenAlgorithmProvider(
            out: *mut *mut c_void,
            algorithm: *const u16,
            implementation: *const u16,
            flags: u32,
        ) -> i32;
        fn BCryptHash(
            algorithm: *mut c_void,
            secret: *const u8,
            secret_len: u32,
            input: *const u8,
            input_len: u32,
            output: *mut u8,
            output_len: u32,
        ) -> i32;
        fn BCryptCloseAlgorithmProvider(algorithm: *mut c_void, flags: u32) -> i32;
    }
    let input_len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::other("native cache hash input is too large"))?;
    let algorithm = [83, 72, 65, 50, 53, 54, 0]; // SHA256, UTF-16
    let mut handle = null_mut();
    // SAFETY: valid terminated algorithm name and writable output handle. The
    // matching close runs after hashing, on success or failure.
    let status = unsafe { BCryptOpenAlgorithmProvider(&mut handle, algorithm.as_ptr(), null(), 0) };
    if status != 0 {
        return Err(io::Error::other(format!(
            "unable to open SHA-256 provider ({status:#x})"
        )));
    }
    let mut output = [0u8; 32];
    // SAFETY: documented input buffer is read-only; all lengths match live
    // slices, no MAC key is supplied and output is exactly one SHA-256 digest.
    let hashed = unsafe {
        BCryptHash(
            handle,
            null(),
            0,
            bytes.as_ptr(),
            input_len,
            output.as_mut_ptr(),
            32,
        )
    };
    let closed = unsafe { BCryptCloseAlgorithmProvider(handle, 0) };
    if hashed != 0 || closed != 0 {
        return Err(io::Error::other(format!(
            "native SHA-256 failed ({hashed:#x}, {closed:#x})"
        )));
    }
    Ok(output.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn target_argument(args: &[OsString]) -> Option<&str> {
    args.iter().enumerate().find_map(|(i, arg)| {
        if arg == "--target" {
            args.get(i + 1)?.to_str()
        } else {
            arg.to_str()?.strip_prefix("--target=")
        }
    })
}
fn object_valid(bytes: &[u8], args: &[OsString]) -> bool {
    let target = target_argument(args);
    let windows = target.map_or(cfg!(windows), |name| name.contains("-windows-"));
    let machine = target.and_then(|name| {
        if name.starts_with("x86_64-") {
            Some(if windows { 0x8664 } else { 62 })
        } else if name.starts_with("i686-") || name.starts_with("i586-") {
            Some(if windows { 0x014c } else { 3 })
        } else {
            None
        }
    });
    if target.is_some() && machine.is_none() {
        return false;
    }
    let u16_at = |offset| {
        bytes
            .get(offset..offset + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    if !windows {
        let class = bytes.get(4).copied();
        return bytes.starts_with(b"\x7fELF")
            && bytes.get(5) == Some(&1)
            && u16_at(16) == Some(1)
            && matches!(
                (class, u16_at(18)),
                (Some(1), Some(3)) | (Some(2), Some(62))
            )
            && bytes.len() >= if class == Some(1) { 52 } else { 64 }
            && machine.is_none_or(|m| u16_at(18) == Some(m));
    }
    let (kind, sections, header) = if bytes.starts_with(&[0, 0, 255, 255]) {
        // LLVM COFF BigObj, not a short import-library object. The class id
        // and fixed header layout are defined in LLVM BinaryFormat/COFF.h.
        const CLASS_ID: &[u8] = &[
            0xc7, 0xa1, 0xba, 0xd1, 0xee, 0xba, 0xa9, 0x4b, 0xaf, 0x20, 0xfa, 0xf6, 0x6a, 0xa4,
            0xdc, 0xb8,
        ];
        if bytes.len() < 56
            || u16_at(4).is_none_or(|v| v < 2)
            || bytes.get(12..28) != Some(CLASS_ID)
        {
            return false;
        }
        (
            u16_at(6),
            u32::from_le_bytes(bytes[44..48].try_into().unwrap()) as u64,
            56u64,
        )
    } else {
        if bytes.len() < 20 || u16_at(16) != Some(0) {
            return false;
        }
        (u16_at(0), u64::from(u16_at(2).unwrap()), 20u64)
    };
    matches!(kind, Some(0x8664 | 0x014c))
        && machine.is_none_or(|m| kind == Some(m))
        && sections > 0
        && header + sections * 40 <= bytes.len() as u64
}

fn regular(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_OBJECT {
        return Err(io::Error::other("invalid native cache file"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_OBJECT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_OBJECT {
        return Err(io::Error::other("native cache file exceeded its bound"));
    }
    Ok(bytes)
}

fn key(args: &[OsString], source: &Path, context: &str) -> io::Result<String> {
    let mut data = b"GamePKG prepared Rust object v1\0".to_vec();
    data.extend_from_slice(context.as_bytes());
    data.push(0);
    data.extend_from_slice(digest(&regular(source)?)?.as_bytes());
    data.push(0);
    // A native Windows build has a fresh private source directory each time.
    // Normalize it only when rustc is explicitly remapping that exact directory
    // to our fixed virtual source root, including file!()/panic locations.
    let mut remap = OsString::from("--remap-path-prefix=");
    remap.push(
        source
            .parent()
            .ok_or_else(|| io::Error::other("invalid unit source"))?,
    );
    remap.push("=/gamepkg/prepared/src");
    let remapped = args.iter().any(|arg| arg == &remap);
    let mut stable_source = OsString::from("/gamepkg/prepared/src/");
    stable_source.push(
        source
            .file_name()
            .ok_or_else(|| io::Error::other("invalid unit name"))?,
    );
    let stable_remap =
        OsString::from("--remap-path-prefix=/gamepkg/prepared/src=/gamepkg/prepared/src");
    let mut skip = false;
    for (index, original) in args.iter().enumerate() {
        let arg = if remapped && index == 0 && original == source.as_os_str() {
            &stable_source
        } else if remapped && original == &remap {
            &stable_remap
        } else {
            original
        };
        if skip {
            skip = false;
            continue;
        }
        if arg == "-o" {
            skip = true;
            continue;
        }
        data.extend_from_slice(arg.as_encoded_bytes());
        data.push(0);
    }
    digest(&data)
}

fn unit(args: &[OsString], source_root: &Path) -> Option<(PathBuf, PathBuf)> {
    if !args.iter().any(|arg| arg == "--emit=obj") {
        return None;
    }
    let source = Path::new(args.first()?);
    if source.parent()? != source_root || source.extension()? != "rs" {
        return None;
    }
    let output = args.windows(2).find(|pair| pair[0] == "-o")?;
    Some((source.to_owned(), PathBuf::from(&output[1])))
}

fn atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension(format!("partial-{}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

fn run(real: &OsString, args: &[OsString]) -> io::Result<i32> {
    Ok(Command::new(real).args(args).status()?.code().unwrap_or(1))
}

fn cached(real: &OsString, args: &[OsString]) -> io::Result<i32> {
    let source_root = std::env::var_os("GAMEPKG_RUST_UNIT_ROOT").map(PathBuf::from);
    let cache = std::env::var_os("GAMEPKG_RUST_UNIT_CACHE").map(PathBuf::from);
    let context = std::env::var("GAMEPKG_RUST_UNIT_CONTEXT");
    let (Some(source_root), Some(cache), Ok(context)) = (source_root, cache, context) else {
        return run(real, args);
    };
    let Some((source, output)) = unit(args, &source_root) else {
        return run(real, args);
    };
    let key = key(args, &source, &context)?;
    let object = cache.join(format!("{key}.o"));
    let receipt = cache.join(format!("{key}.sha256"));
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache.join(format!("{key}.lock")))?;
    lock.lock()?;
    if let (Ok(bytes), Ok(expected)) = (regular(&object), regular(&receipt)) {
        if object_valid(&bytes, args) && digest(&bytes)?.as_bytes() == expected {
            fs::write(&output, &bytes)?;
            eprintln!("GamePKG: reused prepared Rust unit {}", source.display());
            return Ok(0);
        }
    }
    let status = run(real, args)?;
    if status == 0 {
        if key != self::key(args, &source, &context)? {
            eprintln!("GamePKG: prepared Rust source changed during compilation");
            return Ok(1);
        }
        let publish = || -> io::Result<()> {
            let bytes = regular(&output)?;
            if !object_valid(&bytes, args) {
                return Err(io::Error::other(
                    "native unit is not a supported target object",
                ));
            }
            atomic(&object, &bytes)?;
            atomic(&receipt, digest(&bytes)?.as_bytes())
        };
        if let Err(error) = publish() {
            // Cache failure must not repeat an already successful long compile.
            eprintln!("GamePKG: unit compiled; cache checkpoint unavailable: {error}");
        }
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    static ENVIRONMENT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn real_compiler() -> OsString {
        std::env::var_os("GAMEPKG_CACHE_TEST_RUSTC").unwrap_or_else(|| "rustc".into())
    }
    fn target(args: &mut Vec<OsString>) {
        if let Some(target) = std::env::var_os("GAMEPKG_CACHE_TEST_TARGET") {
            args.extend(["--target".into(), target]);
        }
    }
    #[test]
    fn hashes_match_standard_sha256_vectors() {
        assert_eq!(
            digest(b"").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            digest(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            digest(&vec![b'a'; 1_000_000]).unwrap(),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }
    #[test]
    fn explicit_source_remapping_reuses_across_private_build_directories() {
        let _guard = ENVIRONMENT.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "gamepkg-cache-remap-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("cache")).unwrap();
        let mut reference = None;
        for directory in ["first private build", "second private build"] {
            let source_root = root.join(directory);
            fs::create_dir(&source_root).unwrap();
            let source = source_root.join("tiny.rs");
            fs::write(
                &source,
                "#[no_mangle] pub extern \"C\" fn source_path_len() -> usize { file!().len() }",
            )
            .unwrap();
            let output = source_root.join("tiny.o");
            let mut remap = OsString::from("--remap-path-prefix=");
            remap.push(&source_root);
            remap.push("=/gamepkg/prepared/src");
            let mut args = vec![
                source.clone().into_os_string(),
                "--crate-type=lib".into(),
                "--emit=obj".into(),
                remap.clone(),
                "-o".into(),
                output.clone().into_os_string(),
            ];
            target(&mut args);
            std::env::set_var("GAMEPKG_RUST_UNIT_ROOT", &source_root);
            std::env::set_var("GAMEPKG_RUST_UNIT_CACHE", root.join("cache"));
            std::env::set_var("GAMEPKG_RUST_UNIT_CONTEXT", "explicit-remap-context");
            let compiler = if reference.is_none() {
                real_compiler()
            } else {
                root.join("unavailable.exe").into_os_string()
            };
            assert_eq!(cached(&compiler, &args).unwrap(), 0);
            let bytes = fs::read(&output).unwrap();
            assert!(object_valid(&bytes, &args));
            let current = key(&args, &source, "explicit-remap-context").unwrap();
            if let Some((expected_key, expected_bytes)) = &reference {
                assert_eq!(&current, expected_key);
                assert_eq!(&bytes, expected_bytes);
            } else {
                reference = Some((current.clone(), bytes));
            }
            // Without the matching compiler remap, the path stays in the key.
            args.retain(|arg| arg != &remap);
            assert_ne!(
                key(&args, &source, "explicit-remap-context").unwrap(),
                current
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "requires an actual MSVC /bigobj compiler fixture"]
    fn native_bigobj_is_a_relocatable_object_not_a_short_import() {
        let bytes = fs::read(
            std::env::var_os("GAMEPKG_CACHE_TEST_BIGOBJ").expect("compiled BigObj fixture"),
        )
        .unwrap();
        assert!(bytes.starts_with(&[0, 0, 255, 255]));
        let args = vec![
            "--target".into(),
            std::env::var_os("GAMEPKG_CACHE_TEST_TARGET").unwrap(),
        ];
        assert!(object_valid(&bytes, &args));
        let mut import = bytes.clone();
        import[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert!(!object_valid(&import, &args));
    }

    #[test]
    fn real_object_reuse_and_input_or_cache_changes_require_compilation() {
        let _guard = ENVIRONMENT.lock().unwrap();
        let root = std::env::temp_dir().join(format!(
            "gamepkg-rustc-cache-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source_root = root.join("source");
        let cache = root.join("cache");
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir(&cache).unwrap();
        let source = source_root.join("tiny.rs");
        let output = root.join("tiny.o");
        fs::write(
            &source,
            "#[no_mangle] pub extern \"C\" fn game_value() -> u64 { 7 }",
        )
        .unwrap();
        let real = real_compiler();
        let unavailable: OsString = root.join("missing-compiler").into();
        let mut args = vec![
            source.clone().into_os_string(),
            "--edition=2021".into(),
            "--crate-type=lib".into(),
            "--emit=obj".into(),
            "-C".into(),
            "opt-level=3".into(),
            "-o".into(),
            output.clone().into_os_string(),
        ];
        target(&mut args);
        std::env::set_var("GAMEPKG_RUST_UNIT_ROOT", &source_root);
        std::env::set_var("GAMEPKG_RUST_UNIT_CACHE", &cache);
        std::env::set_var("GAMEPKG_RUST_UNIT_CONTEXT", "compiler-cpu-kit-a");
        assert_eq!(cached(&real, &args).unwrap(), 0);
        let expected = fs::read(&output).unwrap();
        assert!(object_valid(&expected, &args));
        let windows = !expected.starts_with(b"\x7fELF");
        let kind = if windows {
            let at = if expected.starts_with(&[0, 0, 255, 255]) {
                6
            } else {
                0
            };
            u16::from_le_bytes(expected[at..at + 2].try_into().unwrap())
        } else {
            u16::from_le_bytes(expected[18..20].try_into().unwrap())
        };
        let is_64 = matches!(kind, 0x8664 | 62);
        let wrong = if windows {
            if is_64 {
                "i686-pc-windows-gnu"
            } else {
                "x86_64-pc-windows-gnu"
            }
        } else if is_64 {
            "i686-unknown-linux-gnu"
        } else {
            "x86_64-unknown-linux-gnu"
        };
        assert!(!object_valid(&expected, &["--target".into(), wrong.into()]));
        let wrong_os = if windows {
            "x86_64-unknown-linux-gnu"
        } else {
            "x86_64-pc-windows-gnu"
        };
        assert!(!object_valid(
            &expected,
            &["--target".into(), wrong_os.into()]
        ));

        fs::remove_file(&output).unwrap();
        assert_eq!(cached(&unavailable, &args).unwrap(), 0);
        assert_eq!(fs::read(&output).unwrap(), expected);
        let first = key(&args, &source, "compiler-cpu-kit-a").unwrap();
        fs::write(cache.join(format!("{first}.o")), b"corrupted").unwrap();
        assert!(cached(&unavailable, &args).is_err());
        assert_eq!(cached(&real, &args).unwrap(), 0);
        assert_eq!(
            fs::read(cache.join(format!("{first}.o"))).unwrap(),
            expected
        );
        fs::write(cache.join(format!("{first}.sha256")), b"incorrect receipt").unwrap();
        assert!(cached(&unavailable, &args).is_err());
        assert_eq!(cached(&real, &args).unwrap(), 0);
        args[5] = "opt-level=0".into();
        assert!(cached(&unavailable, &args).is_err());
        assert_eq!(cached(&real, &args).unwrap(), 0);
        fs::write(
            &source,
            "#[no_mangle] pub extern \"C\" fn game_value() -> u64 { 8 }",
        )
        .unwrap();
        assert!(cached(&unavailable, &args).is_err());
        assert_eq!(cached(&real, &args).unwrap(), 0);
        std::env::set_var("GAMEPKG_RUST_UNIT_CONTEXT", "compiler-cpu-kit-b");
        assert!(cached(&unavailable, &args).is_err());
        assert_eq!(cached(&real, &args).unwrap(), 0);
        let count = fs::read_dir(&cache)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "o"))
            .count();
        fs::write(&source, "this is invalid Rust").unwrap();
        assert_ne!(cached(&real, &args).unwrap(), 0);
        assert_eq!(
            fs::read_dir(&cache)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "o"))
                .count(),
            count
        );
        assert!(unit(&["--version".into()], &source_root).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let real = std::env::var_os("GAMEPKG_REAL_RUSTC").unwrap_or_else(|| {
        if cfg!(windows) {
            "rustc.exe".into()
        } else {
            "/rust/bin/rustc".into()
        }
    });
    match cached(&real, &args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("GamePKG: prepared Rust cache unavailable ({error}); compiling directly");
            std::process::exit(run(&real, &args).unwrap_or(1));
        }
    }
}
