//! Versioned typed WIT imports. No WASI or ambient host interfaces are linked.
use super::Data;
use anyhow::Result;
use wasmtime::{
    component::{Linker, WasmList, WasmStr},
    AsContext, Engine, StoreContextMut,
};
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
        "input-events",
        |mut store: StoreContextMut<'_, Data>, (capacity,): (u32,)| {
            Ok((store.data_mut().input_events.read(capacity as usize),))
        },
    )?;
    host.func_wrap(
        "file-call",
        |mut store: StoreContextMut<'_, Data>,
         (operation, request, capacity): (u32, WasmList<u8>, u32)| {
            if operation > 5
                || request.len() > crate::user_files::MAX_REQUEST
                || capacity > crate::user_files::MAX_CHUNK as u32
            {
                return Ok(((-1i32, Vec::<u8>::new()),));
            }
            let request = request.as_le_slice(store.as_context()).to_vec();
            Ok((
                match store
                    .data_mut()
                    .user_files
                    .call(operation, &request, capacity as usize)
                {
                    Some(bytes) => (bytes.len() as i32, bytes),
                    None => (-1, Vec::new()),
                },
            ))
        },
    )?;
    host.func_wrap(
        "window-control",
        |mut store: StoreContextMut<'_, Data>, (operation, value): (u32, u32)| {
            Ok((store.data_mut().window.request(operation, value),))
        },
    )?;
    host.func_wrap("random-u64", |_: StoreContextMut<'_, Data>, (): ()| {
        Ok((crate::random::value()?,))
    })?;
    host.func_wrap(
        "network-call",
        |mut store: StoreContextMut<'_, Data>,
         (operation, request, capacity): (u32, WasmList<u8>, u32)| {
            if operation > 9 || request.len() > 65540 || capacity > 65536 {
                return Ok(((-1i32, Vec::<u8>::new()),));
            }
            let request = request.as_le_slice(store.as_context()).to_vec();
            let reply = store
                .data_mut()
                .network
                .call(operation as u8, &request, capacity as usize);
            Ok((match reply.first() {
                Some(0) => ((reply.len() - 1) as i32, reply[1..].to_vec()),
                Some(2) => (-2, Vec::new()),
                Some(3) => (-3, Vec::new()),
                _ => (-1, Vec::new()),
            },))
        },
    )?;
    host.func_wrap(
        "asset-read",
        |mut store: StoreContextMut<'_, Data>, (path, offset, length): (String, u32, u32)| {
            if path.len() > 1024 || !crate::archive::valid_path(&path) || length > 65536 {
                return Ok((None::<Vec<u8>>,));
            }
            let bytes = store.data_mut().package.as_mut().and_then(|package| {
                package
                    .asset_chunk(&path, u64::from(offset), length as usize)
                    .ok()
            });
            Ok((bytes,))
        },
    )?;
    host.func_wrap(
        "achievement-unlock",
        |mut store: StoreContextMut<'_, Data>, (index,): (u32,)| {
            let Some(id) = store.data().achievements.get(index as usize).cloned() else {
                return Ok((false,));
            };
            if !store.data().pending_achievements.contains(&id) {
                store.data_mut().pending_achievements.push(id);
            }
            Ok((true,))
        },
    )?;
    host.func_wrap("rom-count", |store: StoreContextMut<'_, Data>, (): ()| {
        Ok((store.data().rom.as_ref().map_or(0, |v| v.len() as u32),))
    })?;
    host.func_wrap(
        "rom-size-at",
        |store: StoreContextMut<'_, Data>, (index,): (u32,)| {
            Ok((store
                .data()
                .rom
                .as_ref()
                .and_then(|v| v.get(index as usize))
                .map(|v| v.len() as u32),))
        },
    )?;
    host.func_wrap(
        "rom-read-at",
        |store: StoreContextMut<'_, Data>, (index, offset, length): (u32, u32, u32)| {
            if length > 65536 {
                return Ok((None::<Vec<u8>>,));
            }
            Ok((chunk(
                store
                    .data()
                    .rom
                    .as_ref()
                    .and_then(|v| v.get(index as usize))
                    .map(Vec::as_slice),
                offset,
                length,
            ),))
        },
    )?;
    host.func_wrap("rom-size", |store: StoreContextMut<'_, Data>, (): ()| {
        Ok((store
            .data()
            .rom
            .as_ref()
            .and_then(|v| v.first())
            .map(|v| v.len() as u32),))
    })?;
    host.func_wrap("save-size", |store: StoreContextMut<'_, Data>, (): ()| {
        Ok((store.data().save.as_ref().map(|v| v.bytes.len() as u32),))
    })?;
    host.func_wrap(
        "rom-read",
        |store: StoreContextMut<'_, Data>, (offset, length): (u32, u32)| {
            if length > 65536 {
                return Ok((None::<Vec<u8>>,));
            }
            Ok((chunk(
                store
                    .data()
                    .rom
                    .as_ref()
                    .and_then(|v| v.first())
                    .map(Vec::as_slice),
                offset,
                length,
            ),))
        },
    )?;
    host.func_wrap(
        "save-read",
        |store: StoreContextMut<'_, Data>, (offset, length): (u32, u32)| {
            if length > 65536 {
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
            if bytes.len() > crate::saves::MAX_SAVE || store.data().save.is_none() {
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
            store.data_mut().frame.clear(rgba);
            Ok((true,))
        },
    )?;
    host.func_wrap(
        "draw-clip",
        |mut store: StoreContextMut<'_, Data>, (x, y, w, h): (f32, f32, f32, f32)| {
            Ok((store.data_mut().frame.clip(Some([x, y, w, h])),))
        },
    )?;
    host.func_wrap(
        "draw-clip-reset",
        |mut store: StoreContextMut<'_, Data>, (): ()| Ok((store.data_mut().frame.clip(None),)),
    )?;
    host.func_wrap(
        "draw-rect",
        |mut store: StoreContextMut<'_, Data>, (x, y, w, h, rgba): (f32, f32, f32, f32, u32)| {
            Ok((store.data_mut().frame.rect(x, y, w, h, rgba),))
        },
    )?;
    host.func_wrap(
        "draw-circle",
        |mut store: StoreContextMut<'_, Data>, (x, y, r, rgba): (f32, f32, f32, u32)| {
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
            let result = super::draw_asset_image(store.data_mut(), &path, x, y, w, h);
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "draw-asset-image-linear",
        |mut store: StoreContextMut<'_, Data>,
         (path, x, y, w, h): (WasmStr, f32, f32, f32, f32)| {
            let path = path.to_str(store.as_context())?;
            if path.len() > 1024 {
                return Ok((false,));
            }
            let path = path.into_owned();
            let result = super::draw_asset_image_linear(store.data_mut(), &path, x, y, w, h);
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "draw-shader-asset-image",
        |mut store: StoreContextMut<'_, Data>, (index, path): (u32, WasmStr)| {
            let path = path.to_str(store.as_context())?;
            if index > i32::MAX as u32 || path.is_empty() || path.len() > 1024 {
                return Ok((false,));
            }
            let path = path.into_owned();
            Ok((super::draw_shader_asset_image(
                store.data_mut(),
                index as i32,
                &path,
            ),))
        },
    )?;
    for (image_name, linear) in [("draw-image", false), ("draw-image-linear", true)] {
        host.func_wrap(
        image_name,
        move |mut store: StoreContextMut<'_, Data>,
         (x, y, w, h, pw, ph, pixels): (f32, f32, f32, f32, u32, u32, WasmList<u8>)| {
            let Some(bytes) = crate::drawing::image::Image::byte_length(pw, ph) else {
                return Ok((false,));
            };
            if pixels.len() != bytes || !store.data().frame.can_image(x, y, w, h, bytes) {
                return Ok((false,));
            }
            let Some(image) =
                crate::drawing::image::Image::new(pw, ph, pixels.as_le_slice(store.as_context()))
            else {
                return Ok((false,));
            };
            Ok((if linear {store.data_mut().frame.linear_image(x,y,w,h,image)} else {store.data_mut().frame.image(x,y,w,h,image)},))
        },
    )?;
    }
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
            if pixels.len() != bytes || !store.data().frame.can_image(x, y, w, h, bytes) {
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
            if index > i32::MAX as u32 {
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
                || (!store.data().audio.is_empty() && store.data().audio_channels != 1)
                || samples.len() + store.data().audio.len() > 1600
            {
                return Ok((false,));
            }
            store.data_mut().audio.extend(samples);
            Ok((true,))
        },
    )?;
    Ok(linker)
}
