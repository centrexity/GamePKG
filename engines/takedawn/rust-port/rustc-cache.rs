//! Host compiler wrapper for resumable prepared game-unit builds. This is
//! compiled by GamePKG, outside the game source kit. No player Python is used.
use std::{
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const MAX_OBJECT: u64 = 128 * 1048576;

fn digest(bytes: &[u8]) -> io::Result<String> {
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
    let mut skip = false;
    for arg in args {
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
        if bytes.starts_with(b"\x7fELF") && digest(&bytes)?.as_bytes() == expected {
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
            if !bytes.starts_with(b"\x7fELF") {
                return Err(io::Error::other("native unit is not an ELF object"));
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
    #[test]
    fn real_object_reuse_and_input_or_cache_changes_require_compilation() {
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
        let real: OsString = "rustc".into();
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
        std::env::set_var("GAMEPKG_RUST_UNIT_ROOT", &source_root);
        std::env::set_var("GAMEPKG_RUST_UNIT_CACHE", &cache);
        std::env::set_var("GAMEPKG_RUST_UNIT_CONTEXT", "compiler-cpu-kit-a");
        assert_eq!(cached(&real, &args).unwrap(), 0);
        let expected = fs::read(&output).unwrap();
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
    let real = std::env::var_os("GAMEPKG_REAL_RUSTC").unwrap_or_else(|| "/rust/bin/rustc".into());
    match cached(&real, &args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("GamePKG: prepared Rust cache unavailable ({error}); compiling directly");
            std::process::exit(run(&real, &args).unwrap_or(1));
        }
    }
}
