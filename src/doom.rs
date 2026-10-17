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
pub(crate) fn stage_config_source(root: &std::path::Path) -> Result<()> {
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
    Ok(())
}
pub fn validate_wad(wad: &[u8]) -> Result<()> {
    if wad.len() < 12 || wad.len() > 64 * 1048576 || &wad[..4] != b"IWAD" {
        bail!("select a supported DOOM IWAD (DOOM.WAD or DOOM2.WAD)");
    }
    let count = u32::from_le_bytes(wad[4..8].try_into()?) as usize;
    let offset = u32::from_le_bytes(wad[8..12].try_into()?) as usize;
    if count == 0 || count > 65536 || offset > wad.len() || count > (wad.len() - offset) / 16 {
        bail!("invalid DOOM WAD directory");
    }
    let mut level = false;
    for entry in wad[offset..offset + count * 16].as_chunks::<16>().0 {
        let start = u32::from_le_bytes(entry[..4].try_into()?) as usize;
        let length = u32::from_le_bytes(entry[4..8].try_into()?) as usize;
        if start > wad.len() || length > wad.len() - start {
            bail!("DOOM WAD lump exceeds file bounds");
        }
        level |= &entry[8..] == b"E1M1\0\0\0\0" || &entry[8..] == b"MAP01\0\0\0";
    }
    if !level {
        bail!("DOOM WAD has no supported starting map");
    }
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
        stage_config_source(kit.path())?;
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
        validate(&bytes)?;
        Ok(bytes)
    }
}
pub struct Doom {
    store: Store<StoreLimits>,
    memory: Memory,
    step: TypedFunc<i32, i32>,
    reset: TypedFunc<(), ()>,
    started: bool,
    pixels: TypedFunc<(), i32>,
    audio: TypedFunc<(), i32>,
    audio_channels: u32,
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
        save: Option<&[u8]>,
        deadline: u64,
    ) -> Result<Self> {
        validate_wad(wad)?;
        validate(bytes)?;
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
        store.set_epoch_deadline(deadline);
        let initialized =
            initialize.call(&mut store, (pointer, wad.len() as i32, save.len() as i32));
        if initialized.as_ref().is_ok_and(|code| *code != 0) {
            bail!("DOOM save bundle is invalid or damaged");
        }
        if let Err(e) = initialized {
            let logs = instance.get_typed_func::<(), i32>(&mut store, "diagnostics")?;
            store.set_epoch_deadline(deadline);
            let p = logs.call(&mut store, ())?;
            let mut text = vec![0u8; 4096];
            memory.read(&store, p as usize, &mut text)?;
            bail!(
                "DOOM initialization failed: {e}: {}",
                String::from_utf8_lossy(&text).trim_end_matches('\0')
            );
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
            reset: instance.get_typed_func(&mut store, "reset")?,
            started: false,
            pixels: instance.get_typed_func(&mut store, "pixels")?,
            audio: instance.get_typed_func(&mut store, "audio")?,
            audio_channels,
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
        self.step.call(&mut self.store, input as i32)?;
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
