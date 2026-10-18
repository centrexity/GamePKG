//! Host-owned encoded-RGB sampling for game sprites. UI textures keep egui's path.
use crate::drawing::image::Image;
use anyhow::{bail, Context, Result};
use eframe::{
    egui, egui_wgpu,
    glow::{self, HasContext},
    wgpu,
};
use std::sync::Arc;
#[derive(Default)]
pub(super) struct Cache {
    backend: Option<Backend>,
    straight_backend: Option<Backend>,
    slots: Vec<Slot>,
}
enum Backend {
    Vulkan(Vulkan),
    OpenGl(Arc<Gl>),
}
struct Slot {
    straight: bool,
    source: Arc<Image>,
    resource: Resource,
    bounds: [f32; 8],
}
enum Resource {
    Vulkan(Arc<VulkanSlot>),
    OpenGl(Arc<GlSlot>),
}
impl Cache {
    pub fn clear(&mut self) {
        self.slots.clear();
        self.backend = None;
        self.straight_backend = None;
    }
    pub fn finish(&mut self, count: usize) {
        self.slots.truncate(count);
    }
    pub fn callback(
        &mut self,
        index: usize,
        source: (&Arc<Image>, bool),
        bounds: [f32; 4],
        viewport_info: (egui::Rect, f32),
        state: Option<&egui_wgpu::RenderState>,
        gl: Option<&Arc<glow::Context>>,
    ) -> Result<egui::PaintCallback> {
        let (image, straight) = source;
        let (viewport, ppp) = viewport_info;
        let (coverage, texcoords) = sampling_quad(bounds, viewport, ppp);
        let uniform = [
            coverage[0],
            coverage[1],
            coverage[2],
            coverage[3],
            texcoords[0],
            texcoords[1],
            texcoords[2],
            texcoords[3],
        ];
        let selected = if straight {
            &mut self.straight_backend
        } else {
            &mut self.backend
        };
        if selected.is_none() {
            *selected = Some(if let Some(state) = state {
                Backend::Vulkan(Vulkan::new(state, straight)?)
            } else {
                Backend::OpenGl(Arc::new(Gl::new(
                    gl.context("game graphics context missing")?.clone(),
                    straight,
                )?))
            });
        }
        let backend = selected.as_ref().unwrap();
        let changed = self.slots.get(index).is_none_or(|s| {
            s.straight != straight
                || (!Arc::ptr_eq(&s.source, image) && s.source.as_ref() != image.as_ref())
        });
        if changed {
            let pixels: Vec<u8> = if straight {
                image.pixels().to_vec()
            } else {
                image
                    .pixels()
                    .chunks_exact(4)
                    .flat_map(|p| super::game_colors::rgba(p[0], p[1], p[2], p[3]).to_array())
                    .collect()
            };
            let resource = match backend {
                Backend::Vulkan(v) => Resource::Vulkan(Arc::new(v.slot(image.size(), &pixels))),
                Backend::OpenGl(g) => Resource::OpenGl(Arc::new(g.slot(image.size(), &pixels)?)),
            };
            let slot = Slot {
                straight,
                source: image.clone(),
                resource,
                bounds: [f32::NAN; 8],
            };
            if index == self.slots.len() {
                self.slots.push(slot);
            } else {
                self.slots[index] = slot;
            }
        }
        let slot = &mut self.slots[index];
        match (backend, &slot.resource) {
            (Backend::Vulkan(v), Resource::Vulkan(resource)) => {
                if slot.bounds != uniform {
                    let bytes: Vec<u8> = uniform.into_iter().flat_map(f32::to_le_bytes).collect();
                    v.queue.write_buffer(&resource.uniform, 0, &bytes);
                    slot.bounds = uniform;
                }
                Ok(egui_wgpu::Callback::new_paint_callback(
                    viewport,
                    VulkanPass {
                        pipeline: v.pipeline.clone(),
                        slot: resource.clone(),
                    },
                ))
            }
            (Backend::OpenGl(g), Resource::OpenGl(resource)) => {
                let g = g.clone();
                let resource = resource.clone();
                Ok(egui::PaintCallback {
                    rect: viewport,
                    callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |_, _| {
                        g.paint(&resource, texcoords, coverage)
                    })),
                })
            }
            _ => unreachable!("sprite backend stays fixed within a graphics context"),
        }
    }
}
struct Vulkan {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}
struct VulkanSlot {
    _texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}
