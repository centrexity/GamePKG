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
const RENDERER_REVISION: i32 = 6;
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
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_ppu(source: &str) -> Result<String> {
    let patch: serde_json::Value =
        serde_json::from_str(include_str!("../engines/zelda3/adapters/ppu-patches.json"))?;
    if patch["file"] != "snes/ppu.c" {
        bail!("invalid PPU adaptation path");
    }
    let mut adapted = source.to_owned();
    for (item, count) in [
        (&patch, 3),
        (&patch["unsigned_masks"], 1),
        (&patch["mosaic_spans"][0], 2),
        (&patch["mosaic_spans"][1], 1),
    ] {
        let before = item["before"]
            .as_str()
            .context("missing PPU adaptation anchor")?;
        let after = item["after"]
            .as_str()
            .context("missing PPU adaptation replacement")?;
        if item["count"] != count || adapted.matches(before).count() != count as usize {
            bail!("pinned PPU adaptation anchor changed");
        }
        adapted = adapted.replace(before, after);
    }
    Ok(adapted)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_dma_state(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/zelda3/adapters/dma-state-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid DMA state adaptations")?;
    anyhow::ensure!(patches.len() == 2, "invalid DMA state adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid DMA state anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid DMA state replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned Zelda DMA state anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_dsp_state(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/zelda3/adapters/dsp-state-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid DSP state adaptations")?;
    anyhow::ensure!(patches.len() == 4, "invalid DSP state adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid DSP state anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid DSP state replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned Zelda DSP state anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_ppu_state(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/zelda3/adapters/ppu-state-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid PPU state adaptations")?;
    anyhow::ensure!(patches.len() == 2, "invalid PPU state adaptation count");
    // The pinned original has CRLF; the full-function anchor uses logical lines.
    let mut staged = source.replace("\r\n", "\n");
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid PPU state anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid PPU state replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned Zelda PPU state anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn stage_snapshot_bridge(source: &str) -> Result<String> {
    for anchor in [
        "static void InternalSaveLoad(",
        "static void SaveSnesState(",
        "static void LoadSnesState(",
    ] {
        anyhow::ensure!(
            source.matches(anchor).count() == 1,
            "pinned Zelda snapshot helper changed"
        );
    }
    anyhow::ensure!(
        !source.contains("gamepkg_snapshot_bridge.h"),
        "Zelda snapshot bridge already staged"
    );
    Ok(format!(
        "{source}\n#include \"gamepkg_snapshot_bridge.h\"\n"
    ))
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_spc_state(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/zelda3/adapters/spc-state-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid SPC state adaptations")?;
    anyhow::ensure!(patches.len() == 5, "invalid SPC state adaptation count");
    let mut staged = source.to_owned();
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid SPC state anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid SPC state replacement")?;
        anyhow::ensure!(
            patch["count"] == 1 && staged.matches(before).count() == 1,
            "pinned Zelda SPC state anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn adapt_spc_playback(source: &str) -> Result<String> {
    let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../engines/zelda3/adapters/spc-playback-patches.json"
    ))?;
    let patches = patches
        .as_array()
        .context("invalid SPC playback adaptations")?;
    anyhow::ensure!(patches.len() == 23, "invalid SPC playback adaptation count");
    let mut staged = source.replace("\r\n", "\n");
    for patch in patches {
        let before = patch["before"]
            .as_str()
            .context("invalid SPC playback anchor")?;
        let after = patch["after"]
            .as_str()
            .context("invalid SPC playback replacement")?;
        let count = patch["count"]
            .as_u64()
            .context("invalid SPC playback count")?;
        anyhow::ensure!(
            matches!(count, 1 | 2 | 4) && staged.matches(before).count() == count as usize,
            "pinned Zelda SPC playback anchor changed"
        );
        staged = staged.replace(before, after);
    }
    Ok(staged)
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
        std::fs::write(rtl_path, stage_snapshot_bridge(&rtl)?)?;
        let ppu = upstream.join("snes/ppu.c");
        std::fs::write(
            &ppu,
            adapt_ppu_state(&adapt_ppu(&std::fs::read_to_string(&ppu)?)?)?,
        )?;
        let dma = upstream.join("snes/dma.c");
        std::fs::write(&dma, adapt_dma_state(&std::fs::read_to_string(&dma)?)?)?;
        let dsp = upstream.join("snes/dsp.c");
        std::fs::write(&dsp, adapt_dsp_state(&std::fs::read_to_string(&dsp)?)?)?;
        let spc = upstream.join("src/spc_player.c");
        std::fs::write(
            &spc,
            adapt_spc_playback(&adapt_spc_state(&std::fs::read_to_string(&spc)?)?)?,
        )?;
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
        stamp_engine(bytes)
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
const SOURCE_SECTION: &str = "gamepkg.zelda3.sources.v1";
fn source_identity() -> &'static str {
    static IDENTITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(|| {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        for (name, bytes) in crate::zelda3_sources::FILES {
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
    validate_renderer(&bytes)?;
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
/// Source compatibility metadata; build receipts and sandbox checks still apply.
pub fn validate_current(bytes: &[u8]) -> Result<()> {
    validate_renderer(bytes)?;
    let mut found = false;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CustomSection(section) = payload? {
            if section.name() == SOURCE_SECTION {
                anyhow::ensure!(
                    !found && section.data() == source_identity().as_bytes(),
                    "rebuild the pinned Zelda engine: source kit changed"
                );
                found = true;
            }
        }
    }
    anyhow::ensure!(
        found,
        "rebuild the pinned Zelda engine: source revision missing"
    );
    Ok(())
}
fn validate_renderer(bytes: &[u8]) -> Result<()> {
    validate(bytes)?;
    let mut signatures = Vec::new();
    let mut functions = Vec::new();
    let mut revision_function = None;
    let mut code_index = 0;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        match payload? {
            wasmparser::Payload::TypeSection(types) => {
                for ty in types.into_iter_err_on_gc_types() {
                    let ty = ty?;
                    signatures.push((
                        ty.params().is_empty() && ty.results() == [wasmparser::ValType::I32],
                        ty.params().is_empty() && ty.results().is_empty(),
                    ));
                }
            }
            wasmparser::Payload::FunctionSection(types) => {
                for ty in types {
                    functions.push(ty?);
                }
            }
            wasmparser::Payload::ExportSection(exports) => {
                for export in exports {
                    let export = export?;
                    if export.name == "renderer_revision"
                        && export.kind == wasmparser::ExternalKind::Func
                    {
                        revision_function = Some(export.index);
                    }
                }
            }
            wasmparser::Payload::CodeSectionEntry(body) => {
                if revision_function == Some(code_index) {
                    let signature = functions
                        .get(code_index as usize)
                        .and_then(|ty| signatures.get(*ty as usize))
                        .copied()
                        .unwrap_or((false, false));
                    if !signature.0 || body.get_locals_reader()?.get_count() != 0 {
                        break;
                    }
                    let mut ops = body.get_operators_reader()?;
                    let mut constant = false;
                    let mut returned = false;
                    let mut constructor_prefix = 0;
                    while !ops.eof() {
                        match ops.read()? {
                            wasmparser::Operator::Nop => {}
                            wasmparser::Operator::Block {
                                blockty: wasmparser::BlockType::Empty,
                            } if !constant && constructor_prefix == 0 => constructor_prefix = 1,
                            wasmparser::Operator::Call { function_index }
                                if !constant
                                    && constructor_prefix <= 1
                                    && functions
                                        .get(function_index as usize)
                                        .and_then(|ty| signatures.get(*ty as usize))
                                        .is_some_and(|signature| signature.1) =>
                            {
                                constructor_prefix = if constructor_prefix == 1 { 2 } else { 3 };
                            }
                            wasmparser::Operator::End if !constant && constructor_prefix == 2 => {
                                constructor_prefix = 3
                            }
                            wasmparser::Operator::I32Const { value }
                                if !constant
                                    && matches!(constructor_prefix, 0 | 3)
                                    && value == RENDERER_REVISION =>
                            {
                                constant = true
                            }
                            wasmparser::Operator::Return if constant && !returned => {
                                returned = true
                            }
                            wasmparser::Operator::End if constant && ops.eof() => return Ok(()),
                            _ => break,
                        }
                    }
                    break;
                }
                code_index += 1;
            }
            _ => {}
        }
    }
    bail!("rebuild the Zelda engine to apply the current renderer fixes")
}

fn require_renderer_revision(revision: i32) -> Result<()> {
    if revision != RENDERER_REVISION {
        bail!("rebuild the Zelda engine to apply the current renderer fixes");
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
    width: u32,
    height: u32,
    sram: TypedFunc<(), i32>,
    dirty: TypedFunc<(), i32>,
    status: TypedFunc<(), i32>,
    position: TypedFunc<(), i32>,
    controller: TypedFunc<(), i32>,
    snapshot_size: TypedFunc<(), i32>,
    snapshot_data: TypedFunc<(), i32>,
    snapshot_capture: TypedFunc<(), i32>,
    snapshot_restore: TypedFunc<i32, i32>,
}
pub struct Frame {
    pub width: u32,
    pub height: u32,
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
        video: Option<crate::manifest::ZeldaVideo>,
    ) -> Result<Self> {
        validate_current(bytes)?;
        let options = video.unwrap_or_default();
        options.validate()?;
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
        let revision = instance
            .get_typed_func::<(), i32>(&mut store, "renderer_revision")
            .context("rebuild the Zelda engine to apply the current renderer fixes")?;
        require_renderer_revision(revision.call(&mut store, ())?)?;
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
        if let Some(function) = instance.get_func(&mut store, "configure_video") {
            if function.typed::<(i32, i32), i32>(&store)?.call(
                &mut store,
                (
                    options.extra_width as i32,
                    options.configuration_bits() as i32,
                ),
            )? != 0
            {
                bail!("Zelda video configuration failed; rebuild the engine to apply display settings");
            }
        } else {
            crate::video_selections::require_engine_support(Some(options), false)?;
        }
        let (width, height) = options.dimensions();
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
        let snapshot_size = instance.get_typed_func(&mut store, "snapshot_size")?;
        let snapshot_data = instance.get_typed_func(&mut store, "snapshot_data")?;
        let snapshot_capture = instance.get_typed_func(&mut store, "snapshot_capture")?;
        let snapshot_restore = instance.get_typed_func(&mut store, "snapshot_restore")?;
        Ok(Self {
            store,
            memory,
            step,
            reset,
            pixels,
            audio,
            audio_channels,
            width,
            height,
            sram,
            dirty,
            status,
            position,
            controller,
            snapshot_size,
            snapshot_data,
            snapshot_capture,
            snapshot_restore,
        })
    }
    fn snapshot_pointer(&mut self, deadline: u64) -> Result<usize> {
        self.store.set_epoch_deadline(deadline);
        anyhow::ensure!(
            self.snapshot_size.call(&mut self.store, ())? == crate::zelda_snapshots::SIZE as i32,
            "invalid Zelda snapshot extent"
        );
        let pointer = self.snapshot_data.call(&mut self.store, ())?;
        anyhow::ensure!(pointer > 0, "missing Zelda snapshot buffer");
        Ok(pointer as usize)
    }
    pub(crate) fn capture_snapshot(&mut self, output: &mut [u8], deadline: u64) -> Result<()> {
        anyhow::ensure!(
            output.len() == crate::zelda_snapshots::SIZE,
            "invalid Zelda snapshot size"
        );
        let pointer = self.snapshot_pointer(deadline)?;
        anyhow::ensure!(
            self.snapshot_capture.call(&mut self.store, ())? == 0,
            "Zelda snapshot capture failed"
        );
        self.memory.read(&self.store, pointer, output)?;
        Ok(())
    }
    pub(crate) fn restore_snapshot(&mut self, bytes: &[u8], deadline: u64) -> Result<()> {
        anyhow::ensure!(
            bytes.len() == crate::zelda_snapshots::SIZE,
            "invalid Zelda snapshot size"
        );
        let pointer = self.snapshot_pointer(deadline)?;
        self.memory.write(&mut self.store, pointer, bytes)?;
        anyhow::ensure!(
            self.snapshot_restore
                .call(&mut self.store, bytes.len() as i32)?
                == 0,
            "Zelda snapshot was rejected"
        );
        Ok(())
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
        let mut pixels = vec![0u8; self.width as usize * self.height as usize * 4];
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
            width: self.width,
            height: self.height,
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
    fn spc_live_staging_removes_native_byte_aliases_and_rejects_changed_anchors() {
        let normalized =
            include_str!("../engines/zelda3/upstream/src/spc_player.c").replace("\r\n", "\n");
        let original = normalized.as_str();
        let staged = super::adapt_spc_playback(original).unwrap();
        assert!(
            !staged.contains("WORD(")
                && !staged.contains("HIBYTE(")
                && !staged.contains("*(uint16 *)")
        );
        for (call, count) in [
            ("gamepkg_spc_read_le16(", 2),
            ("gamepkg_spc_read_ram(", 8),
            ("gamepkg_spc_high(", 6),
            ("gamepkg_spc_with_high(", 6),
        ] {
            assert_eq!(staged.matches(call).count(), count);
        }
        assert!(super::adapt_spc_playback(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_str(include_str!(
            "../engines/zelda3/adapters/spc-playback-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            assert!(super::adapt_spc_playback(
                &original.replace(patch["before"].as_str().unwrap(), "/* changed */")
            )
            .is_err());
        }
        assert_eq!(
            staged.replace("\r\n", "\n"),
            super::adapt_spc_playback(&original.replace('\n', "\r\n")).unwrap()
        );
        let combined =
            super::adapt_spc_playback(&super::adapt_spc_state(original).unwrap()).unwrap();
        assert!(
            combined.contains("gamepkg_spc_to_ram(") && combined.contains("gamepkg_spc_read_ram(")
        );
        for file in [
            "engines/zelda3/adapters/spc-playback-patches.json",
            "engines/zelda3/adapters/include/gamepkg_spc_playback.h",
        ] {
            assert!(crate::zelda3_sources::FILES
                .iter()
                .any(|(name, _)| *name == file));
        }
    }
    #[test]
    fn spc_word_staging_rejects_changed_or_repeated_copy_anchors() {
        let original = include_str!("../engines/zelda3/upstream/src/spc_player.c");
        let staged = super::adapt_spc_state(original).unwrap();
        assert!(staged.contains("#include \"gamepkg_spc_state.h\""));
        assert_eq!(staged.matches("gamepkg_spc_to_ram(").count(), 2);
        assert_eq!(staged.matches("gamepkg_spc_from_ram(").count(), 2);
        assert!(super::adapt_spc_state(&staged).is_err());
        assert_eq!(
            staged.replace("\r\n", "\n"),
            super::adapt_spc_state(&original.replace("\r\n", "\n")).unwrap()
        );
        let patches: serde_json::Value = serde_json::from_str(include_str!(
            "../engines/zelda3/adapters/spc-state-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            assert!(super::adapt_spc_state(
                &original.replace(patch["before"].as_str().unwrap(), "/* changed */")
            )
            .is_err());
        }
        for file in [
            "engines/zelda3/adapters/spc-state-patches.json",
            "engines/zelda3/adapters/include/gamepkg_spc_state.h",
        ] {
            assert!(crate::zelda3_sources::FILES
                .iter()
                .any(|(name, _)| *name == file));
        }
    }
    #[test]
    fn whole_snapshot_bridge_rejects_changed_or_repeated_helpers() {
        let original = include_str!("../engines/zelda3/upstream/src/zelda_rtl.c");
        let staged = super::stage_snapshot_bridge(original).unwrap();
        assert!(staged.ends_with("#include \"gamepkg_snapshot_bridge.h\"\n"));
        assert!(super::stage_snapshot_bridge(&staged).is_err());
        for helper in [
            "static void InternalSaveLoad(",
            "static void SaveSnesState(",
            "static void LoadSnesState(",
        ] {
            assert!(
                super::stage_snapshot_bridge(&original.replace(helper, "/* changed */")).is_err()
            );
        }
    }
    #[test]
    fn ppu_state_adaptation_is_checked_and_preserves_renderer_adaptations() {
        let original = include_str!("../engines/zelda3/upstream/snes/ppu.c").replace("\r\n", "\n");
        let staged = super::adapt_ppu_state(&original).unwrap();
        assert!(staged.contains("gamepkg_ppu_saveload(ppu, func, ctx)"));
        assert_eq!(
            super::adapt_ppu_state(&original.replace('\n', "\r\n")).unwrap(),
            staged
        );
        assert!(super::adapt_ppu_state(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/zelda3/adapters/ppu-state-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(super::adapt_ppu_state(&original.replace(before, "/* changed */")).is_err());
        }
        let combined = super::adapt_ppu_state(&super::adapt_ppu(&original).unwrap()).unwrap();
        assert!(combined.contains("gamepkg_ppu_saveload(ppu, func, ctx)"));
        assert_eq!(
            combined
                .matches("((uint32)addr[0] | ((uint32)addr[8] << 16))")
                .count(),
            3
        );
        assert!(combined.contains("0x01010101u << i"));
    }
    #[test]
    fn dsp_state_adaptation_rejects_changed_or_repeated_sources() {
        let original = include_str!("../engines/zelda3/upstream/snes/dsp.c");
        let staged = super::adapt_dsp_state(original).unwrap();
        assert!(staged.contains("gamepkg_dsp_saveload(dsp, func, ctx)"));
        assert!(super::adapt_dsp_state(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/zelda3/adapters/dsp-state-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(super::adapt_dsp_state(&original.replace(before, "/* changed */")).is_err());
        }
    }
    #[test]
    fn dma_state_adaptation_rejects_changed_or_repeated_sources() {
        let original = include_str!("../engines/zelda3/upstream/snes/dma.c");
        let staged = super::adapt_dma_state(original).unwrap();
        assert!(staged.contains("gamepkg_dma_saveload(dma, func, ctx)"));
        assert!(super::adapt_dma_state(&staged).is_err());
        let patches: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../engines/zelda3/adapters/dma-state-patches.json"
        ))
        .unwrap();
        for patch in patches.as_array().unwrap() {
            let before = patch["before"].as_str().unwrap();
            assert!(super::adapt_dma_state(&original.replace(before, "/* changed */")).is_err());
        }
    }
    #[test]
    fn current_renderer_without_source_kit_is_not_current() {
        let old = wat::parse_str(r#"(module (memory 1 256) (func (export "renderer_revision") (result i32) i32.const 6))"#).unwrap();
        super::validate(&old).unwrap();
        assert!(super::validate_current(&old).is_err());
    }
    #[test]
    fn current_renderer_metadata_rejects_stale_caches_without_changing_memory_policy() {
        let old = wat::parse_str("(module (memory 1 256))").unwrap();
        super::validate(&old).unwrap();
        assert!(super::validate_current(&old)
            .unwrap_err()
            .to_string()
            .contains("rebuild"));
        let current = wat::parse_str(r#"(module (memory 1 256) (func (export "renderer_revision") (result i32) i32.const 6))"#).unwrap();
        let current = super::stamp_engine(current).unwrap();
        super::validate_current(&current).unwrap();
        assert!(super::stamp_engine(current.clone()).is_err());
        let mut changed = current;
        *changed.last_mut().unwrap() ^= 1;
        assert!(super::validate_current(&changed).is_err());
        for source in [
            r#"(module (memory 1 256) (func (export "renderer_revision") (result i32) i32.const 0))"#,
            r#"(module (memory 1 256) (func (export "renderer_revision") (param i32) (result i32) i32.const 6))"#,
            r#"(module (memory 1 256) (func (export "renderer_revision") (result i32) i32.const 6 i32.const 0 i32.add))"#,
        ] {
            assert!(super::stamp_engine(wat::parse_str(source).unwrap()).is_err());
        }
        let padded = wat::parse_str(r#"(module (memory 1 256) (func (export "renderer_revision") (result i32) nop i32.const 6 nop return))"#).unwrap();
        super::validate_current(&super::stamp_engine(padded).unwrap()).unwrap();
        let clang_wrapper = wat::parse_str(r#"(module (memory 1 256) (func $ctors nop) (func (export "renderer_revision") (result i32) block call $ctors end i32.const 6))"#).unwrap();
        super::validate_current(&super::stamp_engine(clang_wrapper).unwrap()).unwrap();
        let wrong_constructor_type = wat::parse_str(r#"(module (memory 1 256) (func $ctor (param i32)) (func (export "renderer_revision") (result i32) block i32.const 0 call $ctor end i32.const 6))"#).unwrap();
        assert!(super::stamp_engine(wrong_constructor_type).is_err());

        let wrong_kind = wat::parse_str(
            r#"(module (memory 1 256) (global (export "renderer_revision") i32 (i32.const 6)))"#,
        )
        .unwrap();
        assert!(super::stamp_engine(wrong_kind).is_err());
        super::require_renderer_revision(6).unwrap();
        for revision in [0, 1, 2, 3, 4, 5, -1] {
            assert!(super::require_renderer_revision(revision).is_err());
        }
        for invalid in [
            r#"(module (memory 1) (func (export "renderer_revision") (result i32) i32.const 6))"#,
            r#"(module (memory 1 257) (func (export "renderer_revision") (result i32) i32.const 6))"#,
            r#"(module (import "ambient" "read" (func)) (memory 1 256) (func (export "renderer_revision") (result i32) i32.const 6))"#,
        ] {
            assert!(super::stamp_engine(wat::parse_str(invalid).unwrap()).is_err());
        }
    }
    #[test]
    fn unsigned_plane_adaptation_is_exact_and_rejects_changed_or_repeated_sources() {
        let original = include_str!("../engines/zelda3/upstream/snes/ppu.c");
        let adapted = super::adapt_ppu(original).unwrap();
        assert_eq!(
            adapted
                .matches("((uint32)addr[0] | ((uint32)addr[8] << 16))")
                .count(),
            3
        );
        assert_eq!(adapted.matches("0x01010101u << i").count(), 1);
        assert!(super::adapt_ppu(&original.replacen("0x01010101 << i", "changed", 1)).is_err());
        assert_eq!(
            adapted
                .matches("(sx % ppu->mosaicSize + ppu->mosaicSize)")
                .count(),
            2
        );
        assert_eq!(
            adapted
                .matches("(x % ppu->mosaicSize + ppu->mosaicSize)")
                .count(),
            1
        );
        for anchor in [
            "ppu->mosaicSize - (sx - ppu->mosaicModulo[sx])",
            "ppu->mosaicSize - (x - ppu->mosaicModulo[x])",
        ] {
            assert!(super::adapt_ppu(&original.replacen(anchor, "changed", 1)).is_err());
        }
        assert!(super::adapt_ppu(&adapted).is_err());
        assert!(
            super::adapt_ppu(&original.replacen("addr[0] | addr[8] << 16", "changed", 1)).is_err()
        );
    }
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
