//! Pinned Zelda3 engine: Wasm-only execution and selected-ROM decoding in RAM.
use anyhow::{bail, Context, Result};
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;
use wasmtime::{
    Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
pub const PROFILE: &str = "zelda3-45a149d8";
pub const MEMORY: u64 = 16 * 1048576;
#[cfg(target_os = "linux")]
fn kit() -> Result<tempfile::TempDir> {
    let kit = tempfile::tempdir()?;
    for (name, bytes) in crate::zelda3_sources::FILES {
        let path = kit.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, bytes)?;
    }
    Ok(kit)
}
#[cfg(target_os = "linux")]
fn isolated(kit: &Path, max_file: u64) -> Command {
    let mut cmd = crate::compiler::command(max_file);
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
    cmd.arg("--ro-bind").arg(kit).arg("/source");
    cmd.args(["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"]);
    cmd
}
pub fn build() -> Result<Vec<u8>> {
    #[cfg(not(target_os = "linux"))]
    {
        bail!("Zelda3 source compilation currently requires the Linux isolated compiler");
    }
    #[cfg(target_os = "linux")]
    {
        let kit = kit()?;
        let output = tempfile::tempdir()?;
        let sysroot = crate::c_toolchain::resolve(&["GAMEPKG_ZELDA3_SYSROOT"])?;
        // The host owns these sources; no package can supply compiler scripts.
        let upstream = kit.path().join("engines/zelda3/upstream");
        let lock: serde_json::Value =
            serde_json::from_slice(include_bytes!("../engines/zelda3/upstream-lock.json"))?;
        for (name, digest) in lock["files"]
            .as_object()
            .context("invalid engine source lock")?
        {
            if name.ends_with(".py") {
                continue;
            }
            use sha2::Digest;
            let bytes = std::fs::read(upstream.join(name))?;
            if hex::encode(sha2::Sha256::digest(bytes)) != digest.as_str().unwrap_or("") {
                bail!("pinned Zelda3 source changed: {name}");
            }
        }
        let rtl_path = upstream.join("src/zelda_rtl.c");
        let mut rtl = std::fs::read_to_string(&rtl_path)?;
        for name in ["ZeldaReadSram", "ZeldaWriteSram"] {
            let begin = rtl
                .find(&format!("void {name}() {{"))
                .with_context(|| format!("missing pinned engine function {name}"))?;
            let brace = begin + rtl[begin..].find('{').unwrap();
            let mut depth = 0;
            let mut end = None;
            for (offset, byte) in rtl.as_bytes()[brace..].iter().enumerate() {
                match byte {
                    b'{' => depth += 1,
                    b'}' => depth -= 1,
                    _ => {}
                }
                if depth == 0 {
                    end = Some(brace + offset + 1);
                    break;
                }
            }
            rtl.replace_range(begin..end.context("invalid pinned engine function")?, "");
        }
        std::fs::write(rtl_path, rtl)?;
        let source_root = "/source/engines/zelda3/upstream";
        let mut sources: Vec<String> = crate::zelda3_sources::FILES
            .iter()
            .filter_map(|(name, _)| {
                let tail = name.strip_prefix("engines/zelda3/upstream/src/")?;
                (tail.ends_with(".c")
                    && !tail.contains('/')
                    && ![
                        "main.c",
                        "audio.c",
                        "config.c",
                        "glsl_shader.c",
                        "opengl.c",
                        "zelda_cpu_infra.c",
                    ]
                    .contains(&tail))
                .then(|| format!("{source_root}/src/{tail}"))
            })
            .collect();
        sources.sort();
        for name in ["ppu.c", "dma.c", "dsp.c", "snes_other.c"] {
            sources.push(format!("{source_root}/snes/{name}"));
        }
        sources.push("/source/engines/zelda3/adapters/engine.c".into());
        let prefix = if sysroot.join("usr/include/wasm32-wasi").is_dir() {
            "/sysroot/usr"
        } else {
            "/sysroot"
        };
        let flags = [
            "--target=wasm32-wasi".to_string(),
            format!("--sysroot={prefix}"),
            format!("-resource-dir={prefix}/lib/llvm-18/lib/clang/18"),
            "-isystem".into(),
            "/usr/lib/llvm-18/lib/clang/18/include".into(),
            "-O2".into(),
            "-mbulk-memory".into(),
            "-fno-strict-aliasing".into(),
            "-ffunction-sections".into(),
            "-fdata-sections".into(),
            format!("-I{source_root}"),
            "-I/source/engines/zelda3/adapters/include".into(),
        ];
        for syntax_only in [true, false] {
            let mut cmd = isolated(kit.path(), 32 * 1048576);
            cmd.arg("--ro-bind").arg(&sysroot).arg("/sysroot");
            cmd.arg("--bind").arg(output.path()).arg("/work");
            cmd.args(["--", "/usr/bin/clang-18"])
                .args(&flags)
                .args(&sources);
            if syntax_only {
                cmd.arg("-fsyntax-only");
            } else {
                cmd.args([
                    "-nostartfiles",
                    "-Wl,--no-entry",
                    "-Wl,--export-memory",
                    "-Wl,--gc-sections",
                    "-Wl,--wrap=memcpy",
                    "-Wl,--wrap=memmove",
                    "-Wl,--wrap=memset",
                    "-Wl,--wrap=__wasi_fd_close",
                    "-Wl,--wrap=__wasi_fd_fdstat_get",
                    "-Wl,--wrap=__wasi_fd_seek",
                    "-Wl,--wrap=__wasi_fd_write",
                    "-Wl,--initial-memory=4194304",
                    "-Wl,--max-memory=16777216",
                    "-Wl,-z,stack-size=1048576",
                    "-o",
                    "/work/engine.wasm",
                ]);
            }
            let result = crate::compiler::run(&mut cmd)?;
            if !result.status.success() {
                bail!("isolated Zelda3 engine build failed: {}", result.stderr);
            }
        }
        let bytes =
            crate::files::read_regular(&output.path().join("engine.wasm"), 32 * 1048576, false)?;
        validate(&bytes)?;
        Ok(bytes)
    }
}
pub fn validate(bytes: &[u8]) -> Result<()> {
    wasmparser::Validator::new().validate_all(bytes)?;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload? {
            wasmparser::Payload::ImportSection(imports) if imports.count() != 0 => {
                bail!("Zelda3 engine must have zero host imports")
            }
            wasmparser::Payload::MemorySection(memories) => {
                for memory in memories {
                    let memory = memory?;
                    if memory.memory64
                        || memory.shared
                        || memory.maximum.is_none_or(|max| max > 256)
                    {
                        bail!("Zelda3 engine exceeds its 16 MiB memory boundary");
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
pub struct Zelda {
    store: Store<StoreLimits>,
    memory: Memory,
    step: TypedFunc<i32, i32>,
    reset: TypedFunc<(), ()>,
    pixels: TypedFunc<(), i32>,
    audio: TypedFunc<(), i32>,
    audio_channels: u32,
    sram: TypedFunc<(), i32>,
    dirty: TypedFunc<(), i32>,
    status: TypedFunc<(), i32>,
    position: TypedFunc<(), i32>,
    controller: TypedFunc<(), i32>,
}
pub struct Frame {
    pub pixels: Vec<u8>,
    pub audio: Vec<i16>,
    pub audio_channels: u32,
    pub save: Option<Vec<u8>>,
}
impl Zelda {
    pub fn new(
        engine: &Engine,
        bytes: &[u8],
        rom: &[u8],
        save: Option<&[u8]>,
        deadline: u64,
    ) -> Result<Self> {
        validate(bytes)?;
        let resources = crate::zelda3_rom::decode(rom)?;
        let module = Module::new(engine, bytes)?;
        if module.imports().next().is_some() {
            bail!("Zelda3 engine cannot import host capabilities");
        }
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
            .context("missing Zelda3 engine memory")?;
        let allocate = instance.get_typed_func::<i32, i32>(&mut store, "allocate")?;
        let initialize = instance.get_typed_func::<(i32, i32), i32>(&mut store, "initialize")?;
        let pointer = allocate.call(&mut store, resources.len() as i32)?;
        if pointer <= 0 {
            bail!("unable to allocate Zelda3 resources");
        }
        memory.write(&mut store, pointer as usize, &resources)?;
        if initialize.call(&mut store, (pointer, resources.len() as i32))? != 0 {
            bail!("Zelda3 resource initialization failed");
        }
        let sram = instance.get_typed_func::<(), i32>(&mut store, "sram")?;
        if let Some(save) = save.filter(|s| !s.is_empty()) {
            if save.len() != 8192 {
                bail!("Zelda3 save must be an 8 KiB SRAM file");
            }
            let pointer = sram.call(&mut store, ())?;
            memory.write(&mut store, pointer as usize, save)?;
        }
        let step = instance.get_typed_func(&mut store, "step")?;
        let reset = instance.get_typed_func(&mut store, "reset")?;
        let pixels = instance.get_typed_func(&mut store, "pixels")?;
        let audio = instance.get_typed_func(&mut store, "audio")?;
        let audio_channels = if let Some(function) = instance.get_func(&mut store, "audio_channels")
        {
            store.set_epoch_deadline(deadline);
            function.typed::<(), i32>(&store)?.call(&mut store, ())? as u32
        } else {
            1
        };
        if !matches!(audio_channels, 1 | 2) {
            bail!("Invalid Zelda audio channel count");
        }

        let dirty = instance.get_typed_func(&mut store, "save_dirty")?;
        let status = instance.get_typed_func(&mut store, "status")?;
        let position = instance.get_typed_func(&mut store, "position")?;
        let controller = instance.get_typed_func(&mut store, "controller")?;
        Ok(Self {
            store,
            memory,
            step,
            reset,
            pixels,
            audio,
            audio_channels,
            sram,
            dirty,
            status,
            position,
            controller,
        })
    }
    pub fn controller(&mut self) -> Result<u32> {
        Ok(self.controller.call(&mut self.store, ())? as u32)
    }
    pub fn state(&mut self) -> Result<(u32, u16, u16)> {
        let mode = self.status.call(&mut self.store, ())? as u32;
        let xy = self.position.call(&mut self.store, ())? as u32;
        Ok((mode, xy as u16, (xy >> 16) as u16))
    }
    pub fn step(&mut self, input: u32, restart: bool, deadline: u64) -> Result<Frame> {
        self.store.set_epoch_deadline(deadline);
        if restart {
            self.reset.call(&mut self.store, ())?;
        }
        let mut buttons = 0;
        for (mask, bit) in [
            (0x101, 6),
            (0x202, 7),
            (0x404, 4),
            (0x808, 5),
            (0x1010, 0),
            (0x2020, 1),
            (0x4040, 3),
            (0x8080, 2),
            (0x110000, 8),
            (0x220000, 9),
            (0x440000, 10),
            (0x880000, 11),
        ] {
            if input & mask != 0 {
                buttons |= 1 << bit;
            }
        }
        if self.step.call(&mut self.store, buttons)? != 0 {
            bail!("Zelda3 engine stopped");
        }
        let pointer = self.pixels.call(&mut self.store, ())? as usize;
        let mut pixels = vec![0u8; 256 * 224 * 4];
        self.memory.read(&self.store, pointer, &mut pixels)?;
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        let pointer = self.audio.call(&mut self.store, ())? as usize;
        let mut pcm = vec![0u8; 1600 * self.audio_channels as usize];
        self.memory.read(&self.store, pointer, &mut pcm)?;
        let audio = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        let save = if self.dirty.call(&mut self.store, ())? != 0 {
            let pointer = self.sram.call(&mut self.store, ())? as usize;
            let mut bytes = vec![0; 8192];
            self.memory.read(&self.store, pointer, &mut bytes)?;
            Some(bytes)
        } else {
            None
        };
        Ok(Frame {
            pixels,
            audio,
            audio_channels: self.audio_channels,
            save,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_ambient_imports_and_unbounded_engine_memory() {
        for source in [
            "(module (import \"wasi_snapshot_preview1\" \"fd_write\" (func)) (memory 1 256))",
            "(module (memory 1))",
            "(module (memory 1 257))",
            "(module (memory 1 256 shared))",
        ] {
            assert!(super::validate(&wat::parse_str(source).unwrap()).is_err());
        }
        assert!(super::validate(&wat::parse_str("(module (memory 1 256))").unwrap()).is_ok());
    }
}
