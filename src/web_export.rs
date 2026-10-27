//! Portable browser player exports from verified installed builds.
use anyhow::{bail, Context, Result};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

const SUPPORTED: &[&str] = &[
    "frame_time_us",
    "window_state",
    "window_size",
    "random_u64",
    "input_events",
    "file_call",
    "window_control",
    "asset_read",
    "rom_count",
    "rom_size",
    "rom_size_at",
    "rom_read",
    "rom_read_at",
    "save_size",
    "save_read",
    "save_write",
    "save_chunk",
    "draw_clear",
    "draw_clip",
    "draw_clip_reset",
    "draw_rect",
    "draw_circle",
    "achievement_unlock",
    "draw_image",
    "draw_image_linear",
    "draw_asset_image",
    "draw_asset_image_linear",
    "draw_asset_image_linear_straight",
    "effect_call",
    "audio_submit",
    "audio_submit_stereo",
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

// Header/chunk policy for the worker's bounded lossless PNG decoder.
// Call only after the host PNG decoder has validated the source image.
fn exact_png_source(bytes: &[u8]) -> bool {
    if bytes.len() < 33
        || !matches!(bytes[25], 0 | 2 | 3 | 4 | 6)
        || (if matches!(bytes[25], 0 | 3) {
            !matches!(bytes[24], 1 | 2 | 4 | 8)
        } else {
            bytes[24] != 8
        })
        || bytes[28] != 0
    {
        return false;
    }
    let mut at = 8usize;
    let mut chunks = 0usize;
    while at + 12 <= bytes.len() {
        chunks += 1;
        if chunks > 2048 {
            return false;
        }
        let size = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let Some(end) = at
            .checked_add(12)
            .and_then(|n| n.checked_add(size))
            .filter(|n| *n <= bytes.len())
        else {
            return false;
        };
        let kind = &bytes[at + 4..at + 8];
        if kind == b"acTL" || (kind == b"tRNS" && !matches!(bytes[25], 0 | 2 | 3)) {
            return false;
        }
        if kind == b"IEND" {
            return end == bytes.len();
        }
        at = end;
    }
    false
}

const ASSET_INFLATE_SCRATCH: u64 = 263168;
fn compressed_effect_wav(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    if !(131072..=1049088).contains(&bytes.len()) {
        return Ok(None);
    }
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    if &bytes[..4] != b"RIFF"
        || &bytes[8..16] != b"WAVEfmt "
        || word(4) as usize != bytes.len() - 8
        || word(16) != 16
        || bytes[20..24] != [1, 0, 2, 0]
        || word(24) != 44100
        || word(28) != 176400
        || bytes[32..36] != [4, 0, 16, 0]
        || &bytes[36..40] != b"data"
        || word(40) as usize != bytes.len() - 44
        || !(bytes.len() - 44).is_multiple_of(4)
    {
        return Ok(None);
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(bytes)?;
    let packed = encoder.finish()?;
    Ok((packed.len() + 64 < bytes.len()).then_some(packed))
}

pub fn export(root: &Path, id: &str, output: &Path) -> Result<()> {
    let _guard = crate::registry::LibraryChange::acquire(root)?;
    let mut package = crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    let effects = manifest.limits.effects_memory_bytes.unwrap_or(0);
    let music = manifest.limits.music_memory_bytes.unwrap_or(0);
    let recorded = effects != 0 || music != 0;
    let converter = include_bytes!(concat!(env!("OUT_DIR"), "/audio-converter.wasm")).as_slice();
    let decoder = if music != 0 {
        crate::audio_decode::bundled_bytes()?
    } else {
        &[]
    };
    if recorded {
        if converter.is_empty() {
            bail!("this app was built without the browser audio converter companion");
        }
        let factory = crate::audio_decode::Factory::new(converter)?;
        if factory.maximum_memory_bytes() > 262144 {
            bail!("browser audio converter exceeds its memory limit");
        }
    }
    let guest_memory = manifest
        .limits
        .hard_memory_bytes
        .checked_sub(effects)
        .and_then(|n| n.checked_sub(music))
        .filter(|n| *n >= 65536)
        .context("audio shares leave insufficient guest memory")?;
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
    let mut needs_precise_images = false;
    let mut uses_shaders = false;
    let mut custom_renderer = false;
    for payload in wasmparser::Parser::new(0).parse_all(&wasm) {
        if let wasmparser::Payload::ImportSection(imports) = payload? {
            for import in imports {
                let import = import?;
                needs_precise_images |= matches!(
                    import.name,
                    "draw_asset_image_linear_straight" | "draw-asset-image-linear-straight"
                );
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
    let wasm = cap_memory(&wasm, guest_memory)?;
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
    let mut precise_images = BTreeMap::new();
    let mut exact_png_images = Vec::new();
    let mut size = 0usize;
    let mut asset_scratch_bytes = 0u64;
    for name in names {
        let bytes = package.asset(&name)?;
        size += bytes.len();
        if needs_precise_images && name.to_lowercase().ends_with(".png") {
            if let Some(image) = crate::drawing::image::Image::from_png(&bytes) {
                if exact_png_source(&bytes) {
                    exact_png_images.push(name.clone());
                } else if image
                    .pixels()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] != 255)
                {
                    size += image.pixels().len();
                    if size > 32 * 1024 * 1024 {
                        bail!("web assets exceed 32 MiB export limit");
                    }
                    precise_images.insert(name.clone(),serde_json::json!({"width":image.size()[0],"height":image.size()[1],"rgba":hex::encode(image.pixels())}));
                }
            }
        }
        let mut packed = None;
        if effects != 0 && name.ends_with(".wav") {
            if let Some(candidate) = compressed_effect_wav(&bytes)? {
                let scratch = asset_scratch_bytes.max(bytes.len() as u64 + ASSET_INFLATE_SCRATCH);
                if let Some(remaining) = guest_memory
                    .checked_sub(2 * scratch)
                    .filter(|n| *n >= 65536)
                {
                    if cap_memory(&wasm, remaining).is_ok() {
                        asset_scratch_bytes = scratch;
                        packed = Some(candidate);
                    }
                }
            }
        }
        let payload = if let Some(packed) = packed {
            size = size - bytes.len() + packed.len();
            serde_json::json!({"length":bytes.len(),"zlib":hex::encode(packed)})
        } else {
            serde_json::Value::String(hex::encode(bytes))
        };
        if size > 32 * 1024 * 1024 {
            bail!("web assets exceed 32 MiB export limit");
        }
        assets.insert(name, payload);
    }
    let wasm = cap_memory(&wasm, guest_memory - 2 * asset_scratch_bytes)?;
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
        &serde_json::json!({"format_version":1,"recorded_audio":if recorded {Some(serde_json::json!({"converter_sha256":crate::verify::digest(converter),"decoder_sha256":if music != 0 {Some(crate::verify::digest(decoder))}else{None}}))}else{None},"manifest":manifest,"wasm_sha256":crate::verify::digest(&wasm),"assets":assets,"asset_scratch_bytes":asset_scratch_bytes,"precise_images":precise_images,"exact_png_images":exact_png_images,"shaders":shaders,"custom_renderer":custom_renderer}),
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
            "asset-store.js",
            include_bytes!("../web-player/asset-store.js").as_slice(),
        ),
        (
            "png-inflate.js",
            include_bytes!("../web-player/png-inflate.js").as_slice(),
        ),
        (
            "png-exact.js",
            include_bytes!("../web-player/png-exact.js").as_slice(),
        ),
        (
            "worker.js",
            include_bytes!("../web-player/worker.js").as_slice(),
        ),
        (
            "recorded-budget.js",
            include_bytes!("../web-player/recorded-budget.js").as_slice(),
        ),
        (
            "progress-store.js",
            include_bytes!("../web-player/progress-store.js").as_slice(),
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
    if recorded {
        for (name, bytes) in [
            (
                "recorded-session.js",
                include_bytes!("../web-player/recorded-session.js").as_slice(),
            ),
            (
                "recorded-player-start.js",
                include_bytes!("../web-player/recorded-player-start.js").as_slice(),
            ),
            (
                "recorded-output-client.js",
                include_bytes!("../web-player/recorded-output-client.js").as_slice(),
            ),
            (
                "recorded-worklet.js",
                include_bytes!("../web-player/recorded-worklet.js").as_slice(),
            ),
            (
                "recorded-frame-dispatcher.js",
                include_bytes!("../web-player/recorded-frame-dispatcher.js").as_slice(),
            ),
            (
                "recorded-music-loader.js",
                include_bytes!("../web-player/recorded-music-loader.js").as_slice(),
            ),
            (
                "recorded-transport.js",
                include_bytes!("../web-player/recorded-transport.js").as_slice(),
            ),
            (
                "recorded-stream.js",
                include_bytes!("../web-player/recorded-stream.js").as_slice(),
            ),
            (
                "recorded-mixer.js",
                include_bytes!("../web-player/recorded-mixer.js").as_slice(),
            ),
            (
                "recorded-queue.js",
                include_bytes!("../web-player/recorded-queue.js").as_slice(),
            ),
            (
                "recorded-wav.js",
                include_bytes!("../web-player/recorded-wav.js").as_slice(),
            ),
            (
                "recorded-decoder.js",
                include_bytes!("../web-player/recorded-decoder.js").as_slice(),
            ),
            (
                "recorded-decoder-client.js",
                include_bytes!("../web-player/recorded-decoder-client.js").as_slice(),
            ),
            (
                "recorded-decoder-worker.js",
                include_bytes!("../web-player/recorded-decoder-worker.js").as_slice(),
            ),
            ("audio-converter.wasm", converter),
        ] {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join(name))?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        if music != 0 {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join("audio-decoder.wasm"))?;
            file.write_all(decoder)?;
            file.sync_all()?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compressed_effect_wav_preserves_every_original_byte_and_rejects_wrong_formats() {
        let pcm = vec![0x39u8; 131072];
        let mut wav=b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\0\0\0\0".to_vec();
        wav[4..8].copy_from_slice(&((pcm.len() + 36) as u32).to_le_bytes());
        wav[40..44].copy_from_slice(&(pcm.len() as u32).to_le_bytes());
        wav.extend_from_slice(&pcm);
        let packed = compressed_effect_wav(&wav).unwrap().unwrap();
        let mut restored = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(packed.as_slice()),
            &mut restored,
        )
        .unwrap();
        assert_eq!(restored, wav);
        assert!(packed.len() < wav.len());
        for offset in [0, 4, 8, 16, 20, 22, 24, 28, 32, 34, 36, 40] {
            let mut bad = wav.clone();
            bad[offset] ^= 1;
            assert!(compressed_effect_wav(&bad).unwrap().is_none());
        }
        assert!(compressed_effect_wav(&wav[..131071]).unwrap().is_none());
        assert!(compressed_effect_wav(&vec![0; 1049089]).unwrap().is_none());
    }

    #[test]
    fn exact_png_selection_retains_indexed_transparency_and_rejects_unsupported_headers() {
        for source in [
            include_bytes!("../ports/arkanoo/assets/heart.png").as_slice(),
            include_bytes!("../ports/arkanoo/assets/ball.png").as_slice(),
        ] {
            assert!(crate::drawing::image::Image::from_png(source).is_some());
            assert!(exact_png_source(source));
            assert!(!exact_png_source(&source[..source.len() - 1]));
            let mut unsupported = source.to_vec();
            unsupported[24] = 16;
            assert!(!exact_png_source(&unsupported));
            unsupported[24] = 8;
            unsupported[28] = 1;
            assert!(!exact_png_source(&unsupported));
        }
    }

    #[test]
    fn exact_png_selection_retains_grayscale_and_gray_alpha_without_raw_duplicates() {
        use image::ImageEncoder;
        for (color, pixels) in [
            (image::ExtendedColorType::L8, vec![0, 85, 170, 255]),
            (
                image::ExtendedColorType::La8,
                vec![0, 255, 85, 0, 170, 127, 255, 255],
            ),
        ] {
            let mut source = Vec::new();
            image::codecs::png::PngEncoder::new(&mut source)
                .write_image(&pixels, 2, 2, color)
                .unwrap();
            let decoded = crate::drawing::image::Image::from_png(&source).unwrap();
            assert_eq!(decoded.size(), [2, 2]);
            assert!(exact_png_source(&source));
            let expected: Vec<_> = if color == image::ExtendedColorType::L8 {
                pixels
                    .iter()
                    .flat_map(|&gray| [gray, gray, gray, 255])
                    .collect()
            } else {
                pixels
                    .chunks_exact(2)
                    .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                    .collect()
            };
            assert_eq!(decoded.pixels(), expected);
        }
    }

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
