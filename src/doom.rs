//! Pinned DOOM engine. Private Wasm memory, no host imports or ambient I/O.
use anyhow::{bail, Context, Result};
use wasmtime::{
    Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
pub const PROFILE: &str = "doom-e257a095";
pub const MEMORY: u64 = 64 * 1048576;
pub const SAVE_MAX: usize = 4 * 1024 * 1024;
#[cfg(target_os = "linux")]
pub(crate) const MUSIC_SOURCES: &[&str] = &[
    "music_opl.c",
    "midi_memory.c",
    "music_mus.c",
    "opl_memory.c",
    "opl_synth.c",
];
#[cfg(target_os = "linux")]
pub(crate) fn verify_music_sources(root: &std::path::Path) -> Result<()> {
    let lock: serde_json::Value =
        serde_json::from_slice(include_bytes!("../engines/doom/music/upstream-lock.json"))?;
    for (name, hash) in lock["files"]
        .as_object()
        .context("invalid music source lock")?
    {
        if crate::verify::digest(&std::fs::read(
            root.join("engines/doom/music/upstream").join(name),
        )?) != hash.as_str().unwrap_or("")
        {
            bail!("pinned DOOM music source changed: {name}");
        }
    }
    Ok(())
}
#[cfg(target_os = "linux")]
pub(crate) fn verify_merge_sources(root: &std::path::Path) -> Result<()> {
    let lock: serde_json::Value =
        serde_json::from_slice(include_bytes!("../engines/doom/merge/upstream-lock.json"))?;
    for (name, hash) in lock["files"]
        .as_object()
        .context("invalid merge source lock")?
    {
        let path = root.join("engines/doom/merge/upstream").join(name);
        anyhow::ensure!(
            crate::verify::digest(&std::fs::read(path)?) == hash.as_str().unwrap_or(""),
            "pinned DOOM merge source changed: {name}"
        );
    }
    Ok(())
}
#[cfg(any(target_os = "linux", test))]
fn adapt_video(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/video-patches.json"
    ))?;
    anyhow::ensure!(
        !source.contains("#include \"i_swap.h\""),
        "DOOM video already adapted"
    );
    let mut staged = source.to_owned();
    for patch in patches.as_array().context("invalid video adaptations")? {
        let before = patch["before"].as_str().context("invalid video anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid video replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM video anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_endian(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/endian-patches.json"
    ))?;
    let mut staged = source.to_owned();
    for patch in patches.as_array().context("invalid endian adaptations")? {
        let before = patch["before"].as_str().context("invalid endian anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid endian replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM endian anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_map(source: &str) -> Result<String> {
    let patches: serde_json::Value =
        serde_json::from_slice(include_bytes!("../engines/doom/adapters/map-patches.json"))?;
    let patches = patches.as_array().context("invalid map adaptations")?;
    anyhow::ensure!(patches.len() == 15, "invalid map adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"].as_str().context("invalid map anchor")?;
        let after = patch["after"].as_str().context("invalid map replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM map anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_sprites(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/sprite-patches.json"
    ))?;
    let patches = patches.as_array().context("invalid sprite adaptations")?;
    anyhow::ensure!(patches.len() == 3, "invalid sprite adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"].as_str().context("invalid sprite anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid sprite replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM sprite anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_fixed(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/fixed-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid fixed-point adaptations")?;
    anyhow::ensure!(patches.len() == 3, "invalid fixed-point adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid fixed-point anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid fixed-point replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM fixed-point anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_sound(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/sound-patches.json"
    ))?;
    let patches = patches.as_array().context("invalid sound adaptations")?;
    anyhow::ensure!(patches.len() == 3, "invalid sound adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"].as_str().context("invalid sound anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid sound replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM sound anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_angles(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/angle-patches.json"
    ))?;
    let patches = patches.as_array().context("invalid angle adaptations")?;
    anyhow::ensure!(patches.len() == 1, "invalid angle adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"].as_str().context("invalid angle anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid angle replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM angle anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
fn adapt_music_devices(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/doom/adapters/music-device-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid music device adaptations")?;
    anyhow::ensure!(patches.len() == 1, "invalid music device adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid music device anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid music device replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned DOOM music device anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(target_os = "linux")]
pub(crate) fn stage_desktop_sources(root: &std::path::Path) -> Result<()> {
    let devices = root.join("engines/doom/upstream/i_sound.c");
    std::fs::write(
        &devices,
        adapt_music_devices(&std::fs::read_to_string(&devices)?)?,
    )?;
    let angles = root.join("engines/doom/upstream/r_main.c");
    std::fs::write(&angles, adapt_angles(&std::fs::read_to_string(&angles)?)?)?;
    let sound = root.join("engines/doom/upstream/s_sound.c");
    std::fs::write(&sound, adapt_sound(&std::fs::read_to_string(&sound)?)?)?;
    let fixed = root.join("engines/doom/upstream/m_fixed.c");
    std::fs::write(&fixed, adapt_fixed(&std::fs::read_to_string(&fixed)?)?)?;
    let sprites = root.join("engines/doom/upstream/r_data.c");
    std::fs::write(
        &sprites,
        adapt_sprites(&std::fs::read_to_string(&sprites)?)?,
    )?;
    let map = root.join("engines/doom/upstream/p_setup.c");
    std::fs::write(&map, adapt_map(&std::fs::read_to_string(&map)?)?)?;
    let endian = root.join("engines/doom/upstream/i_swap.h");
    std::fs::write(&endian, adapt_endian(&std::fs::read_to_string(&endian)?)?)?;
    let video = root.join("engines/doom/upstream/i_video.c");
    std::fs::write(&video, adapt_video(&std::fs::read_to_string(&video)?)?)?;
    let path = root.join("engines/doom/upstream/doomfeatures.h");
    let features = std::fs::read_to_string(&path)?;
    anyhow::ensure!(
        features.matches("#undef FEATURE_WAD_MERGE").count() == 1,
        "pinned WAD merge feature anchor changed"
    );
    std::fs::write(
        path,
        features.replace("#undef FEATURE_WAD_MERGE", "#define FEATURE_WAD_MERGE 1"),
    )?;
    let path = root.join("engines/doom/upstream/m_config.c");
    let mut source = std::fs::read_to_string(&path)?;
    for function in ["SaveDefaultCollection", "LoadDefaultCollection"] {
        let original =
            format!("static void {function}(default_collection_t *collection)\n{{\n#if ORIGCODE");
        anyhow::ensure!(
            source.matches(&original).count() == 1,
            "pinned configuration function changed"
        );
        source = source.replace(&original, &original.replace("#if ORIGCODE", "#if 1"));
    }
    std::fs::write(path, source)?;
    let path = root.join("engines/doom/upstream/m_controls.c");
    let mut source = std::fs::read_to_string(&path)?;
    for (old, new) in [
        ("int key_fire = KEY_FIRE;", "int key_fire = KEY_RCTRL;"),
        ("int key_use = KEY_USE;", "int key_use = ' ';"),
        (
            "int key_strafeleft = KEY_STRAFE_L;",
            "int key_strafeleft = ',';",
        ),
        (
            "int key_straferight = KEY_STRAFE_R;",
            "int key_straferight = '.';",
        ),
    ] {
        anyhow::ensure!(
            source.matches(old).count() == 1,
            "pinned control defaults changed"
        );
        source = source.replace(old, new);
    }
    std::fs::write(path, source)?;
    let path = root.join("engines/doom/upstream/i_system.c");
    let mut source = std::fs::read_to_string(&path)?;
    let begin = source
        .find("void I_Quit (void)")
        .context("pinned quit function changed")?;
    let end = source[begin..]
        .find("\n}\n")
        .context("pinned quit function end changed")?
        + begin;
    source.insert_str(end, "\n    gamepkg_request_quit();");
    source.insert_str(begin, "void gamepkg_request_quit(void);\n");
    std::fs::write(path, source)?;
    let path = root.join("engines/doom/upstream/d_main.c");
    let mut source = std::fs::read_to_string(&path)?;
    let old = "    I_Endoom(endoom);\n\n\texit(0);";
    anyhow::ensure!(
        source.matches(old).count() == 1,
        "pinned ending function changed"
    );
    source=source.replace(old,"    if(W_LumpLength(W_GetNumForName(DEH_String(\"ENDOOM\")))<4000) I_Error(\"Invalid ENDOOM resource\");\n    gamepkg_show_endoom(endoom);");
    let marker = "static void D_Endoom(void)";
    source = source.replace(
        marker,
        &format!("void gamepkg_show_endoom(const unsigned char *data);\n{marker}"),
    );
    // The upstream cast calls a boolean-returning function through a void
    // pointer. Wasm checks indirect-call signatures; preserve the callback's
    // work through a correctly typed wrapper instead.
    let old = "I_AtExit((atexit_func_t) G_CheckDemoStatus, true);";
    anyhow::ensure!(
        source.matches(old).count() == 1,
        "pinned demo exit callback changed"
    );
    source = source.replace(old, "I_AtExit(gamepkg_check_demo_status, true);");
    source = source.replace(marker, &format!("static void gamepkg_check_demo_status(void) {{ (void)G_CheckDemoStatus(); }}\n{marker}"));
    std::fs::write(path, source)?;
    let path = root.join("engines/doom/upstream/m_menu.c");
    let mut source = std::fs::read_to_string(&path)?;
    let begin = source
        .find("void M_QuitResponse(int key)")
        .context("pinned quit response changed")?;
    let end = source[begin..]
        .find("\n}\n")
        .context("pinned quit response end changed")?
        + begin;
    let body = source[begin..end]
        .replace("I_WaitVBL(105);", "")
        .replace("I_Quit ();", "gamepkg_begin_quit();");
    source.replace_range(begin..end, &body);
    source.insert_str(begin, "void gamepkg_begin_quit(void);\n");
    std::fs::write(path, source)?;
    Ok(())
}
pub fn validate_wad(wad: &[u8]) -> Result<()> {
    if wad.len() < 12 || wad.len() > 64 * 1048576 || &wad[..4] != b"IWAD" {
        bail!("select a supported DOOM IWAD (DOOM.WAD or DOOM2.WAD)");
    }
    let entries = wad_directory(wad)?;
    let mut level = false;
    for entry in entries.as_chunks::<16>().0 {
        level |= &entry[8..] == b"E1M1\0\0\0\0" || &entry[8..] == b"MAP01\0\0\0";
    }
    if !level {
        bail!("DOOM WAD has no supported starting map");
    }
    Ok(())
}
/// Validate an additional resource/map WAD without requiring an IWAD start map.
/// File order is retained by the original engine's -file handling.
pub fn validate_patch_wad(wad: &[u8]) -> Result<()> {
    if wad.len() < 12 || wad.len() > 64 * 1048576 || &wad[..4] != b"PWAD" {
        bail!("select a DOOM patch WAD (PWAD)");
    }
    wad_directory(wad)?;
    Ok(())
}
pub(crate) fn validate_patch_set(base_size: usize, patches: &[Vec<u8>]) -> Result<()> {
    anyhow::ensure!(
        patches.len() <= 6,
        "DOOM supports six patch WADs within its file-handle budget"
    );
    let mut total = base_size;
    for patch in patches {
        validate_patch_wad(patch)?;
        total = total
            .checked_add(patch.len())
            .context("DOOM WAD sizes overflow")?;
    }
    anyhow::ensure!(
        total <= MEMORY as usize,
        "combined DOOM WADs exceed the 64 MiB input boundary"
    );
    Ok(())
}
fn wad_directory(wad: &[u8]) -> Result<&[u8]> {
    let count = u32::from_le_bytes(wad[4..8].try_into()?) as usize;
    let offset = u32::from_le_bytes(wad[8..12].try_into()?) as usize;
    if count > 65536 || offset > wad.len() || count > (wad.len() - offset) / 16 {
        bail!("invalid DOOM WAD directory");
    }
    let entries = &wad[offset..offset + count * 16];
    for entry in entries.as_chunks::<16>().0 {
        let start = u32::from_le_bytes(entry[..4].try_into()?) as usize;
        let length = u32::from_le_bytes(entry[4..8].try_into()?) as usize;
        if start > wad.len() || length > wad.len() - start {
            bail!("DOOM WAD lump exceeds file bounds");
        }
    }
    Ok(entries)
}
const SOURCE_SECTION: &str = "gamepkg.doom.sources.v1";
fn source_identity() -> &'static str {
    static IDENTITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for (name, bytes) in crate::doom_sources::FILES {
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hex::encode(hash.finalize())
    })
}
#[cfg(any(target_os = "linux", test))]
fn stamp_engine(mut bytes: Vec<u8>) -> Result<Vec<u8>> {
    validate(&bytes)?;
    fn leb(mut value: usize, output: &mut Vec<u8>) {
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            output.push(byte | if value != 0 { 128 } else { 0 });
            if value == 0 {
                break;
            }
        }
    }
    let mut section = Vec::new();
    leb(SOURCE_SECTION.len(), &mut section);
    section.extend_from_slice(SOURCE_SECTION.as_bytes());
    section.extend_from_slice(source_identity().as_bytes());
    bytes.push(0);
    leb(section.len(), &mut bytes);
    bytes.extend_from_slice(&section);
    validate_current(&bytes)?;
    Ok(bytes)
}
/// Source compatibility metadata, not a replacement for build receipts or sandbox validation.
pub fn validate_current(bytes: &[u8]) -> Result<()> {
    validate(bytes)?;
    let mut found = false;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CustomSection(section) = payload? {
            if section.name() == SOURCE_SECTION {
                anyhow::ensure!(
                    !found && section.data() == source_identity().as_bytes(),
                    "rebuild the pinned DOOM engine: source kit changed"
                );
                found = true;
            }
        }
    }
    anyhow::ensure!(
        found,
        "rebuild the pinned DOOM engine: source revision missing"
    );
    Ok(())
}
pub fn validate(bytes: &[u8]) -> Result<()> {
    wasmparser::Validator::new().validate_all(bytes)?;
    let mut memories = 0;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload? {
            wasmparser::Payload::ImportSection(imports) if imports.count() != 0 => {
                bail!("DOOM engine must have zero host imports")
            }
            wasmparser::Payload::MemorySection(section) => {
                for memory in section {
                    let m = memory?;
                    memories += 1;
                    if m.memory64
                        || m.shared
                        || m.initial > 1024
                        || m.maximum.is_none_or(|max| max > 1024)
                    {
                        bail!("DOOM engine exceeds its 64 MiB boundary");
                    }
                }
            }
            _ => {}
        }
    }
    if memories != 1 {
        bail!("DOOM engine requires one bounded memory");
    }
    Ok(())
}
pub fn build() -> Result<Vec<u8>> {
    #[cfg(not(target_os = "linux"))]
    {
        bail!("DOOM compilation requires the Linux isolated compiler");
    }
    #[cfg(target_os = "linux")]
    {
        use std::{fs, path::Path};
        let kit = tempfile::tempdir()?;
        let output = tempfile::tempdir()?;
        for (name, bytes) in crate::doom_sources::FILES {
            let path = kit.path().join(name);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, bytes)?;
        }
        let lock: serde_json::Value =
            serde_json::from_slice(include_bytes!("../engines/doom/upstream-lock.json"))?;
        for (name, hash) in lock["files"]
            .as_object()
            .context("invalid DOOM source lock")?
        {
            if crate::verify::digest(&fs::read(
                kit.path().join("engines/doom/upstream").join(name),
            )?) != hash.as_str().unwrap_or("")
            {
                bail!("pinned DOOM source changed: {name}");
            }
        }
        verify_music_sources(kit.path())?;
        verify_merge_sources(kit.path())?;
        stage_desktop_sources(kit.path())?;
        let sysroot =
            crate::c_toolchain::resolve(&["GAMEPKG_DOOM_SYSROOT", "GAMEPKG_ZELDA3_SYSROOT"])?;
        let prefix = if sysroot.join("usr/include/wasm32-wasi").is_dir() {
            "/sysroot/usr"
        } else {
            "/sysroot"
        };
        let mut args = vec![
            "--target=wasm32-wasi".to_string(),
            format!("--sysroot={prefix}"),
            format!("-resource-dir={prefix}/lib/llvm-18/lib/clang/18"),
        ];
        args.extend(
            [
                "-isystem",
                "/usr/lib/llvm-18/lib/clang/18/include",
                "-O2",
                "-mbulk-memory",
                "-fno-strict-aliasing",
                "-ffunction-sections",
                "-fdata-sections",
                "-DDOOMGENERIC_RESX=320",
                "-DDOOMGENERIC_RESY=200",
                "-DFEATURE_SOUND",
                "-DFEATURE_WAD_MERGE",
                "-DNORMALUNIX",
                "-D_DEFAULT_SOURCE",
                "-I/source/engines/doom/upstream",
                "-I/source/engines/doom/adapters/include",
                "-I/source/engines/doom/music",
                "-I/source/engines/doom/music/upstream",
            ]
            .map(str::to_string),
        );
        for name in lock["sources"]
            .as_array()
            .context("invalid DOOM source list")?
        {
            args.push(format!(
                "/source/engines/doom/upstream/{}",
                name.as_str().context("invalid DOOM source name")?
            ));
        }
        args.push("/source/engines/doom/adapters/engine.c".into());
        args.push("/source/engines/doom/merge/upstream/w_merge.c".into());
        args.extend(
            MUSIC_SOURCES
                .iter()
                .map(|name| format!("/source/engines/doom/music/{name}")),
        );
        for syntax in [true, false] {
            let mut cmd = crate::compiler::command(32 * 1048576);
            cmd.args([
                "bwrap",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                "--clearenv",
                "--ro-bind",
                "/usr",
                "/usr",
            ]);
            for path in ["/lib", "/lib64"] {
                if Path::new(path).exists() {
                    cmd.args(["--ro-bind", path, path]);
                }
            }
            cmd.arg("--ro-bind")
                .arg(kit.path())
                .arg("/source")
                .arg("--ro-bind")
                .arg(&sysroot)
                .arg("/sysroot")
                .arg("--bind")
                .arg(output.path())
                .arg("/work")
                .args([
                    "--proc",
                    "/proc",
                    "--dev",
                    "/dev",
                    "--tmpfs",
                    "/tmp",
                    "--",
                    "/usr/bin/clang-18",
                ])
                .args(&args);
            if syntax {
                cmd.arg("-fsyntax-only");
            } else {
                cmd.args([
                    "-fuse-ld=/usr/bin/wasm-ld-18",
                    "-nostartfiles",
                    "-Wl,--no-entry",
                    "-Wl,--export-memory",
                    "-Wl,--gc-sections",
                    "-Wl,--initial-memory=16777216",
                    "-Wl,--max-memory=67108864",
                    "-Wl,-z,stack-size=1048576",
                    "-o",
                    "/work/engine.wasm",
                ]);
                for name in [
                    "fopen",
                    "fclose",
                    "fread",
                    "fwrite",
                    "fseek",
                    "ftell",
                    "feof",
                    "ferror",
                    "fflush",
                    "fgetc",
                    "fputc",
                    "fprintf",
                    "fgets",
                    "fscanf",
                    "__isoc99_fscanf",
                    "remove",
                    "rename",
                    "mkdir",
                    "getenv",
                    "access",
                    "memcpy",
                    "memmove",
                    "memset",
                    "__wasi_fd_write",
                    "__wasi_fd_close",
                    "__wasi_fd_fdstat_get",
                    "__wasi_fd_seek",
                    "__wasi_proc_exit",
                ] {
                    cmd.arg(format!("-Wl,--wrap={name}"));
                }
            }
            let result = crate::compiler::run(&mut cmd)?;
            if !result.status.success() {
                bail!("isolated DOOM build failed: {}", result.stderr);
            }
        }
        let bytes =
            crate::files::read_regular(&output.path().join("engine.wasm"), 32 * 1048576, false)?;
        stamp_engine(bytes)
    }
}
pub struct Doom {
    store: Store<StoreLimits>,
    memory: Memory,
    step: TypedFunc<i32, i32>,
    input: Option<TypedFunc<(i32, i32, i32), i32>>,
    quit: Option<TypedFunc<(), i32>>,
    pub(crate) exited: bool,
    pub(crate) ending: bool,
    reset: TypedFunc<(), ()>,
    started: bool,
    pixels: TypedFunc<(), i32>,
    audio: TypedFunc<(), i32>,
    audio_channels: u32,
    log_pointer: Option<usize>,
    save: TypedFunc<(), i32>,
    save_length: TypedFunc<(), i32>,
    save_capacity: usize,
    dirty: TypedFunc<(), i32>,
    status: TypedFunc<(), i32>,
    position: TypedFunc<(), i32>,
    controller: TypedFunc<(), i32>,
}
impl Doom {
    pub fn new(
        engine: &Engine,
        bytes: &[u8],
        wad: &[u8],
        patches: &[Vec<u8>],
        modes: &[crate::manifest::DoomWadMode],
        save: Option<&[u8]>,
        deadline: u64,
    ) -> Result<Self> {
        validate_wad(wad)?;
        validate_patch_set(wad.len(), patches)?;
        anyhow::ensure!(
            modes.is_empty() || modes.len() == patches.len(),
            "DOOM patch modes do not match selected WADs"
        );
        validate_current(bytes)?;
        let module = Module::new(engine, bytes)?;
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY as usize)
            .instances(1)
            .memories(1)
            .tables(1)
            .table_elements(10000)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(engine, limits);
        store.limiter(|s| s);
        store.set_epoch_deadline(deadline);
        let instance = Instance::new(&mut store, &module, &[])?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("missing DOOM memory")?;
        let allocate = instance.get_typed_func::<i32, i32>(&mut store, "allocate")?;
        let initialize =
            instance.get_typed_func::<(i32, i32, i32), i32>(&mut store, "initialize")?;
        let log_pointer = if let Some(function) = instance.get_func(&mut store, "diagnostics") {
            store.set_epoch_deadline(deadline);
            Some(function.typed::<(), i32>(&store)?.call(&mut store, ())? as usize)
        } else {
            None
        };
        let saved = instance.get_typed_func::<(), i32>(&mut store, "save")?;
        let save = save.unwrap_or(&[]);
        let save_capacity = if let Some(function) = instance.get_func(&mut store, "save_capacity") {
            store.set_epoch_deadline(deadline);
            function.typed::<(), i32>(&store)?.call(&mut store, ())? as usize
        } else {
            512 * 1024
        };
        if !(512 * 1024..=SAVE_MAX).contains(&save_capacity) {
            bail!("Invalid DOOM save capacity");
        }
        if save.len() > save_capacity
            || (save_capacity < SAVE_MAX && save.starts_with(b"\x89GPD2\r\n\x1a"))
        {
            bail!("Rebuild DOOM to open this multi-slot save bundle");
        }
        // Host instantiation and export lookup are outside the guest call budget.
        store.set_epoch_deadline(deadline);
        let pointer = saved.call(&mut store, ())?;
        memory.write(&mut store, pointer as usize, save)?;
        let pointer = allocate.call(&mut store, wad.len() as i32)?;
        if pointer <= 0 {
            bail!("unable to allocate DOOM WAD");
        }
        memory.write(&mut store, pointer as usize, wad)?;
        if !patches.is_empty() {
            let add = instance
                .get_typed_func::<(i32, i32), i32>(&mut store, "add_patch")
                .context("rebuild DOOM for patch WAD support")?;
            let mode_function = if modes.is_empty() {
                None
            } else {
                Some(
                    instance
                        .get_typed_func::<(i32, i32), i32>(&mut store, "set_patch_mode")
                        .context("rebuild DOOM for merge modes")?,
                )
            };
            for (index, patch) in patches.iter().enumerate() {
                store.set_epoch_deadline(deadline);
                let patch_pointer = allocate.call(&mut store, patch.len() as i32)?;
                if patch_pointer <= 0 {
                    bail!("unable to allocate DOOM patch WAD");
                }
                memory.write(&mut store, patch_pointer as usize, patch)?;
                if add.call(&mut store, (patch_pointer, patch.len() as i32))? != 0 {
                    bail!("DOOM rejected patch WAD registration");
                }
                if let Some(function) = mode_function.as_ref() {
                    if function.call(&mut store, (index as i32, modes[index] as i32))? != 0 {
                        bail!("DOOM rejected WAD loading mode");
                    }
                }
            }
        }
        store.set_epoch_deadline(deadline);
        let initialized =
            initialize.call(&mut store, (pointer, wad.len() as i32, save.len() as i32));
        if initialized.as_ref().is_ok_and(|code| *code != 0) {
            bail!("DOOM save bundle is invalid or damaged");
        }
        if let Err(e) = initialized {
            if let Some(pointer) = log_pointer {
                let mut text = [0u8; 4096];
                if memory.read(&store, pointer, &mut text).is_ok() {
                    bail!(
                        "DOOM initialization failed: {e}: {}",
                        String::from_utf8_lossy(&text).trim_end_matches('\0')
                    );
                }
            }
            return Err(e.context("DOOM initialization failed"));
        }
        let audio_channels = if let Some(function) = instance.get_func(&mut store, "audio_channels")
        {
            store.set_epoch_deadline(deadline);
            function.typed::<(), i32>(&store)?.call(&mut store, ())? as u32
        } else {
            1
        };
        if !matches!(audio_channels, 1 | 2) {
            bail!("Invalid DOOM audio channel count");
        }
        Ok(Self {
            step: instance.get_typed_func(&mut store, "step")?,
            input: instance
                .get_func(&mut store, "engine_input")
                .map(|f| f.typed(&store))
                .transpose()?,
            reset: instance.get_typed_func(&mut store, "reset")?,
            started: false,
            quit: instance
                .get_func(&mut store, "quit_requested")
                .map(|f| f.typed(&store))
                .transpose()?,
            exited: false,
            ending: false,
            pixels: instance.get_typed_func(&mut store, "pixels")?,
            audio: instance.get_typed_func(&mut store, "audio")?,
            audio_channels,
            log_pointer,
            save: saved,
            save_capacity,
            save_length: instance.get_typed_func(&mut store, "save_length")?,
            dirty: instance.get_typed_func(&mut store, "save_dirty")?,
            status: instance.get_typed_func(&mut store, "status")?,
            position: instance.get_typed_func(&mut store, "position")?,
            controller: instance.get_typed_func(&mut store, "controller")?,
            store,
            memory,
        })
    }
    pub(crate) fn input_events(
        &mut self,
        events: &[crate::doom_input::Event],
        deadline: u64,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let function = self
            .input
            .as_ref()
            .context("Rebuild DOOM to enable desktop keyboard/mouse input")?;
        self.store.set_epoch_deadline(deadline);
        for event in events {
            let [kind, a, b] = event.words()?;
            anyhow::ensure!(
                function.call(&mut self.store, (kind, a, b))? == 0,
                "invalid DOOM desktop event"
            );
        }
        Ok(())
    }
    pub fn step(
        &mut self,
        input: u32,
        restart: bool,
        deadline: u64,
    ) -> Result<crate::zelda3::Frame> {
        self.store.set_epoch_deadline(deadline);
        if restart && self.started {
            self.reset.call(&mut self.store, ())?;
        }
        self.started = true;
        if let Err(error) = self.step.call(&mut self.store, input as i32) {
            if let Some(pointer) = self.log_pointer {
                let mut bytes = [0u8; 4096];
                if self.memory.read(&self.store, pointer, &mut bytes).is_ok() {
                    let text = String::from_utf8_lossy(&bytes)
                        .trim_end_matches('\0')
                        .trim()
                        .to_owned();
                    if !text.is_empty() {
                        return Err(error.context(format!("DOOM engine: {text}")));
                    }
                }
            }
            return Err(error);
        }
        self.ending = (self.controller.call(&mut self.store, ())? as u32) & (1 << 25) != 0;
        if let Some(quit) = &self.quit {
            self.exited = quit.call(&mut self.store, ())? != 0;
        }
        let p = self.pixels.call(&mut self.store, ())?;
        let mut pixels = vec![0u8; 320 * 200 * 4];
        self.memory.read(&self.store, p as usize, &mut pixels)?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let p = self.audio.call(&mut self.store, ())?;
        let mut pcm = vec![0u8; 1600 * self.audio_channels as usize];
        self.memory.read(&self.store, p as usize, &mut pcm)?;
        let audio = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        let save = if self.dirty.call(&mut self.store, ())? != 0 {
            let n = self.save_length.call(&mut self.store, ())? as usize;
            if n > self.save_capacity {
                bail!("DOOM save exceeds its limit");
            }
            let p = self.save.call(&mut self.store, ())?;
            let mut bytes = vec![0; n];
            self.memory.read(&self.store, p as usize, &mut bytes)?;
            Some(bytes)
        } else {
            None
        };
        Ok(crate::zelda3::Frame {
            width: 320,
            height: 200,
            pixels,
            audio,
            audio_channels: self.audio_channels,
            save,
        })
    }
    pub fn state(&mut self) -> Result<(u32, u16, u16)> {
        let state = self.status.call(&mut self.store, ())? as u32;
        let xy = self.position.call(&mut self.store, ())? as u32;
        Ok((state, xy as u16, (xy >> 16) as u16))
    }
    pub fn controller(&mut self) -> Result<u32> {
        Ok(self.controller.call(&mut self.store, ())? as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn music_source_kit_rejects_changed_or_missing_pinned_files() {
        let root = tempfile::tempdir().unwrap();
        for (name, bytes) in crate::doom_sources::FILES
            .iter()
            .filter(|(name, _)| name.starts_with("engines/doom/music/upstream/"))
        {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        verify_music_sources(root.path()).unwrap();
        let path = root.path().join("engines/doom/music/upstream/opl3.c");
        std::fs::write(&path, "changed emulator").unwrap();
        assert!(verify_music_sources(root.path())
            .unwrap_err()
            .to_string()
            .contains("source changed"));
        std::fs::remove_file(path).unwrap();
        assert!(verify_music_sources(root.path()).is_err());
    }
    fn wad() -> Vec<u8> {
        let mut w = b"IWAD".to_vec();
        w.extend(1u32.to_le_bytes());
        w.extend(12u32.to_le_bytes());
        w.extend(0u32.to_le_bytes());
        w.extend(0u32.to_le_bytes());
        w.extend(b"E1M1\0\0\0\0");
        w
    }
    #[test]
    fn wad_header_directory_and_lump_ranges_are_bounded() {
        assert!(validate_wad(&wad()).is_ok());
        for bytes in [vec![], b"IWAD".to_vec(), b"PWAD00000000".to_vec()] {
            assert!(validate_wad(&bytes).is_err());
        }
        for (offset, value) in [
            (4, 0),
            (4, u32::MAX),
            (8, u32::MAX),
            (12, u32::MAX),
            (16, u32::MAX),
        ] {
            let mut w = wad();
            w[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert!(validate_wad(&w).is_err(), "offset {offset}");
        }
        let mut w = wad();
        w[20..].copy_from_slice(b"NOTMAP00");
        assert!(validate_wad(&w).is_err());
    }
    #[test]
    fn source_stamp_rejects_legacy_changed_duplicate_and_invalid_engines() {
        let old = wat::parse_str("(module (memory 1 1024))").unwrap();
        validate(&old).unwrap();
        assert!(validate_current(&old)
            .unwrap_err()
            .to_string()
            .contains("rebuild"));
        let current = stamp_engine(old).unwrap();
        validate_current(&current).unwrap();
        assert!(stamp_engine(current.clone()).is_err());
        let mut changed = current.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(validate_current(&changed).is_err());
        let unbounded = wat::parse_str("(module (memory 1))").unwrap();
        assert!(stamp_engine(unbounded).is_err());
        let imported =
            wat::parse_str("(module (import \"x\" \"y\" (func)) (memory 1 1024))").unwrap();
        assert!(stamp_engine(imported).is_err());
    }
    #[test]
    fn fixed_math_rejects_changed_or_repeated_adaptation() {
        let original = include_str!("../engines/doom/upstream/m_fixed.c");
        let staged = adapt_fixed(original).unwrap();
        assert!(adapt_fixed(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/doom/adapters/fixed-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(adapt_fixed(&original.replace(before, "/* changed */")).is_err());
            assert!(!staged.contains(before));
        }
        assert!(original.contains("abs(a)"));
    }
    #[test]
    fn sound_id_bounds_reject_changed_or_repeated_adaptation() {
        let original = include_str!("../engines/doom/upstream/s_sound.c");
        let staged = adapt_sound(original).unwrap();
        assert!(staged.contains("sfx_id >= NUMSFX"));
        assert!(adapt_sound(&staged).is_err());
        assert!(adapt_sound(&original.replace("sfx_id > NUMSFX", "sfx_id > 0")).is_err());
        assert!(adapt_sound(&format!("{original}\n{original}")).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/doom/adapters/sound-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(adapt_sound(&original.replace(before, "/* changed */")).is_err());
            assert!(!staged.contains(before));
        }
    }
    #[test]
    fn angle_adaptation_rejects_changed_or_repeated_anchor() {
        let original = include_str!("../engines/doom/upstream/r_main.c");
        let staged = adapt_angles(original).unwrap();
        assert!(staged.contains("int64_t x = (int64_t)point_x"));
        assert!(adapt_angles(&staged).is_err());
        assert!(adapt_angles(&original.replace("x -= viewx;", "x = 0;")).is_err());
        assert!(adapt_angles(&format!("{original}\n{original}")).is_err());
    }
    #[test]
    fn music_device_adaptation_rejects_changed_or_repeated_anchor() {
        let original = include_str!("../engines/doom/upstream/i_sound.c");
        let staged = adapt_music_devices(original).unwrap();
        assert!(staged.contains("SndDeviceInList(snd_musicdevice"));
        assert!(adapt_music_devices(&staged).is_err());
        assert!(adapt_music_devices(
            &original.replace("music_module = &DG_music_module;", "music_module = NULL;")
        )
        .is_err());
        assert!(adapt_music_devices(&format!("{original}\n{original}")).is_err());
    }
    #[test]
    fn sprite_coordinates_reject_changed_or_repeated_adaptation() {
        let original = include_str!("../engines/doom/upstream/r_data.c");
        let staged = adapt_sprites(original).unwrap();
        assert!(adapt_sprites(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/doom/adapters/sprite-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(adapt_sprites(&original.replace(before, "/* changed */")).is_err());
            assert!(!staged.contains(before));
        }
        assert!(original.contains("SHORT(patch->leftoffset)<<FRACBITS"));
    }
    #[test]
    fn map_coordinate_adaptation_checks_every_anchor_and_preserves_original() {
        let original = include_str!("../engines/doom/upstream/p_setup.c");
        let staged = adapt_map(original).unwrap();
        assert!(!staged.contains("<<FRACBITS"));
        assert!(adapt_map(&staged).is_err());
        let patches: serde_json::Value =
            serde_json::from_slice(include_bytes!("../engines/doom/adapters/map-patches.json"))
                .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(adapt_map(&original.replace(before, "/* changed */")).is_err());
        }
        assert!(original.contains("SHORT(ml->x)<<FRACBITS"));
    }
    #[test]
    fn endian_word_adaptation_rejects_changed_or_repeated_anchors() {
        let original = include_str!("../engines/doom/upstream/i_swap.h");
        let staged = adapt_endian(original).unwrap();
        assert!(staged.contains("uint32_t swapLE32(uint32_t val)"));
        assert!(adapt_endian(&staged).is_err());
        assert!(adapt_endian(&original.replace("unsigned long val", "unsigned int val")).is_err());
        assert!(original.contains("unsigned long swapLE32(unsigned long val)"));
    }
    #[test]
    fn video_byte_order_adaptation_is_checked_and_original_is_preserved() {
        let original = include_str!("../engines/doom/upstream/i_video.c");
        let staged = adapt_video(original).unwrap();
        assert!(staged.contains("#include \"i_swap.h\""));
        assert!(staged.contains("p = swapLE16(p)"));
        assert!(adapt_video(&staged).is_err());
        assert!(adapt_video(&original.replace("swapeLE16(p)", "other(p)")).is_err());
        assert!(original.contains("swapeLE16(p)"));
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn merge_feature_is_enabled_in_staging_and_tampered_anchor_rejected() {
        let root = tempfile::tempdir().unwrap();
        for (name, bytes) in crate::doom_sources::FILES {
            let path = root.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        stage_desktop_sources(root.path()).unwrap();
        let path = root.path().join("engines/doom/upstream/doomfeatures.h");
        let staged = std::fs::read_to_string(&path).unwrap();
        assert!(
            staged.contains("#define FEATURE_WAD_MERGE 1")
                && !staged.contains("#undef FEATURE_WAD_MERGE")
        );
        let original = include_str!("../engines/doom/upstream/doomfeatures.h");
        assert!(original.contains("#undef FEATURE_WAD_MERGE"));
        std::fs::write(
            path,
            original.replace("#undef FEATURE_WAD_MERGE", "#undef OTHER_FEATURE"),
        )
        .unwrap();
        assert!(stage_desktop_sources(root.path()).is_err());
    }
    #[test]
    #[cfg(target_os = "linux")]
    fn merge_source_kit_rejects_changed_or_missing_original_module() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("engines/doom/merge/upstream");
        std::fs::create_dir_all(&folder).unwrap();
        let source = include_bytes!("../engines/doom/merge/upstream/w_merge.c");
        let file = folder.join("w_merge.c");
        assert!(verify_merge_sources(root.path()).is_err());
        std::fs::write(&file, source).unwrap();
        verify_merge_sources(root.path()).unwrap();
        std::fs::write(&file, b"substitute merger").unwrap();
        assert!(verify_merge_sources(root.path()).is_err());
    }
    #[test]
    fn patch_wads_accept_resource_only_lumps_and_reject_every_truncation() {
        let mut patch = b"PWAD".to_vec();
        patch.extend(2u32.to_le_bytes());
        patch.extend(16u32.to_le_bytes());
        patch.extend([1, 2, 3, 4]);
        for _ in 0..2 {
            patch.extend(12u32.to_le_bytes());
            patch.extend(4u32.to_le_bytes());
            patch.extend(b"TITLEPIC");
        }
        // Duplicate names, shared payloads and resource-only patches are valid.
        validate_patch_wad(&patch).unwrap();
        assert!(validate_wad(&patch).is_err());
        assert!(validate_patch_wad(&wad()).is_err());
        for length in 0..patch.len() {
            assert!(
                validate_patch_wad(&patch[..length]).is_err(),
                "length {length}"
            );
        }
        for field in [4, 8, 16, 20, 32, 36] {
            let mut malformed = patch.clone();
            malformed[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(validate_patch_wad(&malformed).is_err(), "field {field}");
        }
        let mut empty = b"PWAD".to_vec();
        empty.extend(0u32.to_le_bytes());
        empty.extend(12u32.to_le_bytes());
        validate_patch_wad(&empty).unwrap();
        for signature in [*b"IWAD", *b"pwad", *b"ZIP!"] {
            let mut invalid = patch.clone();
            invalid[..4].copy_from_slice(&signature);
            assert!(validate_patch_wad(&invalid).is_err());
        }
    }
    #[test]
    fn engine_rejects_host_imports_and_unbounded_memory() {
        for text in [
            "(module)",
            "(module (memory 1))",
            "(module (memory 1 1025))",
            "(module (memory 1 1024 shared))",
            "(module (memory i64 1 1024))",
            "(module (import \"wasi_snapshot_preview1\" \"path_open\" (func)) (memory 1 1024))",
            "(module (import \"env\" \"system\" (func)) (memory 1 1024))",
        ] {
            assert!(validate(&wat::parse_str(text).unwrap()).is_err());
        }
        assert!(validate(&wat::parse_str("(module (memory 1 1024))").unwrap()).is_ok());
    }
}
