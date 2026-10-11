//! Versioned typed WIT imports. No WASI or ambient host interfaces are linked.
use super::Data;
use anyhow::Result;
use wasmtime::{
    component::{Linker, WasmList, WasmStr},
    AsContext, Engine, StoreContextMut,
};
fn charge(store: &mut StoreContextMut<'_, Data>, cost: u64) -> Result<bool> {
    let fuel = store.get_fuel()?;
    store.set_fuel(fuel.saturating_sub(cost))?;
    Ok(fuel >= cost)
}
fn chunk(bytes: Option<&[u8]>, offset: u32, length: u32) -> Option<Vec<u8>> {
    if length > 65536 {
        return None;
    }
    let bytes = bytes?.get(offset as usize..)?;
    Some(bytes[..bytes.len().min(length as usize)].to_vec())
}
pub(super) fn linker(engine: &Engine) -> Result<Linker<Data>> {
    let mut linker = Linker::new(engine);
    let mut host = linker.instance("gamepkg:engine/host@0.1.0")?;
    host.func_wrap(
        "asset-read",
        |mut store: StoreContextMut<'_, Data>, (path, offset, length): (String, u32, u32)| {
            if path.len() > 1024 || !crate::archive::valid_path(&path) || length > 65536 {
                return Ok((None::<Vec<u8>>,));
            }
            let cost = store.data_mut().package.as_mut().and_then(|p| {
                p.asset_chunk_cost(&path, u64::from(offset), length as usize)
                    .ok()
            });
            let Some(cost) = cost else {
                return Ok((None,));
            };
            if !charge(&mut store, cost)? {
                return Ok((None,));
            }
            let bytes = store
                .data_mut()
                .package
                .as_mut()
                .unwrap()
                .asset_chunk(&path, u64::from(offset), length as usize)
                .ok();
            Ok((bytes,))
        },
    )?;
    host.func_wrap(
        "achievement-unlock",
        |mut store: StoreContextMut<'_, Data>, (index,): (u32,)| {
            let Some(id) = store.data().achievements.get(index as usize).cloned() else {
                return Ok((false,));
            };
            if !charge(&mut store, 64)? {
                return Ok((false,));
            }
            if !store.data().pending_achievements.contains(&id) {
                store.data_mut().pending_achievements.push(id);
            }
            Ok((true,))
        },
    )?;
    host.func_wrap("rom-size", |store: StoreContextMut<'_, Data>, (): ()| {
        Ok((store.data().rom.as_ref().map(|v| v.len() as u32),))
    })?;
    host.func_wrap("save-size", |store: StoreContextMut<'_, Data>, (): ()| {
        Ok((store.data().save.as_ref().map(|v| v.bytes.len() as u32),))
    })?;
    host.func_wrap(
        "rom-read",
        |mut store: StoreContextMut<'_, Data>, (offset, length): (u32, u32)| {
            if length > 65536 || !charge(&mut store, u64::from(length) + 128)? {
                return Ok((None::<Vec<u8>>,));
            }
            Ok((chunk(store.data().rom.as_deref(), offset, length),))
        },
    )?;
    host.func_wrap(
        "save-read",
        |mut store: StoreContextMut<'_, Data>, (offset, length): (u32, u32)| {
            if length > 65536 || !charge(&mut store, u64::from(length) + 128)? {
                return Ok((None::<Vec<u8>>,));
            }
            Ok((chunk(
                store.data().save.as_ref().map(|v| v.bytes.as_slice()),
                offset,
                length,
            ),))
        },
    )?;
    host.func_wrap(
        "save-write",
        |mut store: StoreContextMut<'_, Data>, (bytes,): (Vec<u8>,)| {
            if bytes.len() > crate::saves::MAX_SAVE
                || store.data().save.is_none()
                || !charge(&mut store, bytes.len() as u64 + 1024)?
            {
                return Ok((false,));
            }
            let save = store.data_mut().save.as_mut().unwrap();
            save.bytes = bytes;
            save.dirty = true;
            Ok((true,))
        },
    )?;
    host.func_wrap(
        "draw-clear",
        |mut store: StoreContextMut<'_, Data>, (rgba,): (u32,)| {
            if !charge(&mut store, 64)? {
                return Ok((false,));
            }
            store.data_mut().frame.reset();
            store.data_mut().frame.background = Some(rgba);
            Ok((true,))
        },
    )?;
    host.func_wrap(
        "draw-rect",
        |mut store: StoreContextMut<'_, Data>, (x, y, w, h, rgba): (f32, f32, f32, f32, u32)| {
            if !charge(&mut store, 64)? {
                return Ok((false,));
            }
            Ok((store.data_mut().frame.rect(x, y, w, h, rgba),))
        },
    )?;
    host.func_wrap(
        "draw-circle",
        |mut store: StoreContextMut<'_, Data>, (x, y, r, rgba): (f32, f32, f32, u32)| {
            if !charge(&mut store, 64)? {
                return Ok((false,));
            }
            Ok((store.data_mut().frame.circle(x, y, r, rgba),))
        },
    )?;
    host.func_wrap(
        "draw-asset-image",
        |mut store: StoreContextMut<'_, Data>,
         (path, x, y, w, h): (WasmStr, f32, f32, f32, f32)| {
            let path = path.to_str(store.as_context())?;
            if path.len() > 1024 {
                return Ok((false,));
            }
            let path = path.into_owned();
            let mut fuel = store.get_fuel()?;
            let result = super::draw_asset_image(store.data_mut(), &mut fuel, &path, x, y, w, h);
            store.set_fuel(fuel)?;
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "draw-image",
        |mut store: StoreContextMut<'_, Data>,
         (x, y, w, h, pw, ph, pixels): (f32, f32, f32, f32, u32, u32, WasmList<u8>)| {
            let Some(bytes) = crate::drawing::image::Image::byte_length(pw, ph) else {
                return Ok((false,));
            };
            if pixels.len() != bytes
                || !store.data().frame.can_image(x, y, w, h, bytes)
                || !charge(&mut store, bytes as u64 + 64)?
            {
                return Ok((false,));
            }
            let Some(image) =
                crate::drawing::image::Image::new(pw, ph, pixels.as_le_slice(store.as_context()))
            else {
                return Ok((false,));
            };
            Ok((store.data_mut().frame.image(x, y, w, h, image),))
        },
    )?;
    host.func_wrap(
        "draw-shader-image",
        |mut store: StoreContextMut<'_, Data>,
         (index, pw, ph, pixels): (u32, u32, u32, WasmList<u8>)| {
            let (x, y, w, h) = (0.0, 0.0, 640.0, 480.0);
            let count = store.data().shader_count;
            if index > i32::MAX as u32 || index as usize >= count {
                return Ok((false,));
            }
            let Some(bytes) = crate::drawing::image::Image::byte_length(pw, ph) else {
                return Ok((false,));
            };
            if pixels.len() != bytes
                || !store.data().frame.can_image(x, y, w, h, bytes)
                || !charge(&mut store, bytes as u64 + 64)?
            {
                return Ok((false,));
            }
            let Some(image) =
                crate::drawing::image::Image::new(pw, ph, pixels.as_le_slice(store.as_context()))
            else {
                return Ok((false,));
            };
            Ok((store
                .data_mut()
                .frame
                .shader_image(index as i32, count, image),))
        },
    )?;
    host.func_wrap(
        "draw-shader",
        |mut store: StoreContextMut<'_, Data>, (index,): (u32,)| {
            if index > i32::MAX as u32 || !charge(&mut store, 64)? {
                return Ok((false,));
            }
            let count = store.data().shader_count;
            Ok((store.data_mut().frame.shader(index as i32, count),))
        },
    )?;
    host.func_wrap(
        "audio-submit",
        |mut store: StoreContextMut<'_, Data>, (samples,): (Vec<i16>,)| {
            if !store.data().audio_allowed
                || samples.len() + store.data().audio.len() > 1600
                || !charge(&mut store, samples.len() as u64 * 2 + 64)?
            {
                return Ok((false,));
            }
            store.data_mut().audio.extend(samples);
            Ok((true,))
        },
    )?;
    Ok(linker)
}
