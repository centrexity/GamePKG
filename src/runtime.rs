//! Experimental grid-v1 core-Wasm ABI with scoped archive access and no WASI.
use crate::manifest::Manifest;
use anyhow::{bail, Context, Result};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use wasmtime::{
    Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};
struct Data {
    limits: StoreLimits,
    package: Option<crate::archive::Package>,
    rom: Option<Vec<u8>>,
    save: Option<crate::saves::Save>,
}
pub struct Game {
    store: Store<Data>,
    tick: TypedFunc<(i32, i64), i64>,
    fuel: u64,
    deadline_epochs: u64,
    stop: Arc<AtomicBool>,
    pub state: u64,
    pub id: String,
    input_allowed: bool,
}
impl Drop for Game {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
impl Game {
    pub fn from_bytes(bytes: &[u8], manifest: &Manifest) -> Result<Self> {
        Self::with_settings(bytes, manifest, &crate::settings::Settings::default())
    }
    pub fn with_settings(
        bytes: &[u8],
        manifest: &Manifest,
        settings: &crate::settings::Settings,
    ) -> Result<Self> {
        Self::with_package(bytes, manifest, settings, None, None, None)
    }
    fn with_package(
        bytes: &[u8],
        manifest: &Manifest,
        settings: &crate::settings::Settings,
        package: Option<crate::archive::Package>,
        rom: Option<Vec<u8>>,
        save: Option<crate::saves::Save>,
    ) -> Result<Self> {
        settings.validate()?;
        if manifest.requires_rom && rom.is_none() {
            bail!("ROM delivery requires a file selected by you");
        }
        if !manifest.requires_rom && rom.is_some() {
            bail!("this game does not request a user ROM");
        }
        if manifest.entry_world != "game" {
            bail!("unsupported runtime entry world");
        }
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config)?;
        if bytes.len() > 32 * 1024 * 1024 {
            bail!("compiled Wasm exceeds 32 MiB runtime limit");
        }
        let module = Module::new(&engine, bytes)?;
        if module.imports().any(|i| {
            i.module() != "gamepkg"
                || !matches!(
                    i.name(),
                    "asset_read"
                        | "rom_read"
                        | "rom_size"
                        | "save_read"
                        | "save_write"
                        | "save_size"
                )
        }) {
            bail!("games cannot import functions outside the scoped GamePKG API");
        }
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        let fuel = manifest.limits.fuel.min(settings.fuel_ceiling);
        let deadline_epochs = settings.tick_deadline_ms / 10;
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory as usize)
            .table_elements(10000)
            .instances(1)
            .memories(1)
            .tables(1)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &engine,
            Data {
                limits,
                package,
                rom,
                save,
            },
        );
        store.limiter(|s| &mut s.limits);
        store.set_fuel(fuel)?;
        store.set_epoch_deadline(deadline_epochs);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let clock = engine.clone();
        thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(10));
                clock.increment_epoch();
            }
        });
        let result = (|| -> Result<_> {
            let mut linker = Linker::new(&engine);
            linker.func_wrap("gamepkg", "asset_read", asset_read)?;
            linker.func_wrap("gamepkg", "rom_read", rom_read)?;
            linker.func_wrap("gamepkg", "save_read", save_read)?;
            linker.func_wrap("gamepkg", "save_write", save_write)?;
            linker.func_wrap("gamepkg", "save_size", |caller: Caller<'_, Data>| -> i32 {
                caller
                    .data()
                    .save
                    .as_ref()
                    .map_or(-1, |s| s.bytes.len() as i32)
            })?;
            linker.func_wrap("gamepkg", "rom_size", |caller: Caller<'_, Data>| -> i32 {
                caller.data().rom.as_ref().map_or(-1, |r| r.len() as i32)
            })?;
            let instance = linker.instantiate(&mut store, &module)?;
            let tick = instance
                .get_typed_func::<(i32, i64), i64>(&mut store, "gamepkg_tick")
                .context("game has no grid-v1 entry point; use a playable example and rebuild")?;
            Ok(tick)
        })();
        let tick = match result {
            Ok(t) => t,
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                return Err(e);
            }
        };
        Ok(Self {
            store,
            tick,
            fuel,
            deadline_epochs,
            stop,
            state: 0,
            id: manifest.id.clone(),
            input_allowed: manifest.permissions.input,
        })
    }
    pub fn load(root: &Path, id: &str) -> Result<Self> {
        Self::load_with_rom(root, id, None)
    }
    pub fn load_with_rom(root: &Path, id: &str, rom_path: Option<&Path>) -> Result<Self> {
        crate::registry::list(root)?
            .into_iter()
            .find(|m| m.id == id)
            .context("game is not installed")?;
        let archive = std::fs::read(root.join(format!("{id}.gamepkg")))?;
        let mut snapshot = tempfile::NamedTempFile::new()?;
        use std::io::Write;
        snapshot.write_all(&archive)?;
        snapshot.flush()?;
        let mut package = crate::archive::Package::open(snapshot.path())?;
        let manifest = crate::verify::verify_embedded(&mut package)?;
        let cache = root
            .join("builds")
            .join(crate::verify::digest(&archive))
            .join("game.wasm");
        let bytes = std::fs::read(cache).context("build this game before playing")?;
        let rom = match rom_path {
            Some(path) => {
                use std::io::Read;
                if !manifest.requires_rom {
                    bail!("this game does not request a user ROM");
                }
                let file = std::fs::File::open(path).context("unable to open selected ROM")?;
                if !file.metadata()?.is_file() {
                    bail!("ROM must be a regular file");
                }
                let mut bytes = Vec::new();
                file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.is_empty() || bytes.len() > 64 * 1024 * 1024 {
                    bail!("ROM must be 1 byte to 64 MiB");
                }
                Some(bytes)
            }
            None => None,
        };
        Self::with_package(
            &bytes,
            &manifest,
            &crate::settings::load()?,
            Some(package),
            rom,
            if manifest.permissions.save {
                Some(crate::saves::Save::open(
                    root,
                    &manifest.id,
                    &manifest.creator_key,
                )?)
            } else {
                None
            },
        )
    }
    pub fn step(&mut self, input: u32) -> Result<u64> {
        self.store.set_fuel(self.fuel)?;
        self.store.set_epoch_deadline(self.deadline_epochs);
        let previous_save = self.store.data().save.as_ref().map(|s| s.bytes.clone());
        let result = self
            .tick
            .call(
                &mut self.store,
                (
                    if self.input_allowed { input as i32 } else { 0 },
                    self.state as i64,
                ),
            )
            .context("game stopped by the Wasm sandbox");
        self.state = match result {
            Ok(state) => state as u64,
            Err(error) => {
                if let (Some(save), Some(bytes)) =
                    (self.store.data_mut().save.as_mut(), previous_save)
                {
                    save.bytes = bytes;
                    save.dirty = false;
                }
                return Err(error);
            }
        };
        if let Some(save) = self.store.data_mut().save.as_mut() {
            save.commit().context("unable to persist game save")?;
        }
        Ok(self.state)
    }
    pub fn player(&self) -> (u8, u8) {
        (self.state as u8, (self.state >> 8) as u8)
    }
    pub fn target(&self) -> (u8, u8) {
        ((self.state >> 16) as u8, (self.state >> 24) as u8)
    }
    pub fn score(&self) -> u32 {
        (self.state >> 32) as u32
    }
}

