//! Host-owned native Windows runtime builder. The caller admits the fixed source
//! kit, SDK and prepared game archive before invoking this internal tool.
//! No game execution, ROM access, translation, Python or WSL is involved.
#[cfg(windows)]
#[allow(dead_code)]
#[path = "rustc-cache.rs"]
mod checkpoint;

#[cfg(any(windows, test))]
fn cmake_path(path: &std::path::Path) -> std::io::Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| std::io::Error::other("CMake path is not Unicode"))?;
    if text.contains([';', '\n', '\r']) {
        return Err(std::io::Error::other(
            "CMake path contains a list separator or newline",
        ));
    }
    // canonicalize() returns verbatim Windows paths. CMake needs ordinary
    // drive/UNC paths rather than //?/C:/... or //?/UNC/server/share/....
    let text = if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        text.strip_prefix("\\\\?\\")
            .unwrap_or(text)
            .replace('\\', "/")
    };
    Ok(text)
}

#[cfg(any(windows, test))]
fn configure_command(
    sdk: &std::path::Path,
    source: &std::path::Path,
    build: &std::path::Path,
    cpu: &str,
) -> std::io::Result<std::process::Command> {
    let llvm = sdk.join("llvm-mingw-windows/bin");
    // Keep the executable's extended path for deeply nested SDKs. CMake source
    // and build arguments must use normal paths: its generated include paths
    // contain `..`, which Windows verbatim paths deliberately do not resolve.
    let mut command = std::process::Command::new(sdk.join("cmake-windows/bin/cmake.exe"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .arg("-S")
        .arg(cmake_path(source)?)
        .arg("-B")
        .arg(cmake_path(build)?)
        .args(["-G", "Ninja"])
        .arg(format!(
            "-DCMAKE_MAKE_PROGRAM={}",
            cmake_path(&sdk.join("ninja-windows/ninja.exe"))?
        ))
        .arg(format!(
            "-DCMAKE_C_COMPILER={}",
            cmake_path(&llvm.join("x86_64-w64-mingw32-clang.exe"))?
        ))
        .arg(format!(
            "-DCMAKE_CXX_COMPILER={}",
            cmake_path(&llvm.join("x86_64-w64-mingw32-clang++.exe"))?
        ))
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg(format!("-DCMAKE_C_FLAGS=-march={cpu}"))
        .arg(format!("-DCMAKE_CXX_FLAGS=-march={cpu}"));
    Ok(command)
}

#[cfg(any(windows, test))]
fn shader_configuration(
    sdk: &std::path::Path,
    build: &std::path::Path,
    install: &std::path::Path,
    cpu: &str,
) -> std::io::Result<std::process::Command> {
    let mut command = configure_command(sdk, &sdk.join("glslang-src"), build, cpu)?;
    command
        .arg(format!("-DCMAKE_INSTALL_PREFIX={}", cmake_path(install)?))
        .args([
            "-DBUILD_SHARED_LIBS=OFF",
            "-DENABLE_OPT=OFF",
            "-DENABLE_GLSLANG_BINARIES=OFF",
            "-DGLSLANG_TESTS=OFF",
            "-DBUILD_EXTERNAL=OFF",
        ]);
    Ok(command)
}

#[cfg(any(windows, test))]
fn admitted_commands(text: &str, roots: &[&std::path::Path]) -> std::io::Result<bool> {
    let mut text = text.replace('\\', "/").to_ascii_lowercase();
    let mut roots = roots
        .iter()
        .map(|p| cmake_path(p).map(|s| s.to_ascii_lowercase()))
        .collect::<std::io::Result<Vec<_>>>()?;
    roots.sort_by_key(|s| std::cmp::Reverse(s.len()));
    for root in roots {
        text = text.replace(&root, "/gamepkg-admitted-root");
    }
    Ok(!text.contains("python")
        && !text
            .lines()
            .any(|line| line.contains("recomp/gen/recomp_") && line.contains(".c")))
}

#[cfg(windows)]
fn build() -> std::io::Result<()> {
    use std::{
        collections::{BTreeMap, BTreeSet},
        ffi::{OsStr, OsString},
        fs, io,
        path::{Path, PathBuf},
        process::{Command, Stdio},
    };
    fn error(message: &str) -> io::Error {
        io::Error::other(message)
    }
    if std::env::consts::ARCH != "x86_64" {
        return Err(error(
            "the reviewed native Windows runtime currently requires x86-64",
        ));
    }
    fn run(command: &mut Command) -> io::Result<()> {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let status = command.stdin(Stdio::null()).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(error(&format!(
                "native runtime build command failed ({status})"
            )))
        }
    }
    fn capture(command: &mut Command) -> io::Result<Vec<u8>> {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        let result = command.stdin(Stdio::null()).output()?;
        if !result.status.success() {
            return Err(error(&String::from_utf8_lossy(&result.stderr)));
        }
        Ok(result.stdout)
    }
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let mut options = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2 || options.insert(pair[0].clone(), pair[1].clone()).is_some() {
            return Err(error("invalid or duplicate runtime builder arguments"));
        }
    }
    let fields = [
        "--kit",
        "--sdk",
        "--game-archive",
        "--output",
        "--cpu",
        "--jobs",
    ];
    if options.len() != fields.len()
        || fields
            .iter()
            .any(|key| !options.contains_key(OsStr::new(key)))
    {
        return Err(error(
            "requires --kit --sdk --game-archive --output --cpu --jobs",
        ));
    }
    let cpu = options
        .get(OsStr::new("--cpu"))
        .unwrap()
        .to_str()
        .ok_or_else(|| error("invalid CPU"))?;
    if !matches!(cpu, "native" | "x86-64") {
        return Err(error("CPU must be native or x86-64"));
    }
    let jobs: usize = options
        .get(OsStr::new("--jobs"))
        .unwrap()
        .to_str()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .ok_or_else(|| error("jobs must be a positive integer"))?;
    let path = |key: &str| -> io::Result<PathBuf> {
        let value = PathBuf::from(options.get(OsStr::new(key)).unwrap());
        if !value.is_absolute() {
            return Err(error("runtime build paths must be absolute"));
        }
        cmake_path(&value)?;
        Ok(value)
    };
    let kit = path("--kit")?;
    let sdk = path("--sdk")?;
    let game = path("--game-archive")?;
    let output = path("--output")?;
    fs::create_dir_all(&output)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(output.join("runtime.lock"))?;
    lock.lock()?;
    let receipt = output.join("runtime-qualification.json");
    match fs::remove_file(&receipt) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let kit_manifest = fs::read(kit.join("kit.json"))?;
    let game_hash = checkpoint::digest(&fs::read(&game)?)?;
    let llvm = sdk.join("llvm-mingw-windows/bin");
    let rust = sdk.join("rust-1.89.0-windows-gnu");
    let rustc = rust.join("bin/rustc.exe");
    let cmake = sdk.join("cmake-windows/bin/cmake.exe");
    let ninja = sdk.join("ninja-windows/ninja.exe");
    let cc = llvm.join("x86_64-w64-mingw32-clang.exe");
    let linker = rust
        .join("lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe");
    // Children see only the verified SDK and Windows system commands. In
    // particular no installed Python, rustup, MSVC or WSL participates.
    let system = PathBuf::from(
        std::env::var_os("SystemRoot").ok_or_else(|| error("missing Windows system root"))?,
    )
    .join("System32");
    let command_path = std::env::join_paths([
        rust.join("bin"),
        llvm.clone(),
        ninja.parent().unwrap().to_owned(),
        cmake.parent().unwrap().to_owned(),
        system,
    ])
    .map_err(|_| error("invalid SDK search path"))?;
    let command = |exe: &Path| {
        let mut c = Command::new(exe);
        c.env("PATH", &command_path);
        c
    };
    let mut versions = Vec::new();
    for (exe, flags) in [
        (&rustc, vec!["--version", "--verbose"]),
        (&cc, vec!["--version"]),
        (&cmake, vec!["--version"]),
        (&ninja, vec!["--version"]),
    ] {
        versions.extend(capture(command(exe).args(flags))?);
        versions.push(b'\n');
    }
    fs::write(output.join("runtime-tool-versions.txt"), &versions)?;
    println!("GamePKG: building native Windows verified-disc reader (offline)");
    let media_dir = output.join("media");
    // Cargo splits RUSTFLAGS on spaces. Use its encoded form so SDK paths
    // containing spaces remain one linker argument.
    let mut encoded = OsString::from("-C\x1flinker=");
    encoded.push(&linker);
    encoded.push("\x1f-C\x1flink-self-contained=yes");
    // This host I/O bridge keeps the portable x64 CPU target and its full
    // release/LTO optimization. Native CPU flags here regressed audio pacing
    // in the complete runtime; both the baseline library and a native-built
    // generic-CPU library passed the unchanged original save/audio gate.
    // Prepared game code and the original C/C++ renderer retain the selected
    // native CPU target. No game logic or audio/input deadline is changed.
    encoded.push("\x1f-C\x1ftarget-cpu=x86-64");
    let mut cargo = command(&rust.join("bin/cargo.exe"));
    cargo
        .current_dir(kit.join("host/engines/takedawn/media-bridge"))
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env("CARGO_ENCODED_RUSTFLAGS", encoded)
        .env("RUSTC", &rustc)
        .env("CARGO_HOME", output.join("cargo-home"))
        .env("CARGO_TARGET_DIR", &media_dir)
        .env("CARGO_NET_OFFLINE", "true")
        .args(["build", "--offline", "--locked", "--release"]);
    run(&mut cargo)?;
    let media = media_dir.join("release/libgamepkg_takedawn_media.a");
    let glslang = output.join("glslang");
    let glslang_build = output.join("glslang-build");
    let configure = |source: &Path, build: &Path| configure_command(&sdk, source, build, cpu);
    println!("GamePKG: building native Windows shader library");
    run(&mut shader_configuration(
        &sdk,
        &glslang_build,
        &glslang,
        cpu,
    )?)?;
    run(command(&cmake)
        .arg("--build")
        .arg(cmake_path(&glslang_build)?)
        .args(["--target", "install", "--parallel", &jobs.to_string()]))?;
    // Match the original dependency recipe: derive the loader import symbols
    // from the pinned Vulkan headers. The GPU driver supplies vulkan-1.dll.
    let mut entries = BTreeSet::new();
    for name in ["vulkan_core.h", "vulkan_win32.h"] {
        let text = fs::read_to_string(sdk.join("vulkan-src/include/vulkan").join(name))?;
        for tail in text.split("VKAPI_CALL").skip(1) {
            let name: String = tail
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if name.starts_with("vk") {
                entries.insert(name);
            }
        }
    }
    if !entries.contains("vkCreateInstance") || !entries.contains("vkGetInstanceProcAddr") {
        return Err(error("Vulkan loader declarations missing"));
    }
    let imports = output.join("vulkan");
    fs::create_dir_all(&imports)?;
    let def = imports.join("vulkan-1.def");
    fs::write(
        &def,
        format!(
            "LIBRARY vulkan-1.dll\nEXPORTS\n{}\n",
            entries.into_iter().collect::<Vec<_>>().join("\n")
        ),
    )?;
    let import_library = imports.join("libvulkan-1.dll.a");
    run(command(&llvm.join("llvm-dlltool.exe"))
        .args(["-m", "i386:x86-64", "-d"])
        .arg(&def)
        .arg("-l")
        .arg(&import_library))?;
    println!("GamePKG: building original native Windows runtime and Vulkan renderer");
    let runtime = output.join("runtime");
    let bin = runtime.join("bin");
    let mut c = configure(&kit.join("game"), &runtime)?;
    for (key, value) in [
        (
            "CMAKE_TOOLCHAIN_FILE",
            kit.join("game/cmake/llvm-mingw-x86_64.cmake"),
        ),
        ("LLVM_MINGW_ROOT", sdk.join("llvm-mingw-windows")),
        (
            "CMAKE_PROJECT_TOP_LEVEL_INCLUDES",
            kit.join("host/engines/takedawn/rust-port/attach.cmake"),
        ),
        ("XBOXRECOMP_DIR", kit.join("toolkit")),
        ("Vulkan_INCLUDE_DIR", sdk.join("vulkan-src/include")),
        ("Vulkan_LIBRARY", import_library),
        (
            "SDL2_DIR",
            sdk.join("SDL2/x86_64-w64-mingw32/lib/cmake/SDL2"),
        ),
        ("glslang_DIR", glslang.join("lib/cmake/glslang")),
        ("BO3_GLYPHS_ART_DIR", kit.join("glyphs")),
        ("BO3_BIN_DIR", bin.clone()),
        ("GAMEPKG_TAKEDAWN_MEDIA_ARCHIVE", media.clone()),
        ("GAMEPKG_TAKEDAWN_RUST_ARCHIVE", game.clone()),
        (
            "GAMEPKG_TAKEDAWN_RUST_TLS",
            kit.join("prepared/native_tls.c"),
        ),
    ] {
        c.arg(format!("-D{key}={}", cmake_path(&value)?));
    }
    c.args([
        "-DBO3_OVERLAY=ON",
        "-DBO3_RELEASE=OFF",
        "-DBO3_WIN_VULKAN=ON",
        "-DBO3_ENHANCE=ON",
    ])
    .arg(format!("-DBO3_COMPILE_JOBS={jobs}"));
    run(&mut c)?;
    let commands = capture(
        command(&ninja)
            .arg("-C")
            .arg(&runtime)
            .args(["-t", "commands", "burnout3"]),
    )?;
    if !admitted_commands(&String::from_utf8_lossy(&commands), &[&sdk, &kit, &output])? {
        return Err(error("unreviewed generation or C game compilation command"));
    }
    fs::write(output.join("runtime-build-commands.txt"), commands)?;
    run(command(&cmake)
        .arg("--build")
        .arg(cmake_path(&runtime)?)
        .args(["--target", "burnout3", "--parallel", &jobs.to_string()]))?;
    let exe = bin.join("burnout3-gamepkg-rust.exe");
    let symbols = capture(
        command(&llvm.join("llvm-nm.exe"))
            .arg("--defined-only")
            .arg(&exe),
    )?;
    let actual: BTreeSet<_> = String::from_utf8_lossy(&symbols)
        .lines()
        .filter_map(|line| line.split_whitespace().last().map(str::to_owned))
        .collect();
    let exports = fs::read_to_string(kit.join("prepared/exports.txt"))?;
    let mut required = BTreeSet::new();
    for name in exports.lines() {
        if name.is_empty() || !required.insert(name) || !actual.contains(name) {
            return Err(error("missing or duplicate prepared game export"));
        }
    }
    if required.is_empty() {
        return Err(error("empty game export inventory"));
    }
    let sdl = sdk.join("SDL2/x86_64-w64-mingw32/bin/SDL2.dll");
    fs::copy(&sdl, bin.join("SDL2.dll"))?;
    if fs::read(kit.join("kit.json"))? != kit_manifest
        || checkpoint::digest(&fs::read(&game)?)? != game_hash
    {
        return Err(error("runtime build inputs changed"));
    }
    let hash = |p: &Path| checkpoint::digest(&fs::read(p)?);
    fs::write(receipt, format!("{{\"format\":\"gamepkg-native-windows-runtime-v1\",\"cpu\":\"{cpu}\",\"media_cpu\":\"x86-64\",\"retained_exports\":{},\"kit_manifest_sha256\":\"{}\",\"game_archive_sha256\":\"{game_hash}\",\"media_archive_sha256\":\"{}\",\"executable_sha256\":\"{}\",\"sdl2_sha256\":\"{}\",\"tool_versions_sha256\":\"{}\"}}\n", required.len(), checkpoint::digest(&kit_manifest)?, hash(&media)?, hash(&exe)?, hash(&sdl)?, checkpoint::digest(&versions)?))?;
    println!(
        "GamePKG: native Windows executable ready: {}",
        exe.display()
    );
    Ok(())
}

