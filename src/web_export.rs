//! Portable browser player exports from verified installed builds.
use anyhow::{bail, Context, Result};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

const SUPPORTED: &[&str] = &[
    "asset_read",
    "rom_count",
    "rom_size",
    "rom_size_at",
    "rom_read",
    "rom_read_at",
    "save_size",
    "save_read",
    "save_write",
    "draw_clear",
    "draw_rect",
    "draw_circle",
    "achievement_unlock",
    "draw_image",
    "draw_asset_image",
    "audio_submit",
    "draw_shader",
    "draw_shader_image",
    "draw_shader_asset_image",
];

fn leb(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    let mut value = 0;
    for shift in (0..35).step_by(7) {
        let byte = *bytes.get(*cursor).context("truncated Wasm section")?;
        *cursor += 1;
        if shift == 28 && byte > 15 {
            bail!("invalid Wasm length");
        }
        value |= u32::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    bail!("invalid Wasm integer")
}
fn encode(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        output.push(byte | if value == 0 { 0 } else { 128 });
        if value == 0 {
            break;
        }
    }
}
/// Browser engines enforce the rewritten memory maximum, including memory.grow.
fn cap_memory(bytes: &[u8], limit: u64) -> Result<Vec<u8>> {
    let maximum = u32::try_from(limit / 65536)?;
    let mut output = bytes[..8].to_vec();
    let mut cursor = 8;
    while cursor < bytes.len() {
        let kind = bytes[cursor];
        cursor += 1;
        let size = leb(bytes, &mut cursor)? as usize;
        let end = cursor.checked_add(size).context("Wasm section overflow")?;
        let section = bytes.get(cursor..end).context("truncated Wasm section")?;
        let mut rewritten = Vec::new();
        if kind == 5 {
            let mut position = 0;
            let count = leb(section, &mut position)?;
            if count > 1 {
                bail!("web export permits one guest memory");
            }
            encode(count, &mut rewritten);
            for _ in 0..count {
                let flags = leb(section, &mut position)?;
                if flags > 1 {
                    bail!("shared and 64-bit memory are unsupported in web exports");
                }
                let initial = leb(section, &mut position)?;
                let declared = if flags == 1 {
                    leb(section, &mut position)?
                } else {
                    maximum
                };
                if initial > maximum {
                    bail!("guest initial memory exceeds manifest hard limit");
                }
                encode(1, &mut rewritten);
                encode(initial, &mut rewritten);
                encode(declared.min(maximum), &mut rewritten);
            }
            if position != section.len() {
                bail!("invalid guest memory section");
            }
        } else {
            rewritten.extend_from_slice(section);
        }
        output.push(kind);
        encode(rewritten.len().try_into()?, &mut output);
        output.extend(rewritten);
        cursor = end;
    }
    wasmparser::Validator::new().validate_all(&output)?;
    Ok(output)
}

/// Select the host-generated core guest inside a verified WIT component.
/// Browser bindings implement its fixed canonical ABI; arbitrary component
/// graphs and additional imports are never forwarded to the browser.
fn browser_module(bytes: &[u8], profile: &str) -> Result<Vec<u8>> {
    if profile == "core-v1" {
        if !bytes.starts_with(b"\0asm\x01\0\0\0") {
            bail!("core runtime requires a core Wasm build");
        }
        return Ok(bytes.to_vec());
    }
    if profile != "component-v1" || !bytes.starts_with(b"\0asm\x0d\0\x01\0") {
        bail!("unsupported browser runtime profile or mismatched build");
    }
    wasmparser::Validator::new().validate_all(bytes)?;
    let mut selected = None;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::ModuleSection {
            unchecked_range, ..
        } = payload?
        {
            let module = bytes
                .get(unchecked_range)
                .context("invalid component module range")?;
            let mut tick = false;
            let mut allocator = false;
            for item in wasmparser::Parser::new(0).parse_all(module) {
                if let wasmparser::Payload::ExportSection(exports) = item? {
                    for export in exports {
                        let export = export?;
                        tick |=
                            export.name == "tick" && export.kind == wasmparser::ExternalKind::Func;
                        allocator |= export.name == "cabi_realloc"
                            && export.kind == wasmparser::ExternalKind::Func;
                    }
                }
            }
            if tick && allocator {
                if selected.is_some() {
                    bail!("ambiguous component guest modules");
                }
                selected = Some(module.to_vec());
            }
        }
    }
    selected.context("component lacks a browser-compatible GamePKG guest module")
}

