//! Pinned Zelda3 engine: Wasm-only execution and selected-ROM decoding in RAM.
use anyhow::{bail, Context, Result};
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    thread,
};
use wasmtime::{
    Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
pub const PROFILE: &str = "zelda3-45a149d8";
pub const MEMORY: u64 = 16 * 1048576;
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
    cmd.args([
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--setenv",
        "PYTHONDONTWRITEBYTECODE",
        "1",
    ]);
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
        let sysroot = std::env::var_os("GAMEPKG_ZELDA3_SYSROOT")
            .map(PathBuf::from).or_else(||Path::new("/usr/include/wasm32-wasi").is_dir().then(||PathBuf::from("/usr")))
            .context("install Clang 18, wasi-libc and libclang-rt-18-dev-wasm32, or configure GAMEPKG_ZELDA3_SYSROOT")?;
        let sysroot = std::fs::canonicalize(sysroot)?;
        let mut cmd = isolated(kit.path(), 32 * 1048576);
        cmd.arg("--ro-bind").arg(sysroot).arg("/sysroot");
        cmd.arg("--bind").arg(output.path()).arg("/work");
        cmd.args([
            "--",
            "/usr/bin/python3",
            "/source/tools/build_zelda3_engine.py",
            "--sysroot",
            "/sysroot",
            "--output",
            "/work/engine.wasm",
        ]);
        let result = crate::compiler::run(&mut cmd)?;
        if !result.status.success() {
            bail!("isolated Zelda3 engine build failed: {}", result.stderr);
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
fn decode(rom: &[u8]) -> Result<Vec<u8>> {
    if !matches!(rom.len(), 1048576 | 1049088) {
        bail!("Zelda3 needs the supported USA 1 MiB ROM, optionally with a 512-byte copier header");
    }
    let kit = kit()?;
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut cmd = isolated(kit.path(), 0);
        cmd.args([
            "--",
            "/usr/bin/python3",
            "/source/engines/zelda3/adapters/rom_resources.py",
            "--stdin",
        ]);
        cmd
    };
    #[cfg(not(target_os = "linux"))]
    let mut cmd = {
        let default = if cfg!(windows) {
            "python.exe"
        } else {
            "python3"
        };
        let interpreter =
            std::env::var_os("GAMEPKG_ZELDA3_PYTHON").unwrap_or_else(|| default.into());
        let mut cmd = Command::new(interpreter);
        cmd.arg("-I");
        cmd.arg(kit.path().join("engines/zelda3/adapters/rom_resources.py"))
            .arg("--stdin")
            .env("PYTHONDONTWRITEBYTECODE", "1");
        cmd
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("ROM decoding requires the host Python runtime with Pillow and PyYAML")?;
    let mut stdin = child.stdin.take().unwrap();
    let input = rom.to_vec();
    let writer = thread::spawn(move || stdin.write_all(&input));
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(8388609).read_to_end(&mut bytes).map(|_| bytes)
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.take(65536).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(65);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("ROM decoding exceeded its deadline");
        }
        thread::sleep(std::time::Duration::from_millis(20));
    };
    let _ = writer.join();
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("ROM decoder output stopped"))??;
    let errors = errors
        .join()
        .map_err(|_| anyhow::anyhow!("ROM decoder diagnostics stopped"))??;
    if !status.success() {
        bail!(
            "unable to decode selected Zelda3 ROM: {}",
            String::from_utf8_lossy(&errors)
        );
    }
    if bytes.len() > 8388608 {
        bail!("ROM decoder exceeded resource limit");
    }
    Ok(bytes)
}
pub struct Zelda {
    store: Store<StoreLimits>,
    memory: Memory,
    step: TypedFunc<i32, i32>,
    reset: TypedFunc<(), ()>,
    pixels: TypedFunc<(), i32>,
    audio: TypedFunc<(), i32>,
    sram: TypedFunc<(), i32>,
    dirty: TypedFunc<(), i32>,
    status: TypedFunc<(), i32>,
    position: TypedFunc<(), i32>,
    controller: TypedFunc<(), i32>,
}
pub struct Frame {
    pub pixels: Vec<u8>,
    pub audio: Vec<i16>,
    pub save: Option<Vec<u8>>,
    pub fuel: u64,
}
impl Zelda {
    pub fn new(
        engine: &Engine,
        bytes: &[u8],
        rom: &[u8],
        save: Option<&[u8]>,
        fuel: u64,
        deadline: u64,
    ) -> Result<Self> {
        validate(bytes)?;
        let resources = decode(rom)?;
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
        store.set_fuel(fuel)?;
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
    pub fn step(&mut self, input: u32, restart: bool, fuel: u64, deadline: u64) -> Result<Frame> {
        self.store.set_fuel(fuel)?;
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
        let mut pcm = vec![0u8; 1600];
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
            save,
            fuel: self.store.get_fuel()?,
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
