use super::*;
/// Host-verified state variants, never arbitrary game shader/pipeline input.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RasterState {
    pub topology: wgpu::PrimitiveTopology,
    pub front_face: wgpu::FrontFace,
    pub color_writes: wgpu::ColorWrites,
    pub depth_format: wgpu::TextureFormat,
    pub bias: wgpu::DepthBiasState,
    pub stencil: wgpu::StencilState,
}
impl Default for RasterState {
    fn default() -> Self {
        Self {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            color_writes: wgpu::ColorWrites::ALL,
            depth_format: wgpu::TextureFormat::Depth32Float,
            bias: Default::default(),
            stencil: Default::default(),
        }
    }
}
impl RasterState {
    pub(super) fn validate(&self) -> bool {
        matches!(
            self.topology,
            wgpu::PrimitiveTopology::PointList
                | wgpu::PrimitiveTopology::LineList
                | wgpu::PrimitiveTopology::TriangleList
        ) && matches!(
            self.depth_format,
            wgpu::TextureFormat::Depth32Float
                | wgpu::TextureFormat::Depth24PlusStencil8
                | wgpu::TextureFormat::Depth32FloatStencil8
        ) && self.color_writes.bits() <= 15
            && self.stencil.read_mask <= 255
            && self.stencil.write_mask <= 255
            && self.bias.slope_scale.is_finite()
            && self.bias.clamp.is_finite()
            && (self.depth_format.has_stencil_aspect()
                || self.stencil == wgpu::StencilState::default())
    }
}