pub fn export(root: &Path, id: &str, output: &Path) -> Result<()> {
    let _guard = crate::registry::LibraryChange::acquire(root)?;
    let mut package = crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    if manifest.id != id {
        bail!("installed package ID differs");
    }
    let source_package = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        256 * 1024 * 1024,
        false,
    )?;
    let license = package.read("LICENSE")?;
    let credits = package.read("CREDITS.md")?;
    let notice = if package.entries.contains_key("NOTICE") {
        package.read("NOTICE")?
    } else {
        Vec::new()
    };
    let path = crate::build::cache_path(root, id)?;
    let wasm = crate::files::read_regular(&path, 32 * 1024 * 1024, false)?;
    crate::integrity::verify_build(&path, &wasm)?;
    if !crate::build::artifact_has_tick(&wasm) {
        bail!("build the game before exporting for web");
    }
    if manifest.builtin_engine.is_some() {
        bail!("builtin-engine browser export is not supported; use the native player");
    }
    let wasm = browser_module(&wasm, &manifest.runtime_profile)?;
    let namespace = if manifest.runtime_profile == "component-v1" {
        "gamepkg:engine/host@0.1.0"
    } else {
        "gamepkg"
    };
    let mut uses_shaders = false;
    let mut custom_renderer = false;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        if let wasmparser::Payload::ImportSection(imports) = payload? {
            for import in imports {
                let import = import?;
                custom_renderer |=
                    import.name.starts_with("draw_") || import.name.starts_with("draw-");
                uses_shaders |= matches!(
                    import.name,
                    "draw_shader"
                        | "draw_shader_image"
                        | "draw_shader_asset_image"
                        | "draw-shader"
                        | "draw-shader-image"
                        | "draw-shader-asset-image"
                );
                if import.module != namespace
                    || !SUPPORTED.iter().any(|name| {
                        if manifest.runtime_profile == "component-v1" {
                            name.replace('_', "-") == import.name
                        } else {
                            *name == import.name
                        }
                    })
                    || !matches!(import.ty, wasmparser::TypeRef::Func(_))
                {
                    bail!(
                        "web player does not support import {}/{}",
                        import.module,
                        import.name
                    );
                }
            }
        }
    }
    let wasm = cap_memory(&wasm, manifest.limits.hard_memory_bytes)?;
    if package.has_shared_assets() {
        package.resolve_shared_assets(root)?;
    }
    let mut names: Vec<String> = package
        .entries
        .keys()
        .filter_map(|name| name.strip_prefix("assets/").map(str::to_owned))
        .collect();
    names.extend(manifest.shared_assets.keys().cloned());
    names.sort();
    names.dedup();
    let mut assets = BTreeMap::new();
    let mut size = 0usize;
    for name in names {
        let bytes = package.asset(&name)?;
        size += bytes.len();
        if size > 32 * 1024 * 1024 {
            bail!("web assets exceed 32 MiB export limit");
        }
        assets.insert(name, hex::encode(bytes));
    }
    let shaders = if uses_shaders {
        crate::shaders::load_fragment_sources(
            root,
            &source_package,
            &manifest,
            &crate::settings::load()?,
        )?
    } else {
        Vec::new()
    };
    if shaders.iter().map(String::len).sum::<usize>() > 32 * 1024 * 1024 {
        bail!("browser shader source exceeds 32 MiB export budget");
    }
    let data = serde_json::to_vec(
        &serde_json::json!({"format_version":1,"manifest":manifest,"wasm_sha256":crate::verify::digest(&wasm),"assets":assets,"shaders":shaders,"custom_renderer":custom_renderer}),
    )?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    fs::create_dir(output).context("web output must be a new directory")?;
    for (name, bytes) in [
        ("source.gamepkg", source_package.as_slice()),
        ("GAME-LICENSE.txt", license.as_slice()),
        ("GAME-CREDITS.md", credits.as_slice()),
        ("GAME-NOTICE.txt", notice.as_slice()),
        (
            "PLAYER-LICENSE.txt",
            include_bytes!("../LICENSE").as_slice(),
        ),
        ("PLAYER-NOTICE.txt", include_bytes!("../NOTICE").as_slice()),
        ("game.wasm", wasm.as_slice()),
        ("game.json", data.as_slice()),
        (
            "player.js",
            include_bytes!("../web-player/player.js").as_slice(),
        ),
        (
            "worker.js",
            include_bytes!("../web-player/worker.js").as_slice(),
        ),
        (
            "style.css",
            include_bytes!("../web-player/style.css").as_slice(),
        ),
        (
            "index.html",
            include_bytes!("../web-player/index.html").as_slice(),
        ),
    ] {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(name))?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_the_verified_component_guest_without_forwarding_component_graphs() {
        let core = wat::parse_str(
            r#"(module (memory (export "memory") 1)
            (func (export "tick") (param i32 i64) (result i64) local.get 1)
            (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 1024))"#,
        )
        .unwrap();
        let component = crate::hostapi::componentize(core.clone()).unwrap();
        let selected = browser_module(&component, "component-v1").unwrap();
        assert!(selected.starts_with(b"\0asm\x01\0\0\0"));
        assert!(browser_module(&core, "component-v1").is_err());
        assert!(browser_module(&component, "core-v1").is_err());
        let duplicate = wat::parse_str(
            r#"(component
            (core module (func (export "tick")) (func (export "cabi_realloc")))
            (core module (func (export "tick")) (func (export "cabi_realloc"))))"#,
        )
        .unwrap();
        assert!(browser_module(&duplicate, "component-v1")
            .unwrap_err()
            .to_string()
            .contains("ambiguous"));
    }

    #[test]
    fn memory_grow_is_bounded_by_manifest() {
        let bytes = wat::parse_str(
            "(module (memory 1) (func (export \"grow\") (result i32) i32.const 1 memory.grow))",
        )
        .unwrap();
        let bounded = cap_memory(&bytes, 65536).unwrap();
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, &bounded).unwrap();
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
        assert_eq!(
            instance
                .get_typed_func::<(), i32>(&mut store, "grow")
                .unwrap()
                .call(&mut store, ())
                .unwrap(),
            -1
        );
        assert!(cap_memory(&bytes, 32768).is_err());
    }
}