// Invalid guest pointers and virtual paths return an error without touching host memory.
fn asset_read(
    mut caller: Caller<'_, Data>,
    path: i32,
    path_len: i32,
    offset: i32,
    output: i32,
    output_len: i32,
) -> i32 {
    if path < 0
        || !(1..=1024).contains(&path_len)
        || offset < 0
        || output < 0
        || !(0..=65536).contains(&output_len)
    {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let mut name = vec![0; path_len as usize];
    if memory.read(&caller, path as usize, &mut name).is_err() {
        return -1;
    }
    let Ok(name) = std::str::from_utf8(&name) else {
        return -1;
    };
    if !crate::archive::valid_path(name) {
        return -1;
    }
    let Some(end) = (output as usize).checked_add(output_len as usize) else {
        return -1;
    };
    if end > memory.data_size(&caller) {
        return -1;
    }
    let size = caller
        .data()
        .package
        .as_ref()
        .and_then(|p| p.entries.get(&format!("assets/{name}")).copied());
    let Some(size) = size else {
        return -1;
    };
    // Charge archive work against fuel as host calls are outside Wasm metering.
    let cost = size.saturating_add(1024);
    let Ok(fuel) = caller.get_fuel() else {
        return -1;
    };
    if fuel < cost {
        let _ = caller.set_fuel(0);
        return -1;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        return -1;
    }
    let Some(package) = caller.data_mut().package.as_mut() else {
        return -1;
    };
    let Ok(bytes) = package.asset(name) else {
        return -1;
    };
    let Some(bytes) = bytes.get(offset as usize..) else {
        return -1;
    };
    let length = bytes.len().min(output_len as usize);
    if memory
        .write(&mut caller, output as usize, &bytes[..length])
        .is_err()
    {
        return -1;
    }
    length as i32
}

fn rom_read(mut caller: Caller<'_, Data>, offset: i32, output: i32, length: i32) -> i32 {
    if offset < 0 || output < 0 || !(0..=65536).contains(&length) {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let Some(end) = (output as usize).checked_add(length as usize) else {
        return -1;
    };
    if end > memory.data_size(&caller) {
        return -1;
    }
    let Ok(fuel) = caller.get_fuel() else {
        return -1;
    };
    let cost = length as u64 + 128;
    if fuel < cost {
        let _ = caller.set_fuel(0);
        return -1;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        return -1;
    }
    let Some(bytes) = caller
        .data()
        .rom
        .as_ref()
        .and_then(|r| r.get(offset as usize..))
    else {
        return -1;
    };
    let bytes = bytes[..bytes.len().min(length as usize)].to_vec();
    if memory.write(&mut caller, output as usize, &bytes).is_err() {
        return -1;
    }
    bytes.len() as i32
}

fn save_write(mut caller: Caller<'_, Data>, pointer: i32, length: i32) -> i32 {
    if pointer < 0
        || !(0..=crate::saves::MAX_SAVE as i32).contains(&length)
        || caller.data().save.is_none()
    {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let Some(end) = (pointer as usize).checked_add(length as usize) else {
        return -1;
    };
    if end > memory.data_size(&caller) {
        return -1;
    }
    let cost = length as u64 + 1024;
    let Ok(fuel) = caller.get_fuel() else {
        return -1;
    };
    if fuel < cost {
        let _ = caller.set_fuel(0);
        return -1;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        return -1;
    }
    let mut bytes = vec![0; length as usize];
    if memory.read(&caller, pointer as usize, &mut bytes).is_err() {
        return -1;
    }
    let save = caller.data_mut().save.as_mut().unwrap();
    save.bytes = bytes;
    save.dirty = true;
    0
}
fn save_read(mut caller: Caller<'_, Data>, offset: i32, pointer: i32, length: i32) -> i32 {
    if offset < 0 || pointer < 0 || !(0..=65536).contains(&length) {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let Some(end) = (pointer as usize).checked_add(length as usize) else {
        return -1;
    };
    if end > memory.data_size(&caller) {
        return -1;
    }
    let cost = length as u64 + 128;
    let Ok(fuel) = caller.get_fuel() else {
        return -1;
    };
    if fuel < cost {
        let _ = caller.set_fuel(0);
        return -1;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        return -1;
    }
    let Some(bytes) = caller
        .data()
        .save
        .as_ref()
        .and_then(|s| s.bytes.get(offset as usize..))
    else {
        return -1;
    };
    let bytes = bytes[..bytes.len().min(length as usize)].to_vec();
    if memory.write(&mut caller, pointer as usize, &bytes).is_err() {
        return -1;
    }
    bytes.len() as i32
}