pub(super) struct Depth {
    pub gpu: wgpu::Texture,
    pub format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    _lease: Option<DepthLease>,
}
struct DepthLease {
    bytes: u64,
    live: std::sync::Arc<std::sync::atomic::AtomicU64>,
    queue: wgpu::Queue,
}
pub(super) struct Readback {
    buffer: wgpu::Buffer,
    receive: std::sync::mpsc::Receiver<bool>,
    width: u32,
    height: u32,
    row: u32,
    pub cost: u64,
}
impl Drop for DepthLease {
    fn drop(&mut self) {
        let live = self.live.clone();
        let bytes = self.bytes;
        self.queue.on_submitted_work_done(move || {
            live.fetch_sub(bytes, std::sync::atomic::Ordering::Relaxed);
        });
    }
}
impl Depth {
    pub(super) fn private(gpu: wgpu::Texture, width: u32, height: u32) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            format: gpu.format(),
            gpu,
            width,
            height,
            _lease: None,
        })
    }
}
impl Graphics {
    pub(super) fn state_call(&mut self, op: u32, bytes: &[u8], capacity: usize) -> Result<Vec<u8>> {
        let mut r = Reader(bytes);
        match op {
            116 => {
                let source = r.u64()?;
                let destination = r.u64()?;
                let sx = r.u32()?;
                let sy = r.u32()?;
                let mip = r.u32()?;
                let layer = r.u32()?;
                let dx = r.u32()?;
                let dy = r.u32()?;
                let width = r.u32()?;
                let height = r.u32()?;
                r.end()?;
                let source = self.target(source)?;
                if self.targets.contains_key(&destination) {
                    return Err(Error::Invalid);
                }
                let destination = self.textures.get(&destination).ok_or(Error::Denied)?;
                super::retained::rect(source, [sx, sy, width, height])?;
                if mip >= destination.mips
                    || layer >= destination.layers
                    || dx
                        .checked_add(width)
                        .is_none_or(|end| end > (destination.width >> mip).max(1))
                    || dy
                        .checked_add(height)
                        .is_none_or(|end| end > (destination.height >> mip).max(1))
                {
                    return Err(Error::Invalid);
                }
                // A raw color copy is byte preserving; view color-space remains
                // the destination texture's explicitly chosen sampling format.
                self.submission_room()?;
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &source.gpu,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: sx, y: sy, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &destination.gpu,
                        mip_level: mip,
                        origin: wgpu::Origin3d {
                            x: dx,
                            y: dy,
                            z: layer,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                self.submit(encoder);
                Ok(Vec::new())
            }
            118 => {
                let width = r.u32()?;
                let height = r.u32()?;
                let format = match r.u32()? {
                    0 => wgpu::TextureFormat::Depth32Float,
                    1 => wgpu::TextureFormat::Depth24PlusStencil8,
                    2 => wgpu::TextureFormat::Depth32FloatStencil8,
                    _ => return Err(Error::Invalid),
                };
                r.end()?;
                if capacity < 8
                    || width == 0
                    || height == 0
                    || width > self.device.limits().max_texture_dimension_2d.min(8192)
                    || height > self.device.limits().max_texture_dimension_2d.min(8192)
                {
                    return Err(Error::Invalid);
                }
                if format == wgpu::TextureFormat::Depth24PlusStencil8 && !self.stencil_samples {
                    return Err(Error::Unsupported);
                }
                if format == wgpu::TextureFormat::Depth32FloatStencil8 && !self.stencil32_samples {
                    return Err(Error::Unsupported);
                }
                let cost = u64::from(width)
                    * u64::from(height)
                    * u64::from(self.samples)
                    * if format == wgpu::TextureFormat::Depth32Float {
                        4
                    } else {
                        8
                    };
                self.reserve(cost)?;
                self.submission_room()?;
                let id = identifier()?;
                let gpu = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("GamePKG independent depth/stencil"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: self.samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = gpu.create_view(&Default::default());
                let mut encoder = self.device.create_command_encoder(&Default::default());
                {
                    let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: format.has_stencil_aspect().then_some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(0),
                                store: wgpu::StoreOp::Store,
                            }),
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                }
                self.submit(encoder);
                self.presentations
                    .fetch_add(cost, std::sync::atomic::Ordering::Relaxed);
                let depth = std::sync::Arc::new(Depth {
                    gpu,
                    format,
                    width,
                    height,
                    _lease: Some(DepthLease {
                        bytes: cost,
                        live: self.presentations.clone(),
                        queue: self.queue.clone(),
                    }),
                });
                self.depths.insert(id, depth);
                Ok(id.to_le_bytes().to_vec())
            }
            119 => {
                let target = r.u64()?;
                let depth = r.u64()?;
                r.end()?;
                let color = self.target(target)?;
                let depth = self.depths.get(&depth).ok_or(Error::Denied)?.clone();
                if depth.width != color.width || depth.height != color.height {
                    return Err(Error::Invalid);
                }
                let attachment = self.targets.get_mut(&target).unwrap();
                attachment.depth = depth;
                let retired = std::mem::take(&mut attachment.private_depth_cost);
                self.textures.get_mut(&target).unwrap().bytes -= retired;
                if retired != 0 {
                    self.retire(retired);
                }
                Ok(Vec::new())
            }
            120 => {
                let id = r.u64()?;
                let flags = r.u32()?;
                let area = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                let color = r.u32()?;
                let depth = f32::from_bits(r.u32()?);
                let stencil = r.u32()?;
                r.end()?;
                if !(1..=7).contains(&flags)
                    || !depth.is_finite()
                    || !(0.0..=1.0).contains(&depth)
                    || stencil > 255
                {
                    return Err(Error::Invalid);
                }
                super::retained::rect(self.target(id)?, area)?;
                self.submission_room()?;
                let mut encoder = self.device.create_command_encoder(&Default::default());
                self.rectangle(&mut encoder, id, area, flags, color, depth, None, stencil)?;
                self.submit(encoder);
                Ok(Vec::new())
            }
            122 | 135 => {
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let min = r.u32()?;
                let mag = r.u32()?;
                let mip = r.u32()?;
                let address = [r.u32()?, r.u32()?, r.u32()?];
                let lod_min = f32::from_bits(r.u32()?);
                let lod_max = f32::from_bits(r.u32()?);
                let anisotropy = r.u32()?;
                let border = if op == 135 { Some(r.u32()?) } else { None };
                r.end()?;
                if min > 1
                    || mag > 1
                    || mip > 1
                    || address.iter().any(|v| *v > if op == 135 { 3 } else { 2 })
                    || border.is_some_and(|v| v > 2)
                    || !lod_min.is_finite()
                    || !lod_max.is_finite()
                    || lod_min < 0.
                    || lod_max < lod_min
                    || lod_max > 32.
                    || !(1..=16).contains(&anisotropy)
                    || (anisotropy > 1 && (min != 1 || mag != 1 || mip != 1))
                {
                    return Err(Error::Invalid);
                }
                if anisotropy > 1 && !self.anisotropy {
                    return Err(Error::Unsupported);
                }
                if address.contains(&3)
                    && !self
                        .device
                        .features()
                        .contains(wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER)
                {
                    return Err(Error::Unsupported);
                }
                self.reserve(256)?;
                let filter = |v| {
                    if v == 0 {
                        wgpu::FilterMode::Nearest
                    } else {
                        wgpu::FilterMode::Linear
                    }
                };
                let mode = |v| match v {
                    0 => wgpu::AddressMode::ClampToEdge,
                    1 => wgpu::AddressMode::Repeat,
                    2 => wgpu::AddressMode::MirrorRepeat,
                    _ => wgpu::AddressMode::ClampToBorder,
                };
                let id = identifier()?;
                let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                    label: Some("GamePKG bounded sampler"),
                    min_filter: filter(min),
                    mag_filter: filter(mag),
                    mipmap_filter: filter(mip),
                    address_mode_u: mode(address[0]),
                    address_mode_v: mode(address[1]),
                    address_mode_w: mode(address[2]),
                    lod_min_clamp: lod_min,
                    lod_max_clamp: lod_max,
                    anisotropy_clamp: anisotropy as u16,
                    border_color: if address.contains(&3) {
                        Some(match border.unwrap_or(0) {
                            0 => wgpu::SamplerBorderColor::TransparentBlack,
                            1 => wgpu::SamplerBorderColor::OpaqueBlack,
                            _ => wgpu::SamplerBorderColor::OpaqueWhite,
                        })
                    } else {
                        None
                    },
                    ..Default::default()
                });
                self.sampler_ids.insert(id, sampler);
                self.used += 256;
                Ok(id.to_le_bytes().to_vec())
            }
            123 | 129 => {
                let target = r.u64()?;
                let query = r.u64()?;
                let mask = r.u32()?;
                if mask > 3 {
                    return Err(Error::Invalid);
                }
                let t = self.target(target)?;
                let request = [
                    t.width.to_le_bytes().as_slice(),
                    &t.height.to_le_bytes(),
                    r.0,
                ]
                .concat();
                self.render_inner(
                    &request,
                    Some(target),
                    mask,
                    (query != 0).then_some(query),
                    if op == 129 { 3 } else { 1 },
                )?;
                Ok(Vec::new())
            }
            124 => {
                let id = r.u64()?;
                let area = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                r.end()?;
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let t = self.target(id)?;
                super::retained::rect(t, area)?;
                let row = (area[2] * 4).div_ceil(256) * 256;
                let bytes = u64::from(row) * u64::from(area[3]);
                if bytes > super::super::MAX_MESSAGE as u64 {
                    return Err(Error::Invalid);
                }
                if self.readbacks.len() >= 8 {
                    return Err(Error::Busy);
                }
                self.reserve(bytes * 2)?;
                self.submission_room()?;
                let handle = identifier()?;
                let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("GamePKG bounded surface readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_buffer(
                    wgpu::TexelCopyTextureInfo {
                        texture: &t.gpu,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: area[0],
                            y: area[1],
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyBufferInfo {
                        buffer: &buffer,
                        layout: wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(row),
                            rows_per_image: Some(area[3]),
                        },
                    },
                    wgpu::Extent3d {
                        width: area[2],
                        height: area[3],
                        depth_or_array_layers: 1,
                    },
                );
                self.submit(encoder);
                let (send, receive) = std::sync::mpsc::sync_channel(1);
                buffer
                    .slice(..)
                    .map_async(wgpu::MapMode::Read, move |result| {
                        let _ = send.send(result.is_ok());
                    });
                self.readbacks.insert(
                    handle,
                    Readback {
                        buffer,
                        receive,
                        width: area[2],
                        height: area[3],
                        row,
                        cost: bytes * 2,
                    },
                );
                self.used += bytes * 2;
                Ok(handle.to_le_bytes().to_vec())
            }
            125 => {
                let id = r.u64()?;
                r.end()?;
                let readback = self.readbacks.get(&id).ok_or(Error::Denied)?;
                if u64::from(readback.width) * u64::from(readback.height) * 4 > capacity as u64 {
                    return Err(Error::Invalid);
                }
                match readback.receive.try_recv() {
                    Err(std::sync::mpsc::TryRecvError::Empty) => return Err(Error::Busy),
                    Ok(true) => {}
                    _ => {
                        let readback = self.readbacks.remove(&id).unwrap();
                        self.retire(readback.cost);
                        return Err(Error::Failed);
                    }
                }
                let readback = self.readbacks.remove(&id).unwrap();
                let mapped = readback.buffer.slice(..).get_mapped_range();
                let mut bytes =
                    Vec::with_capacity(readback.width as usize * readback.height as usize * 4);
                for row in mapped.chunks_exact(readback.row as usize) {
                    bytes.extend(&row[..readback.width as usize * 4]);
                }
                drop(mapped);
                readback.buffer.unmap();
                self.retire(readback.cost);
                Ok(bytes)
            }
            _ => Err(Error::Unsupported),
        }
    }
}
