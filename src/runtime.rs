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
mod component_api;

enum Tick {
    Core(TypedFunc<(i32, i64), i64>),
    Component(wasmtime::component::TypedFunc<(u32, u64), (u64,)>),
}
struct Data {
    achievements: Vec<String>,
    pending_achievements: Vec<String>,
    limits: StoreLimits,
    package: Option<crate::archive::Package>,
    rom: Option<Vec<Vec<u8>>>,
    save: Option<crate::saves::Save>,
    frame: crate::drawing::Frame,
    audio: Vec<i16>,
    audio_allowed: bool,
    shader_count: usize,
}
pub struct Game {
    progress: Option<crate::progress::Session>,
    store: Store<Data>,
    tick: Tick,
    fuel: u64,
    deadline_epochs: u64,
    stop: Arc<AtomicBool>,
    pub state: u64,
    pub id: String,
    input_allowed: bool,
    pub custom_renderer: bool,
    pub shader_sources: Vec<String>,
    uses_shaders: bool,
    pub audio_allowed: bool,
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
        rom: Option<Vec<Vec<u8>>>,
        save: Option<crate::saves::Save>,
    ) -> Result<Self> {
        settings.validate()?;
        if !matches!(
            manifest.runtime_profile.as_str(),
            "core-v1" | "component-v1"
        ) {
            bail!("unsupported runtime profile");
        }
        manifest.ensure_engine_compatible()?;
        if manifest.required_rom_count() != rom.as_ref().map_or(0, Vec::len) {
            bail!("ROM delivery requires every declared file selected by you");
        }
        if !manifest.requires_rom && rom.is_some() {
            bail!("this game does not request a user ROM");
        }
        if manifest.entry_world != "game" {
            bail!("unsupported runtime entry world");
        }
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config)?;
        if bytes.len() > 32 * 1024 * 1024 {
            bail!("compiled Wasm exceeds 32 MiB runtime limit");
        }
        let module = if manifest.runtime_profile == "core-v1" {
            Some(Module::new(&engine, bytes)?)
        } else {
            None
        };
        let component = if module.is_none() {
            Some(wasmtime::component::Component::new(&engine, bytes)?)
        } else {
            None
        };
        let (uses_shaders, custom_renderer) = if let Some(module) = &module {
            if module.imports().any(|i| {
                i.module() != "gamepkg"
                    || !matches!(
                        i.name(),
                        "asset_read"
                            | "rom_read"
                            | "rom_size"
                            | "rom_count"
                            | "rom_size_at"
                            | "rom_read_at"
                            | "save_read"
                            | "save_write"
                            | "save_size"
                            | "draw_clear"
                            | "draw_asset_image"
                            | "draw_image"
                            | "draw_shader_image"
                            | "draw_rect"
                            | "draw_circle"
                            | "draw_shader"
                            | "achievement_unlock"
                            | "audio_submit"
                    )
            }) {
                bail!("games cannot import functions outside the scoped GamePKG API");
            }
            let uses_shaders = module
                .imports()
                .any(|i| matches!(i.name(), "draw_shader" | "draw_shader_image"));
            let custom_renderer = module.imports().any(|i| i.name().starts_with("draw_"));
            (uses_shaders, custom_renderer)
        } else {
            let mut uses_shaders = false;
            let mut custom_renderer = false;
            for (_, item) in component
                .as_ref()
                .unwrap()
                .component_type()
                .imports(&engine)
            {
                if let wasmtime::component::types::ComponentItem::ComponentInstance(instance) = item
                {
                    uses_shaders |= instance.get_export(&engine, "draw-shader").is_some()
                        || instance.get_export(&engine, "draw-shader-image").is_some();
                    custom_renderer |= instance
                        .exports(&engine)
                        .any(|(name, _)| name.starts_with("draw-"));
                }
            }
            (uses_shaders, custom_renderer)
        };
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        let fuel = manifest.limits.fuel.min(settings.fuel_ceiling);
        let deadline_epochs = settings.tick_deadline_ms / 10;
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory as usize)
            .table_elements(10000)
            .instances(if module.is_some() { 1 } else { 4 })
            .memories(1)
            .tables(if module.is_some() { 1 } else { 2 })
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            &engine,
            Data {
                achievements: manifest.achievements.iter().map(|a| a.id.clone()).collect(),
                pending_achievements: Vec::new(),
                limits,
                package,
                rom,
                save,
                frame: crate::drawing::Frame::default(),
                audio: Vec::new(),
                audio_allowed: manifest.permissions.audio,
                shader_count: 0,
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
            if module.is_none() {
                let component = component.as_ref().unwrap();
                let instance =
                    component_api::linker(&engine)?.instantiate(&mut store, component)?;
                let tick = instance.get_typed_func::<(u32, u64), (u64,)>(&mut store, "tick")?;
                return Ok(Tick::Component(tick));
            }
            let mut linker = Linker::new(&engine);
            linker.func_wrap(
                "gamepkg",
                "achievement_unlock",
                |mut caller: Caller<'_, Data>, index: i32| -> i32 {
                    let Some(id) = caller.data().achievements.get(index as usize).cloned() else {
                        return 0;
                    };
                    let fuel = caller.get_fuel().unwrap_or(0);
                    if fuel < 64 {
                        return 0;
                    }
                    if caller.set_fuel(fuel - 64).is_err() {
                        return 0;
                    }
                    if !caller.data().pending_achievements.contains(&id) {
                        caller.data_mut().pending_achievements.push(id);
                    }
                    1
                },
            )?;

            linker.func_wrap("gamepkg", "asset_read", asset_read)?;
            linker.func_wrap("gamepkg", "audio_submit", audio_submit)?;
            linker.func_wrap(
                "gamepkg",
                "draw_asset_image",
                |mut caller: Caller<'_, Data>,
                 pointer: i32,
                 length: i32,
                 x: f32,
                 y: f32,
                 w: f32,
                 h: f32|
                 -> i32 {
                    if pointer < 0 || !(1..=1024).contains(&length) {
                        return -1;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return -1;
                    };
                    let Some(end) = (pointer as usize).checked_add(length as usize) else {
                        return -1;
                    };
                    let Some(bytes) = memory.data(&caller).get(pointer as usize..end) else {
                        return -1;
                    };
                    let Ok(path) = std::str::from_utf8(bytes) else {
                        return -1;
                    };
                    let path = path.to_owned();
                    let Ok(mut fuel) = caller.get_fuel() else {
                        return -1;
                    };
                    let result = draw_asset_image(caller.data_mut(), &mut fuel, &path, x, y, w, h);
                    if caller.set_fuel(fuel).is_err() {
                        return -1;
                    }
                    if result {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_image",
                |mut caller: Caller<'_, Data>,
                 x: f32,
                 y: f32,
                 w: f32,
                 h: f32,
                 pw: u32,
                 ph: u32,
                 pointer: i32,
                 length: i32|
                 -> i32 {
                    let Some(bytes) = crate::drawing::image::Image::byte_length(pw, ph) else {
                        return -1;
                    };
                    if pointer < 0
                        || length < 0
                        || length as usize != bytes
                        || !caller.data().frame.can_image(x, y, w, h, bytes)
                    {
                        return -1;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return -1;
                    };
                    let Some(end) = (pointer as usize).checked_add(bytes) else {
                        return -1;
                    };
                    if end > memory.data_size(&caller) {
                        return -1;
                    }
                    let Ok(fuel) = caller.get_fuel() else {
                        return -1;
                    };
                    let cost = bytes as u64 + 64;
                    if fuel < cost {
                        let _ = caller.set_fuel(0);
                        return -1;
                    }
                    if caller.set_fuel(fuel - cost).is_err() {
                        return -1;
                    }
                    let Some(image) = crate::drawing::image::Image::new(
                        pw,
                        ph,
                        &memory.data(&caller)[pointer as usize..end],
                    ) else {
                        return -1;
                    };
                    if caller.data_mut().frame.image(x, y, w, h, image) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_shader_image",
                |mut caller: Caller<'_, Data>,
                 index: i32,
                 pw: u32,
                 ph: u32,
                 pointer: i32,
                 length: i32|
                 -> i32 {
                    let (x, y, w, h) = (0.0, 0.0, 640.0, 480.0);
                    if index < 0 || index as usize >= caller.data().shader_count {
                        return -1;
                    }
                    let Some(bytes) = crate::drawing::image::Image::byte_length(pw, ph) else {
                        return -1;
                    };
                    if pointer < 0
                        || length < 0
                        || length as usize != bytes
                        || !caller.data().frame.can_image(x, y, w, h, bytes)
                    {
                        return -1;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return -1;
                    };
                    let Some(end) = (pointer as usize).checked_add(bytes) else {
                        return -1;
                    };
                    if end > memory.data_size(&caller) {
                        return -1;
                    }
                    let Ok(fuel) = caller.get_fuel() else {
                        return -1;
                    };
                    let cost = bytes as u64 + 64;
                    if fuel < cost {
                        let _ = caller.set_fuel(0);
                        return -1;
                    }
                    if caller.set_fuel(fuel - cost).is_err() {
                        return -1;
                    }
                    let Some(image) = crate::drawing::image::Image::new(
                        pw,
                        ph,
                        &memory.data(&caller)[pointer as usize..end],
                    ) else {
                        return -1;
                    };
                    let count = caller.data().shader_count;
                    if caller.data_mut().frame.shader_image(index, count, image) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_clear",
                |mut caller: Caller<'_, Data>, rgba: i32| -> i32 {
                    if !draw_fuel(&mut caller) {
                        return -1;
                    }
                    caller.data_mut().frame.reset();
                    caller.data_mut().frame.background = Some(rgba as u32);
                    0
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_rect",
                |mut caller: Caller<'_, Data>, x: f32, y: f32, w: f32, h: f32, rgba: i32| -> i32 {
                    if !draw_fuel(&mut caller) {
                        return -1;
                    }
                    if caller.data_mut().frame.rect(x, y, w, h, rgba as u32) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_circle",
                |mut caller: Caller<'_, Data>, x: f32, y: f32, r: f32, rgba: i32| -> i32 {
                    if !draw_fuel(&mut caller) {
                        return -1;
                    }
                    if caller.data_mut().frame.circle(x, y, r, rgba as u32) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_shader",
                |mut caller: Caller<'_, Data>, index: i32| -> i32 {
                    if !draw_fuel(&mut caller) {
                        return -1;
                    }
                    let count = caller.data().shader_count;
                    if caller.data_mut().frame.shader(index, count) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap("gamepkg", "rom_read", rom_read)?;
            linker.func_wrap("gamepkg", "rom_read_at", rom_read_at)?;
            linker.func_wrap("gamepkg", "rom_count", |caller: Caller<'_, Data>| -> i32 {
                caller.data().rom.as_ref().map_or(0, |r| r.len() as i32)
            })?;
            linker.func_wrap(
                "gamepkg",
                "rom_size_at",
                |caller: Caller<'_, Data>, index: i32| -> i32 {
                    caller
                        .data()
                        .rom
                        .as_ref()
                        .and_then(|r| r.get(index as usize))
                        .map_or(-1, |r| r.len() as i32)
                },
            )?;

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
                caller
                    .data()
                    .rom
                    .as_ref()
                    .and_then(|r| r.first())
                    .map_or(-1, |r| r.len() as i32)
            })?;
            let instance = linker.instantiate(&mut store, module.as_ref().unwrap())?;
            let tick = instance
                .get_typed_func::<(i32, i64), i64>(&mut store, "gamepkg_tick")
                .context("game has no grid-v1 entry point; use a playable example and rebuild")?;
            Ok(Tick::Core(tick))
        })();
        let tick = match result {
            Ok(t) => t,
            Err(e) => {
                stop.store(true, Ordering::Relaxed);
                return Err(e);
            }
        };
        Ok(Self {
            progress: None,
            store,
            tick,
            fuel,
            deadline_epochs,
            stop,
            state: 0,
            id: manifest.id.clone(),
            input_allowed: manifest.permissions.input,
            custom_renderer,
            shader_sources: Vec::new(),
            uses_shaders,
            audio_allowed: manifest.permissions.audio,
        })
    }
    pub fn load(root: &Path, id: &str) -> Result<Self> {
        Self::load_with_rom(root, id, None)
    }
    pub fn load_with_rom(root: &Path, id: &str, rom_path: Option<&Path>) -> Result<Self> {
        let paths: Vec<_> = rom_path.into_iter().map(Path::to_path_buf).collect();
        Self::load_with_roms(root, id, &paths)
    }
    pub fn load_with_roms(root: &Path, id: &str, rom_paths: &[std::path::PathBuf]) -> Result<Self> {
        crate::registry::inspect_library(root)?
            .games
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
        package.resolve_shared_assets(root)?;
        let cache = root
            .join("builds")
            .join(crate::verify::digest(&archive))
            .join("game.wasm");
        let bytes = std::fs::read(&cache).context("build this game before playing")?;
        crate::integrity::verify_build(&cache, &bytes)
            .context("build integrity unavailable or damaged; rebuild this game")?;
        if rom_paths.len() != manifest.required_rom_count() {
            bail!(
                "this game requires {} user-selected ROM files, but {} were supplied",
                manifest.required_rom_count(),
                rom_paths.len()
            );
        }
        let mut snapshots = Vec::new();
        let mut total = 0usize;
        for path in rom_paths {
            use std::io::Read;
            let file = std::fs::File::open(path).context("unable to open selected ROM")?;
            if !file.metadata()?.is_file() {
                bail!("ROM must be a regular file");
            }
            let mut bytes = Vec::new();
            file.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
            if bytes.is_empty() || bytes.len() > 64 * 1024 * 1024 {
                bail!("ROM must be 1 byte to 64 MiB");
            }
            total = total.saturating_add(bytes.len());
            if total > 128 * 1024 * 1024 {
                bail!("combined ROM snapshots exceed 128 MiB");
            }
            snapshots.push(bytes);
        }
        let rom = (!snapshots.is_empty()).then_some(snapshots);
        let settings = crate::settings::load()?;
        let mut game = Self::with_package(
            &bytes,
            &manifest,
            &settings,
            Some(package),
            rom,
            if manifest.permissions.save {
                Some(crate::saves::Save::open(
                    root,
                    &manifest.id,
                    &manifest.creator_key,
                    manifest.save_schema,
                )?)
            } else {
                None
            },
        )?;
        if game.uses_shaders {
            game.shader_sources =
                crate::shaders::load_fragment_sources(root, &archive, &manifest, &settings)?;
            game.store.data_mut().shader_count = game.shader_sources.len();
        }
        game.progress = Some(crate::progress::Session::open(
            root,
            &manifest.id,
            &manifest.creator_key,
        )?);
        Ok(game)
    }
    pub fn finish(&mut self) -> Result<()> {
        if let Some(progress) = &mut self.progress {
            progress.finish()?;
        }
        Ok(())
    }
    pub fn audio_frame(&self) -> &[i16] {
        &self.store.data().audio
    }
    pub fn frame(&self) -> &crate::drawing::Frame {
        &self.store.data().frame
    }
    pub fn step(&mut self, input: u32) -> Result<u64> {
        if let Some(progress) = &mut self.progress {
            progress.start();
        }
        self.store.data_mut().pending_achievements.clear();
        self.store.data_mut().frame.reset();
        self.store.data_mut().audio.clear();
        self.store.set_fuel(self.fuel)?;
        self.store.set_epoch_deadline(self.deadline_epochs);
        let previous_save = self.store.data().save.as_ref().map(|s| s.bytes.clone());
        let input = if self.input_allowed { input } else { 0 };
        let result: Result<u64> = match &self.tick {
            Tick::Core(tick) => tick
                .call(&mut self.store, (input as i32, self.state as i64))
                .map(|v| v as u64),
            Tick::Component(tick) => {
                tick.call(&mut self.store, (input, self.state))
                    .and_then(|(v,)| {
                        tick.post_return(&mut self.store)?;
                        Ok(v)
                    })
            }
        }
        .context("game stopped by the Wasm sandbox");
        self.state = match result {
            Ok(state) => state,
            Err(error) => {
                self.store.data_mut().frame.reset();
                self.store.data_mut().audio.clear();
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
        if let Some(progress) = &mut self.progress {
            progress.tick(&self.store.data().pending_achievements)?;
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
    let cost = caller.data_mut().package.as_mut().and_then(|p| {
        p.asset_chunk_cost(name, offset as u64, output_len as usize)
            .ok()
    });
    let Some(cost) = cost else {
        return -1;
    };
    // Compressed ranges also pay for decoding the prefix before their offset.
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
    let Ok(bytes) = package.asset_chunk(name, offset as u64, output_len as usize) else {
        return -1;
    };
    let length = bytes.len();
    if memory
        .write(&mut caller, output as usize, &bytes[..length])
        .is_err()
    {
        return -1;
    }
    length as i32
}

fn rom_read(caller: Caller<'_, Data>, offset: i32, output: i32, length: i32) -> i32 {
    rom_read_at(caller, 0, offset, output, length)
}
fn rom_read_at(
    mut caller: Caller<'_, Data>,
    index: i32,
    offset: i32,
    output: i32,
    length: i32,
) -> i32 {
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
        .and_then(|r| r.get(index as usize))
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

fn draw_asset_image(
    data: &mut Data,
    fuel: &mut u64,
    path: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    use crate::drawing::image::{Image, MAX_PNG_BYTES};
    if path.len() > 1024
        || !crate::archive::valid_path(path)
        || !data.frame.can_image(x, y, w, h, 0)
    {
        return false;
    }
    let Some(package) = data.package.as_mut() else {
        return false;
    };
    let Ok(size) = package.asset_size(path) else {
        return false;
    };
    if !(33..=MAX_PNG_BYTES as u64).contains(&size) {
        return false;
    }
    if *fuel < 1280 {
        *fuel = 0;
        return false;
    }
    *fuel -= 1280;
    let Ok(header) = package.asset_chunk(path, 0, 33) else {
        return false;
    };
    let Some(pixel_bytes) = Image::png_byte_length(&header) else {
        return false;
    };
    if !data.frame.can_image(x, y, w, h, pixel_bytes) {
        return false;
    }
    let mut cost = pixel_bytes as u64 * 4;
    for offset in (0..size).step_by(65536) {
        let Ok(chunk_cost) = package.asset_chunk_cost(path, offset, 65536) else {
            return false;
        };
        cost += chunk_cost;
    }
    if *fuel < cost {
        *fuel = 0;
        return false;
    }
    *fuel -= cost;
    let mut encoded = Vec::with_capacity(size as usize);
    for offset in (0..size).step_by(65536) {
        let Ok(chunk) = package.asset_chunk(path, offset, 65536) else {
            return false;
        };
        encoded.extend(chunk);
    }
    let Some(image) = Image::from_png(&encoded) else {
        return false;
    };
    if image.pixels().len() != pixel_bytes {
        return false;
    }
    data.frame.image(x, y, w, h, image)
}

fn draw_fuel(caller: &mut Caller<'_, Data>) -> bool {
    let Ok(fuel) = caller.get_fuel() else {
        return false;
    };
    if fuel < 64 {
        let _ = caller.set_fuel(0);
        return false;
    }
    caller.set_fuel(fuel - 64).is_ok()
}

/// Signed 16-bit little-endian mono PCM at 48 kHz; at most 1,600 samples per tick.
fn audio_submit(mut caller: Caller<'_, Data>, pointer: i32, count: i32) -> i32 {
    if !caller.data().audio_allowed
        || pointer < 0
        || !(0..=1600).contains(&count)
        || caller.data().audio.len() + count as usize > 1600
    {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let Some(end) = (pointer as usize).checked_add(count as usize * 2) else {
        return -1;
    };
    if end > memory.data_size(&caller) {
        return -1;
    }
    let Ok(fuel) = caller.get_fuel() else {
        return -1;
    };
    let cost = count as u64 * 2 + 64;
    if fuel < cost {
        let _ = caller.set_fuel(0);
        return -1;
    }
    if caller.set_fuel(fuel - cost).is_err() {
        return -1;
    }
    let mut bytes = vec![0; count as usize * 2];
    if memory.read(&caller, pointer as usize, &mut bytes).is_err() {
        return -1;
    }
    caller.data_mut().audio.extend(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes([b[0], b[1]])),
    );
    0
}
