//! Fixed host attachment commands. No game-supplied programs or GPU handles.
use super::*;
use std::sync::mpsc::{Receiver, TryRecvError};

pub(super) struct Target {
    pub depth: std::sync::Arc<Depth>,
    pub multisample: Option<wgpu::Texture>,
    pub private_depth_cost: u64,
}
pub(super) struct Visibility {
    pub set: wgpu::QuerySet,
    pub resolve: wgpu::Buffer,
    pub read: wgpu::Buffer,
    pub pending: Option<Receiver<bool>>,
    pub result: Option<u64>,
}
impl Graphics {
    pub(super) fn presentation_lease(&self, bytes: u64) -> std::sync::Arc<PresentationLease> {
        self.presentations
            .fetch_add(bytes, std::sync::atomic::Ordering::Relaxed);
        let lease = std::sync::Arc::new(PresentationLease {
            bytes,
            live: self.presentations.clone(),
        });
        let pending = lease.clone();
        self.queue.on_submitted_work_done(move || drop(pending));
        lease
    }
    pub(super) fn submission_room(&self) -> Result<()> {
        if self.submissions.load(std::sync::atomic::Ordering::Relaxed) >= 64 {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    pub(super) fn submit(&self, encoder: wgpu::CommandEncoder) {
        self.submissions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.queue.submit([encoder.finish()]);
        let count = self.submissions.clone();
        self.queue.on_submitted_work_done(move || {
            count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
    }
    pub(super) fn retire(&mut self, cost: u64) {
        let (send, receive) = std::sync::mpsc::channel();
        self.queue.on_submitted_work_done(move || {
            let _ = send.send(());
        });
        self.retired.push((cost, receive));
    }
    pub(super) fn reap(&mut self) {
        let _ = self.device.poll(wgpu::Maintain::Poll);
        self.retired.retain(|(cost, receive)| {
            if receive.try_recv().is_ok() {
                self.used -= cost;
                false
            } else {
                true
            }
        });
        self.poll_variants();
    }
    pub(super) fn retained_call(
        &mut self,
        op: u32,
        bytes: &[u8],
        capacity: usize,
    ) -> Result<Vec<u8>> {
        if op >= 116 {
            return self.state_call(op, bytes, capacity);
        }
        if matches!(op, 106 | 107 | 108 | 110 | 115) {
            self.submission_room()?;
        }
        let mut r = Reader(bytes);
        match op {
            106 => {
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let width = r.u32()?;
                let height = r.u32()?;
                r.end()?;
                let limit = self.device.limits().max_texture_dimension_2d.min(8192);
                if width == 0 || height == 0 || width > limit || height > limit {
                    return Err(Error::Invalid);
                }
                let cost = u64::from(width)
                    * u64::from(height)
                    * if self.samples == 1 {
                        8
                    } else {
                        4 + 8 * u64::from(self.samples)
                    };
                self.reserve(cost)?;
                let id = identifier()?;
                let make = |format, samples, usage| {
                    self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("GamePKG retained attachment"),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage,
                        view_formats: if format == wgpu::TextureFormat::Rgba8Unorm {
                            &[wgpu::TextureFormat::Rgba8UnormSrgb]
                        } else {
                            &[]
                        },
                    })
                };
                let gpu = make(
                    wgpu::TextureFormat::Rgba8Unorm,
                    1,
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | wgpu::TextureUsages::COPY_DST,
                );
                let depth = make(
                    wgpu::TextureFormat::Depth32Float,
                    self.samples,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                );
                let multisample = (self.samples > 1).then(|| {
                    make(
                        wgpu::TextureFormat::Rgba8Unorm,
                        self.samples,
                        wgpu::TextureUsages::RENDER_ATTACHMENT,
                    )
                });
                self.textures.insert(
                    id,
                    Texture {
                        gpu,
                        width,
                        height,
                        layers: 1,
                        mips: 1,
                        dimension: wgpu::TextureViewDimension::D2,
                        bytes: cost,
                    },
                );
                self.targets.insert(
                    id,
                    Target {
                        depth: Depth::private(depth, width, height),
                        multisample,
                        private_depth_cost: u64::from(width)
                            * u64::from(height)
                            * 4
                            * u64::from(self.samples),
                    },
                );
                self.used += cost;
                // Deterministic initial contents, including depth=1 and MSAA.
                let request = [
                    width.to_le_bytes(),
                    height.to_le_bytes(),
                    0u32.to_le_bytes(),
                    1f32.to_bits().to_le_bytes(),
                    0u32.to_le_bytes(),
                ]
                .concat();
                self.render_inner(&request, Some(id), 3, None, 0)?;
                Ok(id.to_le_bytes().to_vec())
            }
            108 | 115 => {
                let id = r.u64()?;
                let query = if op == 115 { Some(r.u64()?) } else { None };
                let flags = r.u32()?;
                if flags > 3 {
                    return Err(Error::Invalid);
                }
                let t = self.target(id)?;
                let request = [
                    t.width.to_le_bytes().as_slice(),
                    &t.height.to_le_bytes(),
                    r.0,
                ]
                .concat();
                self.render_inner(&request, Some(id), flags, query, 0)?;
                Ok(Vec::new())
            }
            107 => {
                let source = r.u64()?;
                let destination = r.u64()?;
                let sx = r.u32()?;
                let sy = r.u32()?;
                let dx = r.u32()?;
                let dy = r.u32()?;
                let width = r.u32()?;
                let height = r.u32()?;
                r.end()?;
                let src = self.target(source)?;
                let dst = self.target(destination)?;
                rect(src, [sx, sy, width, height])?;
                rect(dst, [dx, dy, width, height])?;
                // Snapshot before copying, including self-copy/overlap. The
                // transient is budget checked; commands retain it until done.
                let cost = u64::from(width) * u64::from(height) * 4;
                self.reserve(cost)?;
                let snapshot = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("GamePKG bounded copy snapshot"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &src.gpu,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x: sx, y: sy, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    snapshot.as_image_copy(),
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                self.rectangle(
                    &mut encoder,
                    destination,
                    [dx, dy, width, height],
                    1,
                    0,
                    0.0,
                    Some(&snapshot),
                    0,
                )?;
                self.submit(encoder);
                self.used += cost;
                self.retire(cost);
                Ok(Vec::new())
            }
            110 => {
                let target = r.u64()?;
                let flags = r.u32()?;
                let area = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
                let color = r.u32()?;
                let depth = f32::from_bits(r.u32()?);
                r.end()?;
                if !(1..=3).contains(&flags) || !depth.is_finite() || !(0.0..=1.0).contains(&depth)
                {
                    return Err(Error::Invalid);
                }
                rect(self.target(target)?, area)?;
                let mut encoder = self.device.create_command_encoder(&Default::default());
                self.rectangle(&mut encoder, target, area, flags, color, depth, None, 0)?;
                self.submit(encoder);
                Ok(Vec::new())
            }
            111 => {
                r.end()?;
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                if !self.precise_occlusion {
                    return Err(Error::Unsupported);
                }
                if self.queries.len() >= 64 {
                    return Err(Error::Busy);
                }
                self.reserve(512)?;
                let id = identifier()?;
                let set = self.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("GamePKG visibility"),
                    ty: wgpu::QueryType::Occlusion,
                    count: 1,
                });
                let resolve = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 256,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                });
                let read = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size: 256,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                });
                self.queries.insert(
                    id,
                    Visibility {
                        set,
                        resolve,
                        read,
                        pending: None,
                        result: None,
                    },
                );
                self.used += 512;
                Ok(id.to_le_bytes().to_vec())
            }
            113 => {
                let id = r.u64()?;
                r.end()?;
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let q = self.queries.get_mut(&id).ok_or(Error::Denied)?;
                if let Some(receive) = &q.pending {
                    match receive.try_recv() {
                        Ok(true) => {
                            let mapped = q.read.slice(..8).get_mapped_range();
                            q.result = Some(u64::from_le_bytes(mapped[..8].try_into().unwrap()));
                            drop(mapped);
                            q.read.unmap();
                            q.pending = None;
                        }
                        Ok(false) | Err(TryRecvError::Disconnected) => {
                            q.pending = None;
                            return Err(Error::Failed);
                        }
                        Err(TryRecvError::Empty) => return Err(Error::Busy),
                    }
                }
                let count = q.result.take().ok_or(Error::Busy)?;
                Ok(count.to_le_bytes().to_vec())
            }
            _ => Err(Error::Unsupported),
        }
    }
    pub(super) fn target(&self, id: u64) -> Result<&Texture> {
        if !self.targets.contains_key(&id) {
            return Err(Error::Denied);
        }
        self.textures.get(&id).ok_or(Error::Denied)
    }
    /// Snapshot presentation so later writes/releases cannot change an accepted
    /// displayed frame. Desktop admits only one pending snapshot.
    pub fn present_target(&mut self, request: &[u8]) -> Result<Presented> {
        self.reap();
        self.submission_room()?;
        let mut r = Reader(request);
        let id = r.u64()?;
        r.end()?;
        let t = self.target(id)?;
        let cost = u64::from(t.width) * u64::from(t.height) * 4;
        self.reserve(cost)?;
        let gpu = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GamePKG retained presentation snapshot"),
            size: t.gpu.size(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_texture(t.gpu.as_image_copy(), gpu.as_image_copy(), t.gpu.size());
        self.submit(encoder);
        Ok(Presented {
            width: t.width,
            height: t.height,
            view: gpu.create_view(&wgpu::TextureViewDescriptor {
                format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                ..Default::default()
            }),
            texture: gpu,
            _lease: Some(self.presentation_lease(cost)),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn rectangle(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        id: u64,
        area: [u32; 4],
        flags: u32,
        color: u32,
        depth: f32,
        source: Option<&wgpu::Texture>,
        stencil: u32,
    ) -> Result<()> {
        let target = self.target(id)?;
        let attachment = self.targets.get(&id).ok_or(Error::Denied)?;
        let color_view = target.gpu.create_view(&Default::default());
        let ms_view = attachment
            .multisample
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        let depth_view = attachment.depth.gpu.create_view(&Default::default());
        let depth_format = attachment.depth.format;
        if flags & 4 != 0 && !depth_format.has_stencil_aspect() {
            return Err(Error::Invalid);
        }
        let key = flags
            | if source.is_some() { 8 } else { 0 }
            | ((match depth_format {
                wgpu::TextureFormat::Depth24PlusStencil8 => 1,
                wgpu::TextureFormat::Depth32FloatStencil8 => 2,
                _ => 0,
            }) << 8);
        let pipeline = if let Some(pipeline) = self.rectangle_pipelines.get(&key) {
            pipeline.clone()
        } else {
            let vertex = "@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{var p=array<vec2<f32>,3>(vec2<f32>(-1.,-1.),vec2<f32>(3.,-1.),vec2<f32>(-1.,3.));return vec4<f32>(p[i],0.,1.);}";
            let fragment = if source.is_some() {
                "@group(0) @binding(0) var<uniform> v:array<vec4<f32>,2>; @group(0) @binding(1) var t:texture_2d<f32>; @fragment fn fs(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32>{return textureLoad(t,vec2<i32>(p.xy)-vec2<i32>(v[1].yz),0);}"
            } else {
                "@group(0) @binding(0) var<uniform> v:array<vec4<f32>,2>; struct O{@location(0) c:vec4<f32>,@builtin(frag_depth) d:f32}; @fragment fn fs()->O{var o:O;o.c=v[0];o.d=v[1].x;return o;}"
            };
            let shader = self
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("GamePKG fixed attachment shader"),
                    source: wgpu::ShaderSource::Wgsl(format!("{vertex}\n{fragment}").into()),
                });
            let pipeline = self
                .device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("GamePKG attachment operation"),
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            blend: None,
                            write_mask: if flags & 1 != 0 {
                                wgpu::ColorWrites::ALL
                            } else {
                                wgpu::ColorWrites::empty()
                            },
                        })],
                    }),
                    primitive: Default::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: depth_format,
                        depth_write_enabled: flags & 2 != 0,
                        depth_compare: wgpu::CompareFunction::Always,
                        stencil: if flags & 4 != 0 {
                            let face = wgpu::StencilFaceState {
                                compare: wgpu::CompareFunction::Always,
                                fail_op: wgpu::StencilOperation::Keep,
                                depth_fail_op: wgpu::StencilOperation::Keep,
                                pass_op: wgpu::StencilOperation::Replace,
                            };
                            wgpu::StencilState {
                                front: face,
                                back: face,
                                read_mask: 255,
                                write_mask: 255,
                            }
                        } else {
                            Default::default()
                        },
                        bias: Default::default(),
                    }),
                    multisample: wgpu::MultisampleState {
                        count: self.samples,
                        ..Default::default()
                    },
                    multiview: None,
                    cache: None,
                });
            self.rectangle_pipelines.insert(key, pipeline.clone());
            pipeline
        };
        let mut data = Vec::new();
        for f in color
            .to_be_bytes()
            .map(|c| f32::from(c) / 255.)
            .into_iter()
            .chain([depth, area[0] as f32, area[1] as f32, 0.])
        {
            data.extend(f.to_le_bytes());
        }
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &data,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let view = source.map(|t| t.create_view(&Default::default()));
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }];
        if let Some(view) = &view {
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            });
        }
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GamePKG attachment rectangle"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: ms_view.as_ref().unwrap_or(&color_view),
                resolve_target: ms_view.as_ref().map(|_| &color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: depth_format
                    .has_stencil_aspect()
                    .then_some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_stencil_reference(stencil);
        pass.set_bind_group(0, &bind, &[]);
        pass.set_scissor_rect(area[0], area[1], area[2], area[3]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}
pub(super) fn rect(t: &Texture, r: [u32; 4]) -> Result<()> {
    if r[2] == 0
        || r[3] == 0
        || r[0].checked_add(r[2]).is_none_or(|e| e > t.width)
        || r[1].checked_add(r[3]).is_none_or(|e| e > t.height)
    {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