struct VulkanPass {
    pipeline: wgpu::RenderPipeline,
    slot: Arc<VulkanSlot>,
}
impl egui_wgpu::CallbackTrait for VulkanPass {
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _: &egui_wgpu::CallbackResources,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.slot.bind, &[]);
        pass.draw(0..6, 0..1);
    }
}
impl Vulkan {
    fn new(state: &egui_wgpu::RenderState, straight: bool) -> Result<Self> {
        let device = state.device.clone();
        let queue = state.queue.clone();
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GamePKG encoded sprite"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(32),
                    },
                    count: None,
                },
            ],
        });
        let source=r#"struct Bounds { rect:vec4<f32>,uv:vec4<f32> }; @group(0) @binding(0) var image:texture_2d<f32>; @group(0) @binding(1) var filtering:sampler; @group(0) @binding(2) var<uniform> bounds:Bounds;
struct Vertex { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> };
@vertex fn vertex(@builtin(vertex_index) i:u32)->Vertex {let points=array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(0.,1.),vec2(1.,0.),vec2(1.,1.));var out:Vertex;let point=points[i];out.uv=bounds.uv.xy+point*bounds.uv.zw;out.position=vec4(bounds.rect.xy+point*bounds.rect.zw,0.,1.);return out;}
@fragment fn fragment(input:Vertex)->@location(0) vec4<f32> {var color=textureSample(image,filtering,input.uv);if STRAIGHT_ALPHA {color=vec4(color.rgb*color.a,color.a);}if TARGET_SRGB {let low=color.rgb/12.92;let high=pow((color.rgb+vec3(0.055))/1.055,vec3(2.4));color=vec4(select(high,low,color.rgb<vec3(0.04045)),color.a);}return color;}
"#.replace("TARGET_SRGB",if state.target_format.is_srgb(){"true"}else{"false"}).replace("STRAIGHT_ALPHA",if straight {"true"} else {"false"});
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GamePKG encoded sprite shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GamePKG encoded sprite pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: state.target_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        if let Some(error) = pollster::block_on(device.pop_error_scope()) {
            bail!("Unable to prepare game sprite sampling: {error}");
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            layout,
            sampler,
        })
    }
    fn slot(&self, size: [usize; 2], pixels: &[u8]) -> VulkanSlot {
        let extent = wgpu::Extent3d {
            width: size[0] as u32,
            height: size[1] as u32,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GamePKG encoded sprite pixels"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(extent.width * 4),
                rows_per_image: Some(extent.height),
            },
            extent,
        );
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform.as_entire_binding(),
                },
            ],
        });
        VulkanSlot {
            _texture: texture,
            bind,
            uniform,
        }
    }
}
struct Gl {
    gl: Arc<glow::Context>,
    program: glow::Program,
    buffer: glow::Buffer,
    vao: Option<glow::VertexArray>,
    bounds: glow::UniformLocation,
    sampler: glow::UniformLocation,
    coverage: glow::UniformLocation,
}
struct GlSlot {
    gl: Arc<glow::Context>,
    texture: glow::Texture,
}
impl Drop for GlSlot {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_texture(self.texture);
        }
    }
}
impl Gl {
    fn new(gl: Arc<glow::Context>, straight: bool) -> Result<Self> {
        unsafe {
            let version = gl.version();
            let modern = version.major >= 3;
            let embedded = version.is_embedded;
            let header = if embedded {
                if modern {
                    "#version 300 es\nprecision highp float;\n"
                } else {
                    "#version 100\nprecision mediump float;\n"
                }
            } else if version.major >= 3 {
                if version.major > 3 || version.minor >= 3 {
                    "#version 330 core\n"
                } else if version.minor >= 1 {
                    "#version 140\n"
                } else {
                    "#version 130\n"
                }
            } else {
                "#version 120\n"
            };
            let vertex=format!("{header}{} vec2 uv;uniform vec4 bounds;uniform vec4 coverage;{} vec2 tex;void main(){{vec2 point=coverage.xy+uv*coverage.zw;tex=bounds.xy+uv*bounds.zw;gl_Position=vec4(point,0.,1.);}}",if modern{"in"}else{"attribute"},if modern{"out"}else{"varying"});
            let fragment = format!(
                "{header}{} vec2 tex;uniform sampler2D image;{}void main(){{vec4 value={} (image,tex);{}{}=value;}}",
                if modern { "in" } else { "varying" },
                if modern { "out vec4 pixel;" } else { "" },
                if modern { "texture" } else { "texture2D" },
                if straight {"value.rgb*=value.a;"} else {""},
                if modern { "pixel" } else { "gl_FragColor" }
            );
            let shader = |kind, source: &str| -> Result<glow::Shader> {
                let s = gl.create_shader(kind).map_err(anyhow::Error::msg)?;
                gl.shader_source(s, source);
                gl.compile_shader(s);
                if !gl.get_shader_compile_status(s) {
                    let message = gl.get_shader_info_log(s);
                    gl.delete_shader(s);
                    bail!("Game sprite shader compilation failed: {message}");
                }
                Ok(s)
            };
            let vs = shader(glow::VERTEX_SHADER, &vertex)?;
            let fs = match shader(glow::FRAGMENT_SHADER, &fragment) {
                Ok(s) => s,
                Err(e) => {
                    gl.delete_shader(vs);
                    return Err(e);
                }
            };
            let program = match gl.create_program() {
                Ok(p) => p,
                Err(e) => {
                    gl.delete_shader(vs);
                    gl.delete_shader(fs);
                    bail!(e);
                }
            };
            gl.attach_shader(program, vs);
            gl.attach_shader(program, fs);
            gl.bind_attrib_location(program, 0, "uv");
            gl.link_program(program);
            gl.detach_shader(program, vs);
            gl.detach_shader(program, fs);
            gl.delete_shader(vs);
            gl.delete_shader(fs);
            if !gl.get_program_link_status(program) {
                let message = gl.get_program_info_log(program);
                gl.delete_program(program);
                bail!("Game sprite shader link failed: {message}");
            }
            let Some(bounds) = gl.get_uniform_location(program, "bounds") else {
                gl.delete_program(program);
                bail!("Game sprite bounds missing");
            };
            let Some(coverage) = gl.get_uniform_location(program, "coverage") else {
                gl.delete_program(program);
                bail!("Game sprite coverage missing");
            };
            let Some(sampler) = gl.get_uniform_location(program, "image") else {
                gl.delete_program(program);
                bail!("Game sprite sampler missing");
            };
            let buffer = match gl.create_buffer() {
                Ok(b) => b,
                Err(e) => {
                    gl.delete_program(program);
                    bail!(e);
                }
            };
            // Keep our vertex state separate whenever egui can use a native VAO,
            // including older contexts exposing the ARB/OES extension.
            let vao = if modern
                || [
                    "ARB_vertex_array_object",
                    "GL_ARB_vertex_array_object",
                    "OES_vertex_array_object",
                    "GL_OES_vertex_array_object",
                ]
                .iter()
                .any(|name| gl.supported_extensions().contains(*name))
            {
                match gl.create_vertex_array() {
                    Ok(v) => Some(v),
                    Err(e) => {
                        gl.delete_buffer(buffer);
                        gl.delete_program(program);
                        bail!(e);
                    }
                }
            } else {
                None
            };
            let vertices = [0.0f32, 0., 1., 0., 0., 1., 0., 1., 1., 0., 1., 1.];
            let bytes: Vec<u8> = vertices.into_iter().flat_map(f32::to_ne_bytes).collect();
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(buffer));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            Ok(Self {
                gl,
                program,
                buffer,
                vao,
                bounds,
                sampler,
                coverage,
            })
        }
    }
    fn slot(&self, size: [usize; 2], pixels: &[u8]) -> Result<GlSlot> {
        unsafe {
            let texture = self.gl.create_texture().map_err(anyhow::Error::msg)?;
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                if self.gl.version().is_embedded && self.gl.version().major < 3 {
                    glow::RGBA as i32
                } else {
                    glow::RGBA8 as i32
                },
                size[0] as i32,
                size[1] as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(pixels)),
            );
            for field in [glow::TEXTURE_MIN_FILTER, glow::TEXTURE_MAG_FILTER] {
                self.gl
                    .tex_parameter_i32(glow::TEXTURE_2D, field, glow::LINEAR as i32);
            }
            for field in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
                self.gl
                    .tex_parameter_i32(glow::TEXTURE_2D, field, glow::CLAMP_TO_EDGE as i32);
            }
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            Ok(GlSlot {
                gl: self.gl.clone(),
                texture,
            })
        }
    }
    fn paint(&self, slot: &GlSlot, b: [f32; 4], coverage: [f32; 4]) {
        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_4_f32(Some(&self.bounds), b[0], b[1], b[2], b[3]);
            self.gl.uniform_4_f32(
                Some(&self.coverage),
                coverage[0],
                coverage[1],
                coverage[2],
                coverage[3],
            );
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(slot.texture));
            self.gl.uniform_1_i32(Some(&self.sampler), 0);
            if let Some(vao) = self.vao {
                self.gl.bind_vertex_array(Some(vao));
            }
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer));
            self.gl.enable_vertex_attrib_array(0);
            self.gl
                .vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);
            self.gl.disable_vertex_attrib_array(0);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            if self.vao.is_some() {
                self.gl.bind_vertex_array(None);
            }
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.use_program(None);
        }
    }
}
impl Drop for Gl {
    fn drop(&mut self) {
        unsafe {
            if let Some(vao) = self.vao {
                self.gl.delete_vertex_array(vao);
            }
            self.gl.delete_buffer(self.buffer);
            self.gl.delete_program(self.program);
        }
    }
}

