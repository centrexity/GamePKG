//! Vulkan-backed host-owned fragment pipelines. Games never receive a GPU device.
use anyhow::{ensure, Result};
use eframe::{egui, egui_wgpu, wgpu};
use std::sync::Arc;
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipelines: Vec<wgpu::RenderPipeline>,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}
impl Renderer {
    pub fn new(state: &egui_wgpu::RenderState, sources: &[String]) -> Result<Self> {
        ensure!(
            state.adapter.get_info().backend == wgpu::Backend::Vulkan,
            "The selected renderer is not Vulkan"
        );
        let device = state.device.clone();
        let queue = state.queue.clone();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("GamePKG bounded shader texture"),
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GamePKG fragment layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let vertex=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("GamePKG fullscreen triangle"),source:wgpu::ShaderSource::Wgsl("@vertex fn vertex(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32>{let x=f32((i<<1u)&2u);let y=f32(i&2u);return vec4<f32>(x*2.0-1.0,y*2.0-1.0,0.0,1.0);}".into())});
        let mut pipelines = Vec::new();
        for source in sources {
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Verified GamePKG fragment"),
                source: wgpu::ShaderSource::Wgsl(source.as_str().into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("GamePKG Vulkan fragment"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex,
                    entry_point: Some("vertex"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment,
                    entry_point: None,
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: state.target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                anyhow::bail!("Vulkan shader validation failed: {error}");
            }
            pipelines.push(pipeline);
        }
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("GamePKG texture sampler"),
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            pipelines,
            layout,
            sampler,
        })
    }
    pub fn callback(
        self: &Arc<Self>,
        rect: egui::Rect,
        index: usize,
        image: Option<&Arc<crate::drawing::image::Image>>,
    ) -> Option<egui::PaintCallback> {
        let pipeline = self.pipelines.get(index)?.clone();
        let (w, h, pixels) = image
            .map(|i| {
                let [w, h] = i.size();
                (w as u32, h as u32, i.pixels())
            })
            .unwrap_or((1, 1, &[255, 255, 255, 255]));
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("GamePKG bounded texture"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
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
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&Default::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("GamePKG texture binding"),
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
            ],
        });
        Some(egui_wgpu::Callback::new_paint_callback(
            rect,
            Pass { pipeline, bind },
        ))
    }
}
struct Pass {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
}
impl egui_wgpu::CallbackTrait for Pass {
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _: &egui_wgpu::CallbackResources,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
