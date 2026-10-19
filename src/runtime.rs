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
    clock: crate::frame_clock::Clock,
    window: crate::window_control::Service,
    user_files: crate::user_files::Service,
    input_events: crate::input_events::Queue,
    achievements: Vec<String>,
    pending_achievements: Vec<String>,
    limits: StoreLimits,
    package: Option<crate::archive::Package>,
    asset_image: crate::drawing::image::Cache,
    rom: Option<Vec<Vec<u8>>>,
    save: Option<crate::saves::Save>,
    frame: crate::drawing::Frame,
    audio: Vec<i16>,
    audio_channels: u32,
    audio_allowed: bool,
    effects: crate::audio_effects::Runtime,
    shader_count: usize,
    zelda: Option<crate::zelda3::Zelda>,
    doom: Option<crate::doom::Doom>,
    deadline_epochs: u64,
    network: crate::network::Service,
}
pub struct Game {
    queued_input: crate::input_events::Queue,
    doom_events: Vec<crate::doom_input::Event>,
    progress: Option<crate::progress::Session>,
    store: Option<Store<Data>>,
    tick: Option<Tick>,
    #[cfg(target_os = "linux")]
    native: Option<crate::native::Session>,
    #[cfg(target_os = "linux")]
    cube2: Option<Box<crate::cube2::player::Player>>,
    deadline_epochs: u64,
    stop: Arc<AtomicBool>,
    pub state: u64,
    pub id: String,
    input_allowed: bool,
    pub custom_renderer: bool,
    pub shader_sources: Vec<String>,
    uses_shaders: bool,
    pub audio_allowed: bool,
    pub builtin_engine: bool,
    pub doom_engine: bool,
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
        anyhow::ensure!(
            manifest.builtin_engine.as_deref() != Some(crate::cube2::PROFILE),
            "Cube 2 can only run from its verified native engine cache"
        );
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
        // The guest ABI is wasm32 regardless of the native host word size.
        config.wasm_memory64(false);
        config.wasm_relaxed_simd(false);
        config.cranelift_nan_canonicalization(true);
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
                !matches!(i.ty(), wasmtime::ExternType::Func(_))
                    || i.module() != "gamepkg"
                    || (i.name() == "zelda3_tick"
                        && manifest.builtin_engine.as_deref() != Some(crate::zelda3::PROFILE))
                    || (i.name() == "doom_tick"
                        && manifest.builtin_engine.as_deref() != Some(crate::doom::PROFILE))
                    || !matches!(
                        i.name(),
                        "network_call"
                            | "effect_call"
                            | "zelda3_tick"
                            | "doom_tick"
                            | "asset_read"
                            | "rom_read"
                            | "rom_size"
                            | "rom_count"
                            | "rom_size_at"
                            | "rom_read_at"
                            | "save_read"
                            | "save_write"
                            | "save_size"
                            | "random_u64"
                            | "frame_time_us"
                            | "window_state"
                            | "input_events"
                            | "file_call"
                            | "window_control"
                            | "draw_clear"
                            | "draw_asset_image"
                            | "draw_asset_image_linear"
                            | "draw_asset_image_linear_straight"
                            | "draw_image"
                            | "draw_image_linear"
                            | "draw_shader_image"
                            | "draw_shader_asset_image"
                            | "draw_clip"
                            | "draw_clip_reset"
                            | "draw_rect"
                            | "draw_circle"
                            | "draw_shader"
                            | "achievement_unlock"
                            | "audio_submit"
                            | "audio_submit_stereo"
                    )
            }) {
                bail!("games cannot import functions outside the scoped GamePKG API");
            }
            let uses_shaders = module.imports().any(|i| {
                matches!(
                    i.name(),
                    "draw_shader" | "draw_shader_image" | "draw_shader_asset_image"
                )
            });
            let custom_renderer = module.imports().any(|i| {
                i.name().starts_with("draw_") || matches!(i.name(), "zelda3_tick" | "doom_tick")
            });
            (uses_shaders, custom_renderer)
        } else {
            let mut uses_shaders = false;
            let mut custom_renderer = false;
            for (name, item) in component
                .as_ref()
                .unwrap()
                .component_type()
                .imports(&engine)
            {
                if name != "gamepkg:engine/host@0.1.0" {
                    bail!("games cannot import interfaces outside the scoped GamePKG API");
                }
                if let wasmtime::component::types::ComponentItem::ComponentInstance(instance) = item
                {
                    for (name, export) in instance.exports(&engine) {
                        if !matches!(
                            export,
                            wasmtime::component::types::ComponentItem::ComponentFunc(_)
                        ) || !matches!(
                            name,
                            "network-call"
                                | "effect-call"
                                | "asset-read"
                                | "rom-read"
                                | "rom-size"
                                | "rom-count"
                                | "rom-size-at"
                                | "rom-read-at"
                                | "save-read"
                                | "save-write"
                                | "save-size"
                                | "random-u64"
                                | "frame-time-us"
                                | "window-state"
                                | "input-events"
                                | "file-call"
                                | "window-control"
                                | "draw-clear"
                                | "draw-asset-image"
                                | "draw-asset-image-linear"
                                | "draw-asset-image-linear-straight"
                                | "draw-image"
                                | "draw-image-linear"
                                | "draw-shader-image"
                                | "draw-shader-asset-image"
                                | "draw-clip"
                                | "draw-clip-reset"
                                | "draw-rect"
                                | "draw-circle"
                                | "draw-shader"
                                | "achievement-unlock"
                                | "audio-submit"
                                | "audio-submit-stereo"
                        ) {
                            bail!("games cannot import functions outside the scoped GamePKG API");
                        }
                    }
                    uses_shaders |= instance.get_export(&engine, "draw-shader").is_some()
                        || instance.get_export(&engine, "draw-shader-image").is_some()
                        || instance
                            .get_export(&engine, "draw-shader-asset-image")
                            .is_some();
                    custom_renderer |= instance
                        .exports(&engine)
                        .any(|(name, _)| name.starts_with("draw-"));
                } else {
                    bail!("games cannot import non-interface host resources");
                }
            }
            (uses_shaders, custom_renderer)
        };
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        let memory = if let Some(profile) = manifest.builtin_engine.as_deref() {
            let reserved = crate::builtin::memory(profile)?;
            if memory < reserved + 16 * 1048576 {
                bail!("builtin engine exceeds the selected memory ceiling");
            }
            memory - reserved
        } else {
            memory
        };

        let (effects, memory) = crate::audio_effects::Runtime::new(
            package.as_ref(),
            manifest.limits.effects_memory_bytes,
            memory,
            manifest.permissions.audio,
        )?;

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
                clock: crate::frame_clock::Clock::default(),
                window: crate::window_control::Service::with_homepage(
                    manifest.permissions.input,
                    manifest.homepage.as_deref(),
                )?,
                user_files: crate::user_files::Service::new(manifest.permissions.files.clone()),
                input_events: crate::input_events::Queue::new(manifest.permissions.input),
                achievements: manifest.achievements.iter().map(|a| a.id.clone()).collect(),
                pending_achievements: Vec::new(),
                network: crate::network::Service::new(manifest.permissions.clone()),
                limits,
                package,
                asset_image: crate::drawing::image::Cache::default(),
                rom,
                save,
                frame: crate::drawing::Frame::default(),
                audio: Vec::new(),
                audio_channels: 1,
                audio_allowed: manifest.permissions.audio,
                effects,
                shader_count: 0,
                zelda: None,
                doom: None,
                deadline_epochs,
            },
        );
        store.limiter(|s| &mut s.limits);
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
            if manifest.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE) {
                linker.func_wrap("gamepkg", "zelda3_tick", zelda3_tick)?;
            }
            if manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE) {
                linker.func_wrap("gamepkg", "doom_tick", doom_tick)?;
            }
            linker.func_wrap(
                "gamepkg",
                "achievement_unlock",
                |mut caller: Caller<'_, Data>, index: i32| -> i32 {
                    let Some(id) = caller.data().achievements.get(index as usize).cloned() else {
                        return 0;
                    };

                    if !caller.data().pending_achievements.contains(&id) {
                        caller.data_mut().pending_achievements.push(id);
                    }
                    1
                },
            )?;

            linker.func_wrap("gamepkg", "effect_call", effect_call)?;
            linker.func_wrap("gamepkg", "asset_read", asset_read)?;
            linker.func_wrap(
                "gamepkg",
                "audio_submit",
                |caller: Caller<'_, Data>, p: i32, n: i32| audio_submit(caller, p, n, 1),
            )?;
            linker.func_wrap(
                "gamepkg",
                "audio_submit_stereo",
                |caller: Caller<'_, Data>, p: i32, n: i32| audio_submit(caller, p, n, 2),
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_shader_asset_image",
                |mut caller: Caller<'_, Data>, index: i32, pointer: i32, length: i32| -> i32 {
                    if index < 0 || pointer < 0 || !(1..=1024).contains(&length) {
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
                    if draw_shader_asset_image(caller.data_mut(), index, &path) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
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
                    let result = draw_asset_image(caller.data_mut(), &path, x, y, w, h);

                    if result {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            for (asset_name, straight) in [
                ("draw_asset_image_linear", false),
                ("draw_asset_image_linear_straight", true),
            ] {
                linker.func_wrap(
                    "gamepkg",
                    asset_name,
                    move |mut caller: Caller<'_, Data>,
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
                        let Some(memory) =
                            caller.get_export("memory").and_then(|e| e.into_memory())
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
                        let result = if straight {
                            draw_asset_image_linear_straight(caller.data_mut(), &path, x, y, w, h)
                        } else {
                            draw_asset_image_linear(caller.data_mut(), &path, x, y, w, h)
                        };

                        if result {
                            0
                        } else {
                            -1
                        }
                    },
                )?;
            }
            for (image_name, linear) in [("draw_image", false), ("draw_image_linear", true)] {
                linker.func_wrap(
                    "gamepkg",
                    image_name,
                    move |mut caller: Caller<'_, Data>,
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
                        let Some(memory) =
                            caller.get_export("memory").and_then(|e| e.into_memory())
                        else {
                            return -1;
                        };
                        let Some(end) = (pointer as usize).checked_add(bytes) else {
                            return -1;
                        };
                        if end > memory.data_size(&caller) {
                            return -1;
                        }

                        let Some(image) = crate::drawing::image::Image::new(
                            pw,
                            ph,
                            &memory.data(&caller)[pointer as usize..end],
                        ) else {
                            return -1;
                        };
                        let accepted = if linear {
                            caller.data_mut().frame.linear_image(x, y, w, h, image)
                        } else {
                            caller.data_mut().frame.image(x, y, w, h, image)
                        };
                        if accepted {
                            0
                        } else {
                            -1
                        }
                    },
                )?;
            }
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
                    caller.data_mut().frame.clear(rgba as u32);
                    0
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_clip",
                |mut caller: Caller<'_, Data>, x: f32, y: f32, w: f32, h: f32| -> i32 {
                    if caller.data_mut().frame.clip(Some([x, y, w, h])) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_clip_reset",
                |mut caller: Caller<'_, Data>| -> i32 {
                    if caller.data_mut().frame.clip(None) {
                        0
                    } else {
                        -1
                    }
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "draw_rect",
                |mut caller: Caller<'_, Data>, x: f32, y: f32, w: f32, h: f32, rgba: i32| -> i32 {
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
            linker.func_wrap(
                "gamepkg",
                "input_events",
                |mut caller: Caller<'_, Data>, pointer: i32, capacity: i32| -> i32 {
                    if pointer < 0
                        || capacity <= 0
                        || capacity as usize > crate::input_events::MAX_BYTES
                        || capacity % 16 != 0
                    {
                        return -1;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return -1;
                    };
                    if (pointer as usize)
                        .checked_add(capacity as usize)
                        .is_none_or(|end| end > memory.data_size(&caller))
                    {
                        return -1;
                    }
                    let Some(bytes) = caller.data_mut().input_events.read(capacity as usize) else {
                        return -1;
                    };
                    if memory.write(&mut caller, pointer as usize, &bytes).is_err() {
                        return -1;
                    }
                    bytes.len() as i32
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "file_call",
                |mut caller: Caller<'_, Data>,
                 operation: u32,
                 pointer: i32,
                 length: i32,
                 output: i32,
                 capacity: i32|
                 -> i32 {
                    if operation > 5
                        || pointer < 0
                        || length < 0
                        || length as usize > crate::user_files::MAX_REQUEST
                        || output < 0
                        || capacity < 0
                        || capacity as usize > crate::user_files::MAX_CHUNK
                    {
                        return -1;
                    }
                    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    else {
                        return -1;
                    };
                    let size = memory.data_size(&caller);
                    if (pointer as usize)
                        .checked_add(length as usize)
                        .is_none_or(|n| n > size)
                        || (output as usize)
                            .checked_add(capacity as usize)
                            .is_none_or(|n| n > size)
                    {
                        return -1;
                    }
                    let request = memory.data(&caller)
                        [pointer as usize..pointer as usize + length as usize]
                        .to_vec();
                    let Some(reply) =
                        caller
                            .data_mut()
                            .user_files
                            .call(operation, &request, capacity as usize)
                    else {
                        return -1;
                    };
                    if memory.write(&mut caller, output as usize, &reply).is_err() {
                        return -1;
                    }
                    reply.len() as i32
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "window_control",
                |mut caller: Caller<'_, Data>, operation: u32, value: u32| -> i32 {
                    i32::from(caller.data_mut().window.request(operation, value))
                },
            )?;
            linker.func_wrap(
                "gamepkg",
                "window_state",
                |caller: Caller<'_, Data>| -> u32 { caller.data().window.observation() },
            )?;
            linker.func_wrap(
                "gamepkg",
                "frame_time_us",
                |caller: Caller<'_, Data>| -> i64 { caller.data().clock.micros() as i64 },
            )?;
            linker.func_wrap("gamepkg", "random_u64", || -> Result<i64> {
                Ok(crate::random::value()? as i64)
            })?;
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
            linker.func_wrap("gamepkg", "network_call", network_call)?;
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
            doom_events: Vec::new(),
            queued_input: crate::input_events::Queue::new(manifest.permissions.input),
            progress: None,
            store: Some(store),
            tick: Some(tick),
            #[cfg(target_os = "linux")]
            native: None,
            #[cfg(target_os = "linux")]
            cube2: None,
            deadline_epochs,
            stop,
            state: 0,
            id: manifest.id.clone(),
            input_allowed: manifest.permissions.input,
            custom_renderer,
            shader_sources: Vec::new(),
            uses_shaders,
            audio_allowed: manifest.permissions.audio,
            builtin_engine: manifest.builtin_engine.is_some(),
            doom_engine: manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE),
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
        Self::load_target(root, id, rom_paths, None)
    }
    pub fn load_target(
        root: &Path,
        id: &str,
        rom_paths: &[std::path::PathBuf],
        native: Option<bool>,
    ) -> Result<Self> {
        crate::registry::installed_manifest(root, id)?;
        let _build_read = crate::build::BuildChange::read(root, id)?;
        let archive = crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false,
        )?;
        let mut snapshot = tempfile::NamedTempFile::new()?;
        use std::io::Write;
        snapshot.write_all(&archive)?;
        snapshot.flush()?;
        let mut package = crate::archive::Package::open(snapshot.path())?;
        let manifest = crate::verify::verify_embedded(&mut package)?;
        package.resolve_shared_assets(root)?;
        if manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE) {
            anyhow::ensure!(
                rom_paths.is_empty(),
                "Cube 2 uses its original game-data folder rather than ROM file arguments"
            );
            anyhow::ensure!(
                native != Some(false),
                "Cube 2 uses the native Build and Play actions; Wasm export is unavailable"
            );
            #[cfg(not(target_os = "linux"))]
            bail!("Cube 2 native process isolation is currently available on Linux");
            #[cfg(target_os = "linux")]
            {
                let settings = crate::settings::load()?;
                let player = crate::cube2::player::Player::load(
                    root,
                    &manifest,
                    &crate::verify::digest(&archive),
                    &settings,
                )?;
                return Ok(Self {
                    doom_events: Vec::new(),
                    queued_input: crate::input_events::Queue::new(manifest.permissions.input),
                    progress: Some(crate::progress::Session::open(
                        root,
                        &manifest.id,
                        &manifest.creator_key,
                    )?),
                    store: None,
                    tick: None,
                    native: None,
                    cube2: Some(Box::new(player)),
                    deadline_epochs: 0,
                    stop: Arc::new(AtomicBool::new(false)),
                    state: 0,
                    id: manifest.id.clone(),
                    input_allowed: true,
                    custom_renderer: true,
                    shader_sources: Vec::new(),
                    uses_shaders: false,
                    audio_allowed: true,
                    builtin_engine: true,
                    doom_engine: false,
                });
            }
        }

        let cache = root
            .join("builds")
            .join(crate::verify::digest(&archive))
            .join("game.wasm");
        if rom_paths.len() != manifest.required_rom_count() {
            bail!(
                "this game requires {} ROM files selected by you, but {} were supplied",
                manifest.required_rom_count(),
                rom_paths.len()
            );
        }
        let mut snapshots = Vec::new();
        let mut total = 0usize;
        for (index, path) in rom_paths.iter().enumerate() {
            let title = manifest
                .roms
                .get(index)
                .map(|r| r.title.as_str())
                .unwrap_or("Your ROM file");
            let bytes = crate::files::read_regular(path, 64 * 1024 * 1024, true)
                .with_context(||format!("unable to open selected ROM {} ({title}); ROM must be 1 byte to 64 MiB and a regular file",index+1))?;
            if bytes.is_empty() || bytes.len() > 64 * 1024 * 1024 {
                bail!("ROM {} ({title}) must be 1 byte to 64 MiB", index + 1);
            }
            total = total.saturating_add(bytes.len());
            if total > 128 * 1024 * 1024 {
                bail!("combined ROM snapshots exceed 128 MiB");
            }
            snapshots.push(bytes);
        }
        let rom = (!snapshots.is_empty()).then_some(snapshots);
        let settings = crate::settings::load()?;
        let native = native.unwrap_or(
            crate::native::cache_path(root, id)?
                .symlink_metadata()
                .is_ok(),
        );
        if native {
            #[cfg(not(target_os = "linux"))]
            bail!("native game isolation is not available on this platform");
            #[cfg(target_os = "linux")]
            {
                let mut uses_shaders = false;
                let mut custom_renderer = manifest.builtin_engine.is_some();
                for name in package
                    .entries
                    .keys()
                    .filter(|name| name.starts_with("source/src/") && name.ends_with(".rs"))
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    let source = String::from_utf8(package.read(&name)?)?;
                    let (draw, shader) = crate::native::source_flags(&source)?;
                    uses_shaders |= shader;
                    custom_renderer |= draw;
                }
                let shader_sources = if uses_shaders {
                    crate::shaders::load_desktop_sources(root, &archive, &manifest, &settings)?
                } else {
                    Vec::new()
                };
                let session = crate::native::Session::new(
                    root,
                    &crate::verify::digest(&archive),
                    package,
                    manifest.clone(),
                    rom.unwrap_or_default(),
                    &settings,
                    shader_sources.len(),
                )?;
                return Ok(Self {
                    doom_events: Vec::new(),
                    queued_input: crate::input_events::Queue::new(manifest.permissions.input),
                    progress: Some(crate::progress::Session::open(
                        root,
                        &manifest.id,
                        &manifest.creator_key,
                    )?),
                    store: None,
                    tick: None,
                    native: Some(session),
                    cube2: None,
                    deadline_epochs: 0,
                    stop: Arc::new(AtomicBool::new(false)),
                    state: 0,
                    id: manifest.id.clone(),
                    input_allowed: manifest.permissions.input,
                    custom_renderer,
                    shader_sources,
                    uses_shaders,
                    audio_allowed: manifest.permissions.audio,
                    builtin_engine: manifest.builtin_engine.is_some(),
                    doom_engine: manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE),
                });
            }
        }
        let bytes = crate::files::read_regular(&cache, 32 * 1024 * 1024, false)
            .context("build this game before playing")?;
        crate::integrity::verify_build(&cache, &bytes)
            .context("build integrity unavailable or damaged; rebuild this game")?;
        let mut game = Self::with_package(
            &bytes,
            &manifest,
            &settings,
            Some(package),
            rom,
            if manifest.permissions.save {
                Some(crate::saves::Save::open_for_game(root, &manifest)?)
            } else {
                None
            },
        )?;
        if manifest.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE) {
            let path = cache.with_file_name("zelda3.wasm");
            let bytes = crate::files::read_regular(&path, 32 * 1048576, false)
                .context("rebuild the pinned Zelda3 engine before playing")?;
            crate::integrity::verify_build(&path, &bytes)?;
            let data = game.store.as_ref().unwrap().data();
            let rom = data
                .rom
                .as_ref()
                .and_then(|roms| roms.first())
                .context("select your Zelda3 ROM")?;
            let save = data.save.as_ref().map(|s| s.bytes.as_slice());
            let zelda = crate::zelda3::Zelda::new(
                game.store.as_ref().unwrap().engine(),
                &bytes,
                rom,
                save,
                game.deadline_epochs,
            )?;
            game.store.as_mut().unwrap().data_mut().zelda = Some(zelda);
        }
        if manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE) {
            let path = cache.with_file_name("doom.wasm");
            let bytes = crate::files::read_regular(&path, 32 * 1048576, false)
                .context("rebuild the pinned DOOM engine before playing")?;
            crate::integrity::verify_build(&path, &bytes)?;
            let data = game.store.as_ref().unwrap().data();
            let wad = data
                .rom
                .as_ref()
                .and_then(|roms| roms.first())
                .context("select your DOOM WAD")?;
            let save = data.save.as_ref().map(|s| s.bytes.as_slice());
            let doom = crate::doom::Doom::new(
                game.store.as_ref().unwrap().engine(),
                &bytes,
                wad,
                save,
                game.deadline_epochs,
            )?;
            game.store.as_mut().unwrap().data_mut().doom = Some(doom);
        }
        if game.uses_shaders {
            game.shader_sources =
                crate::shaders::load_desktop_sources(root, &archive, &manifest, &settings)?;
            game.store.as_mut().unwrap().data_mut().shader_count = game.shader_sources.len();
        }
        game.progress = Some(crate::progress::Session::open(
            root,
            &manifest.id,
            &manifest.creator_key,
        )?);
        Ok(game)
    }
    pub fn is_cube2(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.cube2.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }
    pub fn cube2_resize(&mut self, width: u32, height: u32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.resize(width, height);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (width, height);
    }
    pub fn cube2_frame_size(&self) -> Option<(u32, u32)> {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            return Some(cube.frame_size);
        }
        None
    }
    pub fn cube2_desktop_input(&mut self, buttons: u32, dx: i32, dy: i32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.desktop(buttons, dx, dy);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (buttons, dx, dy);
    }
    pub fn cube2_clear_input(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.clear_input();
        }
    }
    /// Let a reviewed native engine progress independently of a UI render loop.
    pub fn background_player(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.background(self.progress.take())?;
        }
        Ok(())
    }
    #[cfg(all(feature = "audio", feature = "gui"))]
    pub(crate) fn cube2_audio_sink(&self, sink: Option<crate::audio::Sink>) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            cube.audio_sink(sink);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = sink;
    }
    pub fn cube2_key(&mut self, scancode: u32, pressed: bool, repeat: bool, modifiers: u32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.key(scancode, pressed, repeat, modifiers);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (scancode, pressed, repeat, modifiers);
    }
    pub fn cube2_text(&mut self, text: &str) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.text(text);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = text;
    }
    pub fn cube2_modifiers(&mut self, modifiers: u32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.modifiers(modifiers);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = modifiers;
    }
    pub fn cube2_mouse_button(&mut self, button: u32, pressed: bool, modifiers: u32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.mouse_button(button, pressed, modifiers);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (button, pressed, modifiers);
    }
    pub fn cube2_wheel(&mut self, x: i32, y: i32, modifiers: u32) {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.wheel(x, y, modifiers);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (x, y, modifiers);
    }
    pub fn exited(&self) -> bool {
        if self.window_state().closed {
            return true;
        }
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            return cube.exited;
        }
        #[cfg(target_os = "linux")]
        if let Some(native) = &self.native {
            return native.exited;
        }
        self.store
            .as_ref()
            .and_then(|s| s.data().doom.as_ref())
            .is_some_and(|doom| doom.exited)
    }
    pub fn finish(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            let save = cube.finish();
            let progress = if let Some(progress) = &mut self.progress {
                progress.finish().context("Unable to save play history")
            } else {
                Ok(())
            };
            save?;
            progress?;
            return Ok(());
        }
        if let Some(progress) = &mut self.progress {
            progress.finish().context("Unable to save play history")?;
        }
        Ok(())
    }
    /// Read-only builtin engine state for playback diagnostics.
    pub fn builtin_controller(&mut self) -> Result<Option<u32>> {
        if self.is_cube2() {
            return Ok(None);
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = &self.native {
            return Ok(n.diagnostic.map(|d| d.2));
        }
        if let Some(doom) = self.store.as_mut().unwrap().data_mut().doom.as_mut() {
            return doom.controller().map(Some);
        }
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .zelda
            .as_mut()
            .map(crate::zelda3::Zelda::controller)
            .transpose()
    }
    pub fn builtin_state(&mut self) -> Result<Option<(u32, u16, u16)>> {
        if self.is_cube2() {
            return Ok(None);
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = &self.native {
            return Ok(n.diagnostic.map(|d| (d.0, d.1 as u16, (d.1 >> 16) as u16)));
        }
        if let Some(doom) = self.store.as_mut().unwrap().data_mut().doom.as_mut() {
            return doom.state().map(Some);
        }
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .zelda
            .as_mut()
            .map(crate::zelda3::Zelda::state)
            .transpose()
    }
    pub fn audio_channels(&self) -> u32 {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            return cube.audio_channels;
        }
        #[cfg(target_os = "linux")]
        if let Some(native) = &self.native {
            return native.audio_channels;
        }
        self.store.as_ref().unwrap().data().audio_channels
    }
    pub fn audio_frame(&self) -> &[i16] {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            return &cube.audio;
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = &self.native {
            return &n.audio;
        }
        &self.store.as_ref().unwrap().data().audio
    }
    pub fn frame(&self) -> &crate::drawing::Frame {
        #[cfg(target_os = "linux")]
        if let Some(cube) = &self.cube2 {
            return &cube.frame;
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = &self.native {
            return &n.frame;
        }
        &self.store.as_ref().unwrap().data().frame
    }
    pub fn doom_input_allowed(&self) -> bool {
        self.doom_engine && self.input_allowed
    }
    pub fn doom_release_input(&mut self) -> Result<()> {
        self.doom_events.clear();
        self.doom_input(crate::doom_input::Event::Release)
    }
    pub fn doom_input(&mut self, event: crate::doom_input::Event) -> Result<()> {
        anyhow::ensure!(
            self.doom_engine && self.input_allowed,
            "DOOM desktop input access denied"
        );
        event.words()?;
        if let (
            Some(crate::doom_input::Event::Mouse {
                buttons: old_buttons,
                dx: old_dx,
                dy: old_dy,
            }),
            crate::doom_input::Event::Mouse { buttons, dx, dy },
        ) = (self.doom_events.last_mut(), event)
        {
            if *old_buttons == buttons {
                if let (Some(x), Some(y)) = (old_dx.checked_add(dx), old_dy.checked_add(dy)) {
                    *old_dx = x;
                    *old_dy = y;
                    return Ok(());
                }
            }
        }
        anyhow::ensure!(
            self.doom_events.len() < crate::doom_input::MAX_QUEUED,
            "DOOM desktop input queue is full"
        );
        self.doom_events.push(event);
        Ok(())
    }
    pub fn input_event(&mut self, event: crate::input_events::Event) -> Result<()> {
        anyhow::ensure!(
            self.queued_input.push(event),
            "game input is denied or invalid"
        );
        Ok(())
    }
    pub fn input_events_allowed(&self) -> bool {
        self.input_allowed && !self.builtin_engine
    }
    fn user_files_mut(&mut self) -> Option<&mut crate::user_files::Service> {
        #[cfg(target_os = "linux")]
        if let Some(native) = &mut self.native {
            return Some(&mut native.user_files);
        }
        self.store
            .as_mut()
            .map(|store| &mut store.data_mut().user_files)
    }
    pub fn observe_fullscreen(&mut self, fullscreen: Option<bool>) {
        #[cfg(target_os = "linux")]
        if let Some(native) = &mut self.native {
            native.window.observe_fullscreen(fullscreen);
            return;
        }
        if let Some(store) = &mut self.store {
            store.data_mut().window.observe_fullscreen(fullscreen);
        }
    }
    pub fn window_state(&self) -> crate::window_control::State {
        #[cfg(target_os = "linux")]
        if let Some(native) = &self.native {
            return native.window.state();
        }
        self.store
            .as_ref()
            .map(|store| store.data().window.state())
            .unwrap_or_default()
    }
    /// Consume one committed request for the signed manifest homepage.
    pub fn take_homepage_url(&mut self) -> Option<String> {
        #[cfg(target_os = "linux")]
        if let Some(native) = &mut self.native {
            return native.window.take_homepage();
        }
        self.store.as_mut()?.data_mut().window.take_homepage()
    }
    fn effects_mut(&mut self) -> Option<&mut crate::audio_effects::Runtime> {
        #[cfg(target_os = "linux")]
        if let Some(native) = &mut self.native {
            return Some(&mut native.effects);
        }
        self.store
            .as_mut()
            .map(|store| &mut store.data_mut().effects)
    }
    /// Trusted desktop output attachment. Game source cannot supply callbacks.
    pub fn attach_effect_output(&mut self, publisher: crate::audio_effects::Publisher) {
        if let Some(effects) = self.effects_mut() {
            effects.attach(publisher);
        }
    }
    pub fn effect_playback(&mut self) -> Result<Option<crate::audio_mix_stream::Playback>> {
        self.effects_mut()
            .map_or(Ok(None), |effects| effects.playback())
    }
    pub fn enable_user_files(&mut self) {
        if let Some(files) = self.user_files_mut() {
            files.enable();
        }
    }
    pub fn take_user_file_job(&mut self) -> Option<crate::user_files::Job> {
        if self.exited() {
            return None;
        }
        self.user_files_mut()?.take_job()
    }
    pub fn complete_user_file(
        &mut self,
        job: &crate::user_files::Job,
        outcome: crate::user_files::Outcome,
    ) -> bool {
        self.user_files_mut()
            .is_some_and(|files| files.complete(job, outcome))
    }
    pub fn step(&mut self, input: u32) -> Result<u64> {
        let input_events = self.queued_input.drain_events();
        let file_gesture =
            crate::input_events::gesture(&input_events, if self.input_allowed { input } else { 0 });
        if let Some(files) = self.user_files_mut() {
            files.begin(file_gesture);
        }
        let events: Vec<_> = self
            .doom_events
            .drain(..self.doom_events.len().min(crate::doom_input::MAX_PER_TICK))
            .collect();
        if let Some(progress) = &mut self.progress {
            progress.start();
        }
        #[cfg(target_os = "linux")]
        if let Some(cube) = &mut self.cube2 {
            cube.step(input)?;
            self.state = self.state.wrapping_add(1);
            if let Some(progress) = &mut self.progress {
                progress.tick(&[])?;
            }
            return Ok(self.state);
        }
        #[cfg(target_os = "linux")]
        if let Some(n) = &mut self.native {
            n.window.begin(file_gesture);
            n.set_input_events(input_events);
            self.state = n.step(
                if self.input_allowed { input } else { 0 },
                self.state,
                &events,
            )?;
            if let Some(progress) = &mut self.progress {
                if let Err(error) = progress.tick(&n.achievements) {
                    n.frame.reset();
                    n.audio.clear();
                    n.audio_channels = 1;
                    n.achievements.clear();
                    n.user_files.finish(false);
                    n.window.finish(false);
                    let _ = n.effects.finish(false);
                    return Err(error);
                }
            }
            if let Err(error) = n.effects.finish(true) {
                n.frame.reset();
                n.audio.clear();
                n.audio_channels = 1;
                n.window.finish(false);
                n.user_files.finish(false);
                return Err(error);
            }
            if n.window.state().closed {
                n.user_files.shutdown();
            }
            return Ok(self.state);
        }
        self.store.as_mut().unwrap().data_mut().effects.begin()?;
        self.store.as_mut().unwrap().data_mut().clock.begin();
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .window
            .begin(file_gesture);
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .input_events
            .begin(input_events);
        if !events.is_empty() {
            self.store
                .as_mut()
                .unwrap()
                .data_mut()
                .doom
                .as_mut()
                .context("DOOM engine is unavailable")?
                .input_events(&events, self.deadline_epochs)?;
        }
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .pending_achievements
            .clear();
        self.store.as_mut().unwrap().data_mut().frame.reset();
        self.store.as_mut().unwrap().data_mut().audio.clear();
        self.store.as_mut().unwrap().data_mut().audio_channels = 1;
        self.store
            .as_mut()
            .unwrap()
            .set_epoch_deadline(self.deadline_epochs);
        let previous_save = self
            .store
            .as_ref()
            .unwrap()
            .data()
            .save
            .as_ref()
            .map(|s| s.bytes.clone());
        let input = if self.input_allowed { input } else { 0 };
        let result: Result<u64> = match self.tick.as_ref().unwrap() {
            Tick::Core(tick) => tick
                .call(
                    self.store.as_mut().unwrap(),
                    (input as i32, self.state as i64),
                )
                .map(|v| v as u64),
            Tick::Component(tick) => tick
                .call(self.store.as_mut().unwrap(), (input, self.state))
                .and_then(|(v,)| {
                    tick.post_return(self.store.as_mut().unwrap())?;
                    Ok(v)
                }),
        }
        .context("game stopped by the Wasm sandbox");
        self.state = match result {
            Ok(state) => state,
            Err(error) => {
                self.store.as_mut().unwrap().data_mut().frame.reset();
                self.store.as_mut().unwrap().data_mut().audio.clear();
                self.store.as_mut().unwrap().data_mut().audio_channels = 1;
                if let (Some(save), Some(bytes)) = (
                    self.store.as_mut().unwrap().data_mut().save.as_mut(),
                    previous_save,
                ) {
                    save.bytes = bytes;
                    save.dirty = false;
                }
                self.store
                    .as_mut()
                    .unwrap()
                    .data_mut()
                    .user_files
                    .finish(false);
                self.store.as_mut().unwrap().data_mut().window.finish(false);
                let _ = self
                    .store
                    .as_mut()
                    .unwrap()
                    .data_mut()
                    .effects
                    .finish(false);
                return Err(error);
            }
        };
        let commit = self
            .store
            .as_mut()
            .unwrap()
            .data_mut()
            .save
            .as_mut()
            .map(|save| save.commit())
            .transpose();
        if let Err(error) = commit {
            self.store.as_mut().unwrap().data_mut().frame.reset();
            self.store.as_mut().unwrap().data_mut().audio.clear();
            self.store.as_mut().unwrap().data_mut().audio_channels = 1;
            self.store
                .as_mut()
                .unwrap()
                .data_mut()
                .user_files
                .finish(false);
            self.store.as_mut().unwrap().data_mut().window.finish(false);
            let _ = self
                .store
                .as_mut()
                .unwrap()
                .data_mut()
                .effects
                .finish(false);
            return Err(error).context("unable to persist game save");
        }
        if let Some(progress) = &mut self.progress {
            if let Err(error) =
                progress.tick(&self.store.as_ref().unwrap().data().pending_achievements)
            {
                let data = self.store.as_mut().unwrap().data_mut();
                data.frame.reset();
                data.audio.clear();
                data.audio_channels = 1;
                data.pending_achievements.clear();
                self.store
                    .as_mut()
                    .unwrap()
                    .data_mut()
                    .user_files
                    .finish(false);
                self.store.as_mut().unwrap().data_mut().window.finish(false);
                let _ = self
                    .store
                    .as_mut()
                    .unwrap()
                    .data_mut()
                    .effects
                    .finish(false);
                return Err(error);
            }
        }
        if let Err(error) = self.store.as_mut().unwrap().data_mut().effects.finish(true) {
            let data = self.store.as_mut().unwrap().data_mut();
            data.frame.reset();
            data.audio.clear();
            data.audio_channels = 1;
            data.user_files.finish(false);
            data.window.finish(false);
            return Err(error);
        }
        self.store
            .as_mut()
            .unwrap()
            .data_mut()
            .user_files
            .finish(true);
        self.store.as_mut().unwrap().data_mut().window.finish(true);
        if self.window_state().closed {
            self.store
                .as_mut()
                .unwrap()
                .data_mut()
                .user_files
                .shutdown();
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

fn draw_asset_image(data: &mut Data, path: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    let Some(image) = decode_asset_image(data, path, x, y, w, h) else {
        return false;
    };
    data.frame.image_arc(x, y, w, h, image, false)
}
fn draw_asset_image_linear(data: &mut Data, path: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    let Some(image) = decode_asset_image(data, path, x, y, w, h) else {
        return false;
    };
    data.frame.image_arc(x, y, w, h, image, true)
}
fn draw_asset_image_linear_straight(
    data: &mut Data,
    path: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    let Some(image) = decode_asset_image(data, path, x, y, w, h) else {
        return false;
    };
    data.frame.straight_linear_image(x, y, w, h, image)
}
fn draw_shader_asset_image(data: &mut Data, index: i32, path: &str) -> bool {
    if index < 0 || index as usize >= data.shader_count {
        return false;
    }
    let Some(image) = decode_asset_image(data, path, 0.0, 0.0, 640.0, 480.0) else {
        return false;
    };
    data.frame
        .shader_image(index, data.shader_count, image.as_ref().clone())
}
fn decode_asset_image(
    data: &mut Data,
    path: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Option<std::sync::Arc<crate::drawing::image::Image>> {
    crate::drawing::image::asset(
        data.package.as_mut()?,
        &mut data.asset_image,
        &data.frame,
        path,
        [x, y, w, h],
    )
}

/// Signed 16-bit PCM at 48 kHz; mono/stereo share 1,600 sample values per tick.
fn effect_call(
    mut caller: Caller<'_, Data>,
    operation: u32,
    pointer: i32,
    length: i32,
    value: u32,
) -> i32 {
    if pointer < 0 || !(0..=1024).contains(&length) {
        return 0;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return 0;
    };
    let mut path = [0u8; 1024];
    if memory
        .read(&caller, pointer as usize, &mut path[..length as usize])
        .is_err()
    {
        return 0;
    }
    let Ok(path) = std::str::from_utf8(&path[..length as usize]) else {
        return 0;
    };
    i32::from(caller.data_mut().effects.call(operation, path, value))
}

fn audio_submit(mut caller: Caller<'_, Data>, pointer: i32, count: i32, channels: u32) -> i32 {
    if caller.data().effects.enabled()
        || !caller.data().audio_allowed
        || !matches!(channels, 1 | 2)
        || (!caller.data().audio.is_empty() && caller.data().audio_channels != channels)
        || pointer < 0
        || count % channels as i32 != 0
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

    let mut bytes = vec![0; count as usize * 2];
    if memory.read(&caller, pointer as usize, &mut bytes).is_err() {
        return -1;
    }
    let samples: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    let data = caller.data_mut();
    if crate::audio_frame::append(
        &mut data.audio,
        &mut data.audio_channels,
        &samples,
        channels,
        data.audio_allowed,
    ) {
        0
    } else {
        -1
    }
}

fn doom_tick(mut caller: Caller<'_, Data>, input: i32, state: i64) -> Result<i64> {
    let deadline = caller.data().deadline_epochs;
    let frame = caller
        .data_mut()
        .doom
        .as_mut()
        .context("DOOM engine is not initialized")?
        .step(input as u32, state == 0, deadline)?;
    let data = caller.data_mut();
    data.frame.background = Some(0x000000ff);
    let image = crate::drawing::image::Image::new(320, 200, &frame.pixels)
        .context("invalid DOOM engine image")?;
    if !data.frame.image(
        0.0,
        if data.doom.as_ref().is_some_and(|doom| doom.ending) {
            40.0
        } else {
            0.0
        },
        640.0,
        if data.doom.as_ref().is_some_and(|doom| doom.ending) {
            400.0
        } else {
            480.0
        },
        image,
    ) {
        bail!("DOOM image exceeds the frame budget");
    }
    if data.audio_allowed {
        data.audio = frame.audio;
        data.audio_channels = frame.audio_channels;
    }
    if let Some(bytes) = frame.save {
        let save = data
            .save
            .as_mut()
            .context("DOOM requires scoped save access")?;
        save.bytes = bytes;
        save.dirty = true;
    }
    Ok((state as u64).wrapping_add(1) as i64)
}

fn zelda3_tick(mut caller: Caller<'_, Data>, input: i32, state: i64) -> Result<i64> {
    let deadline = caller.data().deadline_epochs;
    let frame = caller
        .data_mut()
        .zelda
        .as_mut()
        .context("Zelda3 engine is not initialized")?
        .step(input as u32, state == 0, deadline)?;
    let data = caller.data_mut();
    data.frame.background = Some(0x000000ff);
    let image = crate::drawing::image::Image::new(256, 224, &frame.pixels)
        .context("invalid Zelda3 engine image")?;
    if !data.frame.image(64.0, 16.0, 512.0, 448.0, image) {
        bail!("Zelda3 image exceeds the host frame budget");
    }
    if data.audio_allowed {
        data.audio = frame.audio;
        data.audio_channels = frame.audio_channels;
    }
    if let Some(bytes) = frame.save {
        let save = data
            .save
            .as_mut()
            .context("Zelda3 requires scoped save access")?;
        save.bytes = bytes;
        save.dirty = true;
    }
    Ok((state as u64).wrapping_add(1) as i64)
}

fn network_call(
    mut caller: Caller<'_, Data>,
    operation: i32,
    input: i32,
    length: i32,
    output: i32,
    capacity: i32,
) -> i32 {
    if input < 0
        || output < 0
        || !(0..=65540).contains(&length)
        || !(0..=65536).contains(&capacity)
        || !(0..=9).contains(&operation)
    {
        return -1;
    }
    let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
        return -1;
    };
    let mut request = vec![0; length as usize];
    if memory.read(&caller, input as usize, &mut request).is_err() {
        return -1;
    }
    if (output as usize)
        .checked_add(capacity as usize)
        .is_none_or(|end| end > memory.data_size(&caller))
    {
        return -1;
    }
    let reply = caller
        .data_mut()
        .network
        .call(operation as u8, &request, capacity as usize);
    match reply.first() {
        Some(0) => {
            if reply.len() - 1 > capacity as usize
                || memory
                    .write(&mut caller, output as usize, &reply[1..])
                    .is_err()
            {
                -1
            } else {
                (reply.len() - 1) as i32
            }
        }
        Some(2) => -2,
        Some(3) => -3,
        _ => -1,
    }
}

#[cfg(test)]
mod adversarial_assets_tests {
    use super::*;
    mod cases {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/path_cases.rs"
        ));
    }

    #[test]
    fn raw_guest_imports_deny_path_and_pointer_attacks_without_changing_output() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("game.gamepkg");
        crate::pack::pack(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
            &archive,
            &[29; 32],
        )
        .unwrap();
        let mut package = crate::archive::Package::open(&archive).unwrap();
        let manifest = crate::verify::verify_embedded(&mut package).unwrap();
        let mut data = String::from("(data (i32.const 0) \"hello.txt\")");
        let mut code = String::from("i32.const 0 i32.const 9 i32.const 0 i32.const 60000 i32.const 5 call $read i32.const 5 i32.ne if unreachable end\n");
        // Establish working asset access before adversarial calls. Each rejected
        // call must leave this sentinel intact, including its full output range.
        code.push_str("i32.const 60000 i64.const 123456789 i64.store\n");
        let mut position = 256;
        for bytes in cases::rejected_paths()
            .into_iter()
            .chain(cases::literal_paths().iter().map(|p| p.to_string()))
            .map(String::into_bytes)
            .chain([vec![0xff], vec![0xc0, 0xaf], vec![b'a'; 1025]])
        {
            let escaped = bytes
                .iter()
                .map(|byte| format!("\\{byte:02x}"))
                .collect::<String>();
            data.push_str(&format!("(data (i32.const {position}) \"{escaped}\")\n"));
            code.push_str(&format!("i32.const {position} i32.const {} i32.const 0 i32.const 60000 i32.const 8 call $read i32.const -1 i32.ne if unreachable end\n", bytes.len()));
            code.push_str(&format!("i32.const {position} i32.const {} f32.const 0 f32.const 0 f32.const 1 f32.const 1 call $image i32.const -1 i32.ne if unreachable end\n", bytes.len()));
            code.push_str(&format!("i32.const {position} i32.const {} f32.const 0 f32.const 0 f32.const 1 f32.const 1 call $linear i32.const -1 i32.ne if unreachable end\n", bytes.len()));
            code.push_str(&format!("i32.const 0 i32.const {position} i32.const {} call $texture i32.const -1 i32.ne if unreachable end\n", bytes.len()));
            code.push_str(
                "i32.const 60000 i64.load i64.const 123456789 i64.ne if unreachable end\n",
            );
            position += bytes.len().max(1);
        }
        assert!(position < 60000);
        for (pointer, length, offset, output, count) in [
            (-1, 9, 0, 60000, 8),
            (i32::MAX, 9, 0, 60000, 8),
            (65535, 9, 0, 60000, 8),
            (0, -1, 0, 60000, 8),
            (0, 0, 0, 60000, 8),
            (0, 1025, 0, 60000, 8),
            (0, 9, -1, 60000, 8),
            (0, 9, i32::MAX, 60000, 8),
            (0, 9, 0, -1, 8),
            (0, 9, 0, i32::MAX, 8),
            (0, 9, 0, 65535, 8),
            (0, 9, 0, 60000, -1),
            (0, 9, 0, 0, 65537),
        ] {
            code.push_str(&format!("i32.const {pointer} i32.const {length} i32.const {offset} i32.const {output} i32.const {count} call $read i32.const -1 i32.ne if unreachable end\n"));
            code.push_str(
                "i32.const 60000 i64.load i64.const 123456789 i64.ne if unreachable end\n",
            );
        }
        let wasm = wat::parse_str(format!(r#"(module
            (import "gamepkg" "asset_read" (func $read (param i32 i32 i32 i32 i32) (result i32)))
            (import "gamepkg" "draw_shader_asset_image" (func $texture (param i32 i32 i32) (result i32)))
            (import "gamepkg" "draw_asset_image" (func $image (param i32 i32 f32 f32 f32 f32) (result i32)))
            (import "gamepkg" "draw_asset_image_linear" (func $linear (param i32 i32 f32 f32 f32 f32) (result i32)))
            (memory (export "memory") 1) {data}
            (func (export "gamepkg_tick") (param i32 i64) (result i64) {code} i64.const 42))"#)).unwrap();
        let mut game = Game::with_package(
            &wasm,
            &manifest,
            &Default::default(),
            Some(package),
            None,
            None,
        )
        .unwrap();
        game.store.as_mut().unwrap().data_mut().shader_count = 1;
        assert_eq!(game.step(0).unwrap(), 42);
        assert!(game.frame().commands.is_empty());
    }

    #[test]
    fn typed_component_imports_apply_the_same_path_policy() {
        let temp = tempfile::tempdir().unwrap();
        let archive = temp.path().join("game.gamepkg");
        crate::pack::pack(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
            &archive,
            &[29; 32],
        )
        .unwrap();
        let mut package = crate::archive::Package::open(&archive).unwrap();
        let mut manifest = crate::verify::verify_embedded(&mut package).unwrap();
        manifest.runtime_profile = "component-v1".into();
        let mut data = String::from("(data (i32.const 0) \"hello.txt\")");
        let mut code = String::from("i32.const 0 i32.const 9 i32.const 0 i32.const 5 i32.const 50000 call $read\ni32.const 50000 i32.load i32.const 1 i32.ne if unreachable end\ni32.const 50008 i32.load i32.const 5 i32.ne if unreachable end\n");
        let mut position = 256;
        for path in cases::rejected_paths()
            .into_iter()
            .chain(cases::literal_paths().iter().map(|p| p.to_string()))
        {
            let escaped = path
                .bytes()
                .map(|byte| format!("\\{byte:02x}"))
                .collect::<String>();
            data.push_str(&format!("(data (i32.const {position}) \"{escaped}\")\n"));
            code.push_str("i32.const 60000 i64.const 123456789 i64.store\n");
            code.push_str(&format!("i32.const {position} i32.const {} i32.const 0 i32.const 8 i32.const 50000 call $read\ni32.const 50000 i32.load if unreachable end\n", path.len()));
            code.push_str(&format!("i32.const {position} i32.const {} f32.const 0 f32.const 0 f32.const 1 f32.const 1 call $image if unreachable end\n", path.len()));
            code.push_str(&format!("i32.const {position} i32.const {} f32.const 0 f32.const 0 f32.const 1 f32.const 1 call $linear if unreachable end\n", path.len()));
            code.push_str(&format!(
                "i32.const 0 i32.const {position} i32.const {} call $texture if unreachable end\n",
                path.len()
            ));
            code.push_str(
                "i32.const 60000 i64.load i64.const 123456789 i64.ne if unreachable end\n",
            );
            position += path.len().max(1);
        }
        for (offset, count) in [(u32::MAX, 8), (0, 65537)] {
            code.push_str(&format!("i32.const 0 i32.const 9 i32.const {offset} i32.const {count} i32.const 50000 call $read\ni32.const 50000 i32.load if unreachable end\n"));
        }
        assert!(position < 50000);
        let wasm = wat::parse_str(format!(r#"(module
            (import "gamepkg:engine/host@0.1.0" "asset-read" (func $read (param i32 i32 i32 i32 i32)))
            (import "gamepkg:engine/host@0.1.0" "draw-shader-asset-image" (func $texture (param i32 i32 i32) (result i32)))
            (import "gamepkg:engine/host@0.1.0" "draw-asset-image" (func $image (param i32 i32 f32 f32 f32 f32) (result i32)))
            (import "gamepkg:engine/host@0.1.0" "draw-asset-image-linear" (func $linear (param i32 i32 f32 f32 f32 f32) (result i32)))
            (memory (export "memory") 1) {data}
            (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 55000)
            (func (export "tick") (param i32 i64) (result i64) {code} i64.const 42))"#)).unwrap();
        let component = crate::hostapi::componentize(wasm).unwrap();
        let mut game = Game::with_package(
            &component,
            &manifest,
            &Default::default(),
            Some(package),
            None,
            None,
        )
        .unwrap();
        game.store.as_mut().unwrap().data_mut().shader_count = 1;
        assert_eq!(game.step(0).unwrap(), 42);
        assert!(game.frame().commands.is_empty());
    }
}
#[cfg(test)]
mod doom_input_tests {
    use super::*;
    use crate::doom_input::Event;
    fn fixture(input: bool) -> Game {
        let mut manifest =
            Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
        manifest.permissions.input = input;
        let bytes = wat::parse_str(
            "(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) local.get 1))",
        )
        .unwrap();
        Game::from_bytes(&bytes, &manifest).unwrap()
    }
    #[test]
    fn desktop_queue_permissions_coalescing_budget_and_release_are_enforced() {
        let mut ordinary = fixture(true);
        assert!(ordinary
            .doom_input(Event::Key {
                code: b'a',
                down: true
            })
            .is_err());
        let mut denied = fixture(false);
        denied.doom_engine = true;
        assert!(denied
            .doom_input(Event::Key {
                code: b'a',
                down: true
            })
            .is_err());
        let mut game = fixture(true);
        game.doom_engine = true;
        for _ in 0..10000 {
            game.doom_input(Event::Mouse {
                buttons: 1,
                dx: 1,
                dy: -1,
            })
            .unwrap();
        }
        assert_eq!(
            game.doom_events,
            vec![Event::Mouse {
                buttons: 1,
                dx: 10000,
                dy: -10000
            }]
        );
        for _ in 1..crate::doom_input::MAX_QUEUED {
            game.doom_input(Event::Key {
                code: b'b',
                down: true,
            })
            .unwrap();
        }
        assert!(game
            .doom_input(Event::Key {
                code: b'c',
                down: true
            })
            .is_err());
        game.doom_release_input().unwrap();
        assert_eq!(game.doom_events, vec![Event::Release]);
    }
}

#[cfg(test)]
mod persistence_output_tests {
    use super::*;
    #[test]
    fn persistence_failures_discard_audio_and_drawing_from_the_rejected_tick() {
        for component in [false, true] {
            let mut manifest =
                Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
            manifest.permissions.audio = true;
            manifest.homepage = Some("https://example.org/project".into());
            manifest.permissions.save = true;
            manifest.achievements.push(crate::manifest::Achievement {
                id: "step".into(),
                title: "Step".into(),
                description: "Step".into(),
            });
            let (namespace, audio, draw, unlock, tick) = if component {
                manifest.runtime_profile = "component-v1".into();
                (
                    "gamepkg:engine/host@0.1.0",
                    "audio-submit-stereo",
                    "draw-rect",
                    "achievement-unlock",
                    "tick",
                )
            } else {
                (
                    "gamepkg",
                    "audio_submit_stereo",
                    "draw_rect",
                    "achievement_unlock",
                    "gamepkg_tick",
                )
            };
            let save = if component {
                "save-write"
            } else {
                "save_write"
            };
            let window = if component {
                "window-control"
            } else {
                "window_control"
            };
            let module = wat::parse_str(format!(r#"(module
                (import "{namespace}" "{audio}" (func $audio(param i32 i32)(result i32)))
                (import "{namespace}" "{draw}" (func $draw(param f32 f32 f32 f32 i32)(result i32)))
                (import "{namespace}" "{unlock}" (func $unlock(param i32)(result i32)))
                (import "{namespace}" "{save}" (func $save(param i32 i32)(result i32)))
                (import "{namespace}" "{window}" (func $window(param i32 i32)(result i32)))
                (memory(export "memory")1)
                (data(i32.const 16) "\01\00\ff\ff")
                (func(export "{tick}")(param i32 i64)(result i64)
                    (drop(call $window(i32.const 4)(i32.const 0)))
                    (drop(call $audio(i32.const 16)(i32.const 2)))
                    (drop(call $draw(f32.const 0)(f32.const 0)(f32.const 10)(f32.const 10)(i32.const -1)))
                    (drop(call $unlock(i32.const 0)))
                    (drop(call $save(i32.const 16)(i32.const 4)))
                    i64.const 1))"#)).unwrap();
            let bytes = if component {
                crate::hostapi::componentize(module).unwrap()
            } else {
                module
            };
            let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
            // Prove the fixture really emits both outputs on a successful tick.
            assert_eq!(game.step(4).unwrap(), 1);
            assert_eq!(game.audio_frame(), [1, -1]);
            assert!(!game.frame().commands.is_empty());
            assert_eq!(
                game.take_homepage_url().as_deref(),
                Some("https://example.org/project")
            );
            let temporary = tempfile::tempdir().unwrap();
            game.progress = Some(
                crate::progress::Session::open(
                    temporary.path(),
                    &manifest.id,
                    &manifest.creator_key,
                )
                .unwrap(),
            );
            std::fs::write(
                temporary.path().join("progress"),
                b"blocked history directory",
            )
            .unwrap();
            assert!(game.step(4).is_err());
            assert!(
                game.take_homepage_url().is_none(),
                "rejected tick retained homepage request"
            );
            assert!(
                game.audio_frame().is_empty(),
                "component={component}: rejected tick retained PCM"
            );
            assert_eq!(game.audio_channels(), 1);
            assert!(
                game.frame().commands.is_empty(),
                "component={component}: rejected tick retained drawing"
            );
            let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
            let temporary = tempfile::tempdir().unwrap();
            let save = crate::saves::Save::open_for_game(temporary.path(), &manifest).unwrap();
            game.store.as_mut().unwrap().data_mut().save = Some(save);
            std::fs::rename(
                temporary.path().join("saves"),
                temporary.path().join("saved-directory"),
            )
            .unwrap();
            std::fs::write(temporary.path().join("saves"), b"blocked save directory").unwrap();
            assert!(game.step(4).is_err());
            assert!(
                game.take_homepage_url().is_none(),
                "rejected tick retained homepage request"
            );
            assert!(game.audio_frame().is_empty());
            assert_eq!(game.audio_channels(), 1);
            assert!(
                game.frame().commands.is_empty(),
                "component={component}: save failure retained drawing"
            );
        }
    }
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires the isolated signed native fixture from tools/check_tick_output_rollback.py"]
    fn native_persistence_failures_discard_audio_and_drawing() {
        let root =
            std::path::PathBuf::from(std::env::var_os("GAMEPKG_TICK_OUTPUT_LIBRARY").unwrap());
        let id = "com.gamepkg.tick-output-test";
        let mut package =
            crate::archive::Package::open(&root.join(format!("{id}.gamepkg"))).unwrap();
        let manifest = crate::verify::verify_embedded(&mut package).unwrap();
        for history in [true, false] {
            let mut game = Game::load(&root, id).unwrap();
            assert!(game.native.is_some());
            assert_eq!(game.step(4).unwrap(), 1);
            assert_eq!(game.audio_frame(), [1, -1]);
            assert!(!game.frame().commands.is_empty());
            assert_eq!(
                game.take_homepage_url().as_deref(),
                Some("https://example.org/project")
            );
            let temporary = tempfile::tempdir().unwrap();
            if history {
                game.progress = Some(
                    crate::progress::Session::open(
                        temporary.path(),
                        &manifest.id,
                        &manifest.creator_key,
                    )
                    .unwrap(),
                );
                std::fs::write(
                    temporary.path().join("progress"),
                    b"blocked history directory",
                )
                .unwrap();
            } else {
                game.progress = None;
                std::fs::rename(root.join("saves"), root.join("saved-directory")).unwrap();
                std::fs::write(root.join("saves"), b"blocked save directory").unwrap();
            }
            assert!(game.step(4).is_err());
            assert!(
                game.take_homepage_url().is_none(),
                "rejected tick retained homepage request"
            );
            assert!(
                game.audio_frame().is_empty(),
                "history={history}: native rejected tick retained PCM"
            );
            assert_eq!(game.audio_channels(), 1);
            assert!(
                game.frame().commands.is_empty(),
                "history={history}: native rejected tick retained drawing"
            );
            drop(game);
            if !history {
                std::fs::remove_file(root.join("saves")).unwrap();
                std::fs::rename(root.join("saved-directory"), root.join("saves")).unwrap();
            }
        }
    }
}

#[cfg(test)]
mod effect_runtime_tests {
    use super::*;
    use crate::audio_voices::EffectCommand;
    use std::sync::Mutex;

    fn fixture(component: bool, trap: bool) -> (tempfile::TempDir, Game) {
        let root = tempfile::tempdir().unwrap();
        let recipe = root.path().join("recipe");
        for path in [
            "manifest.toml",
            "LICENSE",
            "NOTICE",
            "CREDITS.md",
            "thumbnail.png",
            "source/Cargo.toml",
            "source/Cargo.lock",
            "source/src/lib.rs",
            "shaders/main.slang",
            "assets/hello.txt",
        ] {
            let target = recipe.join(path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/minimal")
                    .join(path),
                target,
            )
            .unwrap();
        }
        let path = recipe.join("manifest.toml");
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("audio = false", "audio = true")
            .replace("save = false", "save = true")
            .replace("[limits]", "[limits]\neffects_memory_bytes = 1048576");
        let text = if component {
            format!("runtime_profile=\"component-v1\"\n{text}")
        } else {
            text
        };
        let text = format!("{text}\n[[achievements]]\nid=\"first\"\ntitle=\"First\"\ndescription=\"Fixture\"\n[[achievements]]\nid=\"second\"\ntitle=\"Second\"\ndescription=\"Fixture\"\n");
        std::fs::write(path, text).unwrap();
        let mut wav = b"RIFF\x28\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\x04\0\0\0".to_vec();
        wav.extend_from_slice(&[0xff, 0x7f, 0, 0x80]);
        std::fs::write(recipe.join("assets/effect.wav"), wav).unwrap();
        let archive = root.path().join("test.gamepkg");
        crate::pack::pack(&recipe, &archive, &[0x65; 32]).unwrap();
        let mut package = crate::archive::Package::open(&archive).unwrap();
        let manifest = crate::verify::verify_embedded(&mut package).unwrap();
        let (namespace, effect, audio, save, tick, denied) = if component {
            (
                "gamepkg:engine/host@0.1.0",
                "effect-call",
                "audio-submit",
                "save-write",
                "tick",
                0,
            )
        } else {
            (
                "gamepkg",
                "effect_call",
                "audio_submit",
                "save_write",
                "gamepkg_tick",
                -1,
            )
        };
        let unlock = if component {
            "achievement-unlock"
        } else {
            "achievement_unlock"
        };
        let trap = if trap {
            "local.get 0 if unreachable end"
        } else {
            ""
        };
        let module = wat::parse_str(format!(r#"(module
            (import "{namespace}" "{effect}" (func $effect(param i32 i32 i32 i32)(result i32)))
            (import "{namespace}" "{audio}" (func $audio(param i32 i32)(result i32)))
            (import "{namespace}" "{save}" (func $save(param i32 i32)(result i32)))
            (import "{namespace}" "{unlock}" (func $unlock(param i32)(result i32)))
            (memory(export "memory") 1)
            (data(i32.const 0) "effect.wav")
            (func(export "cabi_realloc")(param i32 i32 i32 i32)(result i32) i32.const 60000)
            (func(export "{tick}")(param i32 i64)(result i64)
                i32.const 1 i32.const 0 i32.const 0 i32.const 64 call $effect i32.eqz if unreachable end
                i32.const 2 i32.const 0 i32.const 0 i32.const 0 call $effect i32.eqz if unreachable end
                i32.const 0 i32.const 0 i32.const 10 i32.const 0 call $effect i32.eqz if unreachable end
                i32.const 3 i32.const 0 i32.const 0 i32.const 0 call $effect i32.eqz if unreachable end
                i32.const 0 i32.const 0 i32.const 10 i32.const 0 call $effect i32.eqz if unreachable end
                i32.const 0 i32.const 0 call $audio i32.const {denied} i32.ne if unreachable end
                i32.const 0 i32.const 10 call $save drop
                local.get 0 call $unlock drop
                {trap} i64.const 42))"#)).unwrap();
        let bytes = if component {
            crate::hostapi::componentize(module).unwrap()
        } else {
            module
        };
        let save = crate::saves::Save::open_for_game(root.path(), &manifest).unwrap();
        let mut game = Game::with_package(
            &bytes,
            &manifest,
            &Default::default(),
            Some(package),
            None,
            Some(save),
        )
        .unwrap();
        game.progress = Some(
            crate::progress::Session::open(root.path(), &manifest.id, &manifest.creator_key)
                .unwrap(),
        );
        (root, game)
    }
    fn recorder(game: &mut Game) -> Arc<Mutex<Vec<Vec<EffectCommand>>>> {
        let batches = Arc::new(Mutex::new(Vec::new()));
        let target = batches.clone();
        game.attach_effect_output(Arc::new(move |commands| {
            target.lock().unwrap().push(commands.to_vec());
            true
        }));
        batches
    }
    #[test]
    fn signed_core_and_component_stage_cache_and_commit_only_after_success() {
        for component in [false, true] {
            let (_root, mut game) = fixture(component, false);
            let batches = recorder(&mut game);
            assert_eq!(game.step(0).unwrap(), 42);
            let batches = batches.lock().unwrap();
            assert_eq!(batches.len(), 1);
            assert_eq!(batches[0].len(), 5);
            assert!(matches!(batches[0][0], EffectCommand::Volume(64)));
            assert!(matches!(batches[0][1], EffectCommand::Pause(false)));
            assert!(matches!(batches[0][3], EffectCommand::Stop));
            let (EffectCommand::Play(first), EffectCommand::Play(second)) =
                (&batches[0][2], &batches[0][4])
            else {
                panic!("missing clips")
            };
            assert!(Arc::ptr_eq(first, second));
            let mut mixer = crate::audio_voices::Mixer::default();
            mixer.volume(64);
            mixer.play(first.clone());
            assert_eq!(mixer.frame([0; 2]), [16383, -16384]);
            assert!(game.audio_frame().is_empty());
        }
    }
    #[test]
    fn traps_save_failures_and_history_failures_cannot_publish_effects() {
        for component in [false, true] {
            for failure in ["trap", "save", "history"] {
                let (root, mut game) = fixture(component, failure == "trap");
                let batches = recorder(&mut game);
                assert_eq!(game.step(0).unwrap(), 42);
                batches.lock().unwrap().clear();
                if failure == "save" {
                    std::fs::rename(root.path().join("saves"), root.path().join("old-saves"))
                        .unwrap();
                    std::fs::write(root.path().join("saves"), b"blocked").unwrap();
                } else if failure == "history" {
                    std::fs::rename(
                        root.path().join("progress"),
                        root.path().join("old-progress"),
                    )
                    .unwrap();
                    std::fs::write(root.path().join("progress"), b"blocked").unwrap();
                }
                assert!(
                    game.step(1).is_err(),
                    "component={component} failure={failure}"
                );
                assert!(batches.lock().unwrap().is_empty());
                if failure == "trap" {
                    if component {
                        // Wasmtime prevents re-entry after a component trap;
                        // another attempted tick must not replay audio either.
                        assert!(game.step(0).is_err());
                        assert!(batches.lock().unwrap().is_empty());
                    } else {
                        assert_eq!(game.step(0).unwrap(), 42);
                        assert_eq!(batches.lock().unwrap().len(), 1);
                    }
                }
            }
        }
    }
    #[test]
    fn output_rejection_clears_the_transaction_and_a_fresh_tick_can_recover() {
        for component in [false, true] {
            let (_root, mut game) = fixture(component, false);
            game.attach_effect_output(Arc::new(|_| false));
            assert!(game.step(0).is_err());
            let batches = recorder(&mut game);
            assert_eq!(game.step(0).unwrap(), 42);
            assert_eq!(batches.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn recorded_effects_reduce_guest_memory_inside_the_existing_hard_ceiling() {
        for component in [false, true] {
            let (_root, game) = fixture(component, false);
            let package = game
                .store
                .as_ref()
                .unwrap()
                .data()
                .package
                .as_ref()
                .unwrap();
            for pages in [240, 241] {
                let mut package = package.try_clone_verified().unwrap();
                let mut manifest = crate::verify::verify_embedded(&mut package).unwrap();
                let tick = if component { "tick" } else { "gamepkg_tick" };
                let core = wat::parse_str(format!("(module(memory(export \"memory\"){pages})(func(export \"{tick}\")(param i32 i64)(result i64)i64.const 0))")).unwrap();
                let bytes = if component {
                    crate::hostapi::componentize(core).unwrap()
                } else {
                    core
                };
                let make = |manifest: &Manifest, settings: &crate::settings::Settings| {
                    Game::with_package(
                        &bytes,
                        manifest,
                        settings,
                        Some(package.try_clone_verified().unwrap()),
                        None,
                        None,
                    )
                };
                assert_eq!(make(&manifest, &Default::default()).is_ok(), pages == 240);
                let settings = crate::settings::Settings {
                    memory_ceiling_mib: 1,
                    ..Default::default()
                };
                assert!(make(&manifest, &settings).is_err());
                manifest.limits.effects_memory_bytes = None;
                assert!(make(&manifest, &Default::default()).is_ok());
            }
        }
    }
}
