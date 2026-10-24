//! Host-owned native Windows prepared-source builder. The caller must admit
//! the fixed source kit and verified SDK before invoking this internal tool.
//! No translation, ROM access, Cargo game dependency or Python is involved.
#[cfg(windows)]
#[allow(dead_code)]
#[path = "rustc-cache.rs"]
mod checkpoint;

#[cfg(windows)]
fn build() -> std::io::Result<()> {
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{OsStr, OsString},
        fs, io,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        time::Instant,
    };
    const TARGET: &str = "x86_64-pc-windows-gnu";
    fn fail(message: &str) -> io::Error {
        io::Error::other(message)
    }
    fn names(path: &Path) -> io::Result<Vec<String>> {
        let text = fs::read_to_string(path)?;
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for name in text.lines() {
            if name.is_empty()
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !seen.insert(name.to_owned())
            {
                return Err(fail("invalid or duplicate prepared identifier"));
            }
            out.push(name.to_owned());
        }
        if out.is_empty() {
            return Err(fail("empty prepared inventory"));
        }
        Ok(out)
    }
    fn run(command: &mut Command) -> io::Result<()> {
        let status = command.stdin(Stdio::null()).status()?;
        if !status.success() {
            return Err(fail(&format!("native build command failed ({status})")));
        }
        Ok(())
    }
    fn capture(command: &mut Command) -> io::Result<Vec<u8>> {
        let output = command.stdin(Stdio::null()).output()?;
        if !output.status.success() {
            return Err(fail(&String::from_utf8_lossy(&output.stderr)));
        }
        Ok(output.stdout)
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut options = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2 || options.insert(pair[0].clone(), pair[1].clone()).is_some() {
            return Err(fail("invalid or duplicate builder arguments"));
        }
    }
    let fields = [
        "--prepared",
        "--rustc",
        "--ar",
        "--nm",
        "--cache-wrapper",
        "--output",
        "--cache",
        "--cpu",
    ];
    if options.len() != fields.len()
        || fields
            .iter()
            .any(|key| !options.contains_key(OsStr::new(key)))
    {
        return Err(fail(
            "requires --prepared --rustc --ar --nm --cache-wrapper --output --cache --cpu",
        ));
    }
    let cpu = options.remove(OsStr::new("--cpu")).unwrap();
    if cpu != "native" && cpu != "x86-64" {
        return Err(fail("CPU must be native or x86-64"));
    }
    let cpu = cpu.to_str().unwrap();
    let path = |key: &str| -> io::Result<PathBuf> {
        let path = PathBuf::from(options.get(OsStr::new(key)).unwrap());
        if !path.is_absolute() {
            return Err(fail("native build paths must be absolute"));
        }
        Ok(path)
    };
    let prepared = path("--prepared")?;
    let rustc = path("--rustc")?;
    let ar = path("--ar")?;
    let nm = path("--nm")?;
    let wrapper = path("--cache-wrapper")?;
    let output = path("--output")?;
    let cache = path("--cache")?;
    fs::create_dir_all(&output)?;
    fs::create_dir_all(&cache)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(output.join("build.lock"))?;
    lock.lock()?;
    match fs::remove_file(output.join("qualification.json")) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let units = names(&prepared.join("units.txt"))?;
    let exports = names(&prepared.join("exports.txt"))?;
    let unit_inventory = fs::read(prepared.join("units.txt"))?;
    let export_inventory = fs::read(prepared.join("exports.txt"))?;
    let lib = prepared.join("src/lib.rs");
    let lib_bytes = fs::read(&lib)?;
    let version = capture(Command::new(&rustc).args(["--version", "--verbose"]))?;
    let cfg = capture(Command::new(&rustc).args([
        "--target",
        TARGET,
        "-C",
        &format!("target-cpu={cpu}"),
        "--print",
        "cfg",
    ]))?;
    let mut identity = b"GamePKG native Windows prepared builder v1\0".to_vec();
    for bytes in [
        &version[..],
        &cfg[..],
        cpu.as_bytes(),
        export_inventory.as_slice(),
        fs::read(&wrapper)?.as_slice(),
    ] {
        identity.extend_from_slice(bytes);
        identity.push(0);
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // CPUID model/vendor/brand distinguish CPUs with the same feature set;
        // no per-core APIC id is included. rustc's cfg captures enabled features.
        use std::arch::x86_64::__cpuid;
        for leaf in [0, 0x80000000, 0x80000002, 0x80000003, 0x80000004] {
            if leaf >= 0x80000002 && __cpuid(0x80000000).eax < leaf {
                continue;
            }
            let x = __cpuid(leaf);
            for value in [x.eax, x.ebx, x.ecx, x.edx] {
                identity.extend_from_slice(&value.to_le_bytes());
            }
        }
        identity.extend_from_slice(&__cpuid(1).eax.to_le_bytes());
    }
    let context = checkpoint::digest(&identity)?;
    fs::write(output.join("compiler-version.txt"), &version)?;
    fs::write(output.join("compiler-cfg.txt"), &cfg)?;
    let source_root = prepared.join("src");
    let mut remap = OsString::from("--remap-path-prefix=");
    remap.push(&source_root);
    remap.push("=/gamepkg/prepared/src");
    let began = Instant::now();
    let mut objects = Vec::new();
    for (index, name) in units.iter().enumerate() {
        println!(
            "GamePKG: preparing native Windows Rust unit {}/{}: {name}",
            index + 1,
            units.len()
        );
        let source = source_root.join(format!("{name}.rs"));
        let object = output.join(format!("{name}.o"));
        run(Command::new(&wrapper)
            .arg(source)
            .args([
                "--edition=2021",
                "--crate-type=lib",
                "--emit=obj",
                "-Awarnings",
                "--target",
                TARGET,
                "-C",
                "opt-level=3",
                "-C",
                &format!("target-cpu={cpu}"),
                "-C",
                "panic=abort",
                "-C",
                "overflow-checks=off",
            ])
            .arg(&remap)
            .arg("-o")
            .arg(&object)
            .env("GAMEPKG_REAL_RUSTC", &rustc)
            .env("GAMEPKG_RUST_UNIT_ROOT", &source_root)
            .env("GAMEPKG_RUST_UNIT_CACHE", &cache)
            .env("GAMEPKG_RUST_UNIT_CONTEXT", &context))?;
        objects.push(object);
    }
    let archive = output.join("libgamepkg_takedawn_units.a");
    if archive.exists() {
        fs::remove_file(&archive)?;
    }
    for chunk in objects.chunks(32) {
        run(Command::new(&ar).arg("q").arg(&archive).args(chunk))?;
    }
    run(Command::new(&ar).arg("s").arg(&archive))?;
    let full = output.join("libgamepkg_takedawn_generated_full_windows.a");
    let mut search = OsString::from("native=");
    search.push(&output);
    run(Command::new(&rustc)
        .arg(&lib)
        .args([
            "--edition=2021",
            "--crate-type=staticlib",
            "--crate-name",
            "gamepkg_takedawn_generated",
            "--target",
            TARGET,
            "-C",
            "opt-level=3",
            "-C",
            &format!("target-cpu={cpu}"),
            "-C",
            "panic=abort",
            "-C",
            "overflow-checks=off",
            "-L",
        ])
        .arg(search)
        .args(["-l", "static=gamepkg_takedawn_units", "-o"])
        .arg(&full))?;
    let symbols = capture(Command::new(&nm).arg("--defined-only").arg(&full))?;
    let symbols = String::from_utf8_lossy(&symbols);
    let defined: BTreeSet<_> = symbols
        .lines()
        .filter_map(|line| {
            let parts: Vec<_> = line.split_whitespace().collect();
            (parts.len() >= 3).then(|| parts[parts.len() - 1])
        })
        .collect();
    for name in &exports {
        if !defined.contains(name.as_str()) {
            return Err(fail(&format!("native archive is missing {name}")));
        }
    }
    if unit_inventory != fs::read(prepared.join("units.txt"))?
        || export_inventory != fs::read(prepared.join("exports.txt"))?
        || lib_bytes != fs::read(lib)?
    {
        return Err(fail("prepared inventory changed during compilation"));
    }
    let hash = checkpoint::digest(&fs::read(&full)?)?;
    let record = format!("{{\n  \"format\":\"gamepkg-windows-prepared-build-v1\",\n  \"target\":\"{TARGET}\",\n  \"target_cpu\":\"{cpu}\",\n  \"context\":\"{context}\",\n  \"units\":{},\n  \"retained_exports\":{},\n  \"archive_sha256\":\"{hash}\",\n  \"elapsed_seconds\":{}\n}}\n",units.len(),exports.len(),began.elapsed().as_secs());
    fs::write(output.join("qualification.json"), record)?;
    println!(
        "GamePKG: native Windows Rust archive complete: {} units, {} retained exports ({hash})",
        units.len(),
        exports.len()
    );
    Ok(())
}
#[cfg(windows)]
fn main() {
    if let Err(error) = build() {
        eprintln!("GamePKG: native prepared build failed: {error}");
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This internal builder runs on Windows.");
    std::process::exit(2);
}