pub(super) fn solid_rectangle(bounds: [f32; 4], viewport: egui::Rect, ppp: f32) -> egui::Rect {
    let (coverage, _) = sampling_quad(bounds, viewport, ppp);
    let left = (viewport.left() * ppp).round();
    let top = (viewport.top() * ppp).round();
    let width = (viewport.right() * ppp).round() - left;
    let height = (viewport.bottom() * ppp).round() - top;
    let x0 = (left + (coverage[0] + 1.0) * width * 0.5).round();
    let y0 = (top + (1.0 - coverage[1]) * height * 0.5).round();
    let x1 = (x0 + coverage[2] * width * 0.5).round();
    let y1 = (y0 - coverage[3] * height * 0.5).round();
    egui::Rect::from_min_max(
        egui::pos2(x0 / ppp, y0 / ppp),
        egui::pos2(x1 / ppp, y1 / ppp),
    )
}

// SDL/OpenGL includes the left and lower edge at exact pixel centers. Move only
// raster coverage to whole pixel edges; original bounds still determine UVs.
fn sampling_quad(bounds: [f32; 4], viewport: egui::Rect, ppp: f32) -> ([f32; 4], [f32; 4]) {
    let left = f64::from((viewport.left() * ppp).round());
    let top = f64::from((viewport.top() * ppp).round());
    let width = f64::from((viewport.right() * ppp).round()) - left;
    let height = f64::from((viewport.bottom() * ppp).round()) - top;
    let empty = ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 1.0, 1.0]);
    if width <= 0.0 || height <= 0.0 {
        return empty;
    }
    let x = left + f64::from(bounds[0]) * (width / 640.0);
    let y = top + f64::from(bounds[1]) * (height / 480.0);
    let w = f64::from(bounds[2]) * (width / 640.0);
    let h = f64::from(bounds[3]) * (height / 480.0);
    let x0 = (x - 0.5).ceil();
    let x1 = (x + w - 0.5).ceil();
    let y0 = (y - 0.5).floor() + 1.0;
    let y1 = (y + h - 0.5).floor() + 1.0;
    if x1 <= x0 || y1 <= y0 || w <= 0.0 || h <= 0.0 {
        return empty;
    }
    (
        [
            ((x0 - left) * 2.0 / width - 1.0) as f32,
            (1.0 - (y0 - top) * 2.0 / height) as f32,
            ((x1 - x0) * 2.0 / width) as f32,
            (-(y1 - y0) * 2.0 / height) as f32,
        ],
        [
            ((x0 - x) / w) as f32,
            ((y0 - y) / h) as f32,
            ((x1 - x0) / w) as f32,
            ((y1 - y0) / h) as f32,
        ],
    )
}

#[cfg(test)]
#[path = "linear_sampling_tests.rs"]
mod tests;