fn main() {
    #[cfg(windows)]
    if let Err(error) = build() {
        eprintln!("GamePKG: {error}");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("This internal native runtime builder requires Windows");
        std::process::exit(2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    #[test]
    fn shader_configuration_uses_normalized_roots() {
        let command = shader_configuration(
            Path::new(r"\\?\D:\SDK with spaces"),
            Path::new(r"\\?\C:\shader build"),
            Path::new(r"\\?\C:\shader install"),
            "native",
        )
        .unwrap();
        let args: Vec<_> = command.get_args().map(|a| a.to_str().unwrap()).collect();
        assert_eq!(args[1], "D:/SDK with spaces/glslang-src");
        assert!(args.contains(&"-DCMAKE_INSTALL_PREFIX=C:/shader install"));
        assert!(args
            .iter()
            .all(|a| !a.contains("//?/") && !a.contains(r"\\?\")));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "real standalone SDK gate; set GAMEPKG_WINDOWS_SDK_ROOT"]
    fn real_sdk_shader_source_and_include_paths() {
        use std::{
            fs,
            process::Command,
            time::{SystemTime, UNIX_EPOCH},
        };
        let sdk = std::path::PathBuf::from(
            std::env::var_os("GAMEPKG_WINDOWS_SDK_ROOT").expect("SDK root"),
        )
        .canonicalize()
        .unwrap();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let output = std::env::temp_dir().join(format!(
            "gamepkg shader paths {}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&output).unwrap();
        let output = output.canonicalize().unwrap();
        let build = output.join("build");
        let install = output.join("install");
        let status = shader_configuration(&sdk, &build, &install, "native")
            .unwrap()
            .status()
            .unwrap();
        assert!(
            status.success(),
            "shader configuration failed; evidence: {}",
            output.display()
        );
        let status = Command::new(sdk.join("cmake-windows/bin/cmake.exe"))
            .arg("--build")
            .arg(cmake_path(&build).unwrap())
            .args(["--target", "install", "--parallel", "2"])
            .status()
            .unwrap();
        assert!(
            status.success(),
            "shader compilation failed; evidence: {}",
            output.display()
        );
        for library in ["libglslang.a", "libSPIRV.a"] {
            assert!(
                fs::metadata(install.join("lib").join(library))
                    .unwrap()
                    .len()
                    > 0
            );
        }
        fs::remove_dir_all(output).unwrap();
    }
    #[test]
    fn configure_normalizes_source_build_and_tool_arguments() {
        for (source, build, expected_source, expected_build) in [
            (
                r"\\?\D:\Native library with spaces\glslang-src",
                r"\\?\C:\Temp\shader build",
                "D:/Native library with spaces/glslang-src",
                "C:/Temp/shader build",
            ),
            (
                r"\\?\UNC\server\share\source",
                r"\\?\UNC\server\share\build",
                "//server/share/source",
                "//server/share/build",
            ),
        ] {
            let sdk = Path::new(r"\\?\D:\Native library with spaces\sdk");
            let command =
                configure_command(sdk, Path::new(source), Path::new(build), "native").unwrap();
            let args: Vec<_> = command.get_args().map(|a| a.to_str().unwrap()).collect();
            assert_eq!(&args[..4], &["-S", expected_source, "-B", expected_build]);
            assert!(args
                .iter()
                .all(|a| !a.contains("//?/") && !a.contains(r"\\?\")));
            assert!(args
                .iter()
                .any(|a| a.starts_with("-DCMAKE_CXX_COMPILER=D:/Native library with spaces/sdk/")));
            assert_eq!(
                command.get_program(),
                sdk.join("cmake-windows/bin/cmake.exe")
            );
        }
    }
    #[test]
    fn windows_paths_and_generation_commands() {
        assert_eq!(
            cmake_path(Path::new(r"\\?\C:\Users\Python Fan\SDK")).unwrap(),
            "C:/Users/Python Fan/SDK"
        );
        assert_eq!(
            cmake_path(Path::new(r"\\?\UNC\server\share\SDK")).unwrap(),
            "//server/share/SDK"
        );
        assert!(cmake_path(Path::new("C:/bad;path")).is_err());
        let roots = [
            Path::new(r"C:\Users\Python Fan\SDK"),
            Path::new(r"C:\Users\Python Fan\kit"),
        ];
        assert!(admitted_commands(
            r#""C:\Users\Python Fan\SDK\clang.exe" -c "C:\Users\Python Fan\kit\game\renderer.c""#,
            &roots
        )
        .unwrap());
        assert!(
            !admitted_commands(r#"python "C:\Users\Python Fan\kit\generate.py""#, &roots).unwrap()
        );
        assert!(!admitted_commands(r#""C:\Users\Python Fan\SDK\clang.exe" -c "C:\Users\Python Fan\kit\game\src\game\recomp\gen\recomp_001.c""#, &roots).unwrap());
    }
}
