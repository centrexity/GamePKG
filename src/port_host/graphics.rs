//! Host-owned indexed native rendering. Pipelines/layouts come from verified
//! host shader preparation; games supply only opaque resources and draw data.
use super::{identifier, Error, Result};
use eframe::{egui_wgpu, wgpu};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;
mod retained;
mod state;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod takedawn;
mod variants;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod verified;
use retained::{Target, Visibility};
pub use state::RasterState;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use takedawn::takedawn_nvi1_recipes;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) use verified::PipelinePayload;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use verified::{PipelineRecipe, VerifiedPipelines};
/// Structured data admitted for a particular signed, verified shader template.
/// This validates representation, not NV2A execution/translation or fidelity.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum UniformPolicy {
    #[default]
    Opaque,
    Nv2aV1 {
        cube: [bool; 4],
    },
}
use state::{Depth, Readback};

const MAX_GPU_HANDLES: usize = 4096;

/// Host bootstrap input, never deserialized from a game request. Sources must
/// be the verified WGSL output of the signed shader/Slang preparation path.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    pub vertex: String,
    pub vertex_entry: String,
    pub fragment: String,
    pub fragment_entry: String,
    pub stride: u64,
    pub attributes: Vec<wgpu::VertexAttribute>,
    pub uniform_bytes: u64,
    /// One filterable RGBA texture/sampler pair per slot, up to eight.
    pub textures: Vec<wgpu::TextureViewDimension>,
    pub blend: Option<wgpu::BlendState>,
    pub cull: Option<wgpu::Face>,
    pub depth_write: bool,
    pub depth_compare: wgpu::CompareFunction,
    pub raster: RasterState,
    pub uniform_policy: UniformPolicy,
}
impl Pipeline {
    fn validate_layout(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.raster.validate(), "invalid verified raster state");
        if let UniformPolicy::Nv2aV1 { cube } = &self.uniform_policy {
            anyhow::ensure!(
                self.uniform_bytes == crate::shaders::nv2a::BYTES as u64
                    && self.stride == 256
                    && self.attributes.len() == 16
                    && self
                        .attributes
                        .iter()
                        .enumerate()
                        .all(|(i, a)| a.offset == i as u64 * 16
                            && a.shader_location == i as u32
                            && a.format == wgpu::VertexFormat::Float32x4)
                    && self.textures.len() == 4
                    && self.textures.iter().zip(cube).all(|(d, c)| *d
                        == if *c {
                            wgpu::TextureViewDimension::Cube
                        } else {
                            wgpu::TextureViewDimension::D2
                        }),
                "NVI1 requires its exact verified layout"
            );
        }
        anyhow::ensure!(
            self.stride > 0
                && self.stride <= 256
                && self.stride % 4 == 0
                && self.uniform_bytes > 0
                && self.uniform_bytes <= 65536
                && self.uniform_bytes % 16 == 0
                && self.textures.len() <= 8,
            "invalid host pipeline layout"
        );
        anyhow::ensure!(
            self.attributes.len() <= 16
                && self.attributes.iter().all(|a| a
                    .offset
                    .checked_add(a.format.size())
                    .is_some_and(|end| end <= self.stride))
                && self.textures.iter().all(|d| matches!(
                    d,
                    wgpu::TextureViewDimension::D2 | wgpu::TextureViewDimension::Cube
                )),
            "invalid host attributes/textures"
        );
        Ok(())
    }

    pub fn identity_with_samples(&self, samples: u32) -> String {
        crate::verify::digest(format!("{}|samples={samples}", self.identity()).as_bytes())
    }
    /// Exact verified program/layout/render-state identity, in pipeline-index
    /// order. Include these hashes in the receipt, not just shader filenames.
    pub fn identity(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        hash.update(b"GamePKG native pipeline v1\0");
        hash.update(crate::shaders::NATIVE_PREPARATION_POLICY.as_bytes());
        for part in [
            &self.vertex,
            &self.vertex_entry,
            &self.fragment,
            &self.fragment_entry,
        ] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part.as_bytes());
        }
        hash.update(self.stride.to_le_bytes());
        hash.update(self.uniform_bytes.to_le_bytes());
        hash.update(format!("{:?}", self.raster).as_bytes());
        hash.update(format!("{:?}", self.uniform_policy).as_bytes());
        // Enum spellings and layout order are part of the host SDK identity.
        hash.update(
            format!(
                "{:?}|{:?}|{:?}|{:?}|{}|{:?}",
                self.attributes,
                self.textures,
                self.blend,
                self.cull,
                self.depth_write,
                self.depth_compare
            )
            .as_bytes(),
        );
        hex::encode(hash.finalize())
    }
}
#[derive(Clone)]
struct Program {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    stride: u64,
    uniform: u64,
    textures: Vec<wgpu::TextureViewDimension>,
    topology: wgpu::PrimitiveTopology,
    depth_format: wgpu::TextureFormat,
    uniform_policy: UniformPolicy,
}
struct Buffer {
    gpu: wgpu::Buffer,
    kind: u32,
    len: u64,
    shadow: Vec<u8>,
    validation: std::cell::RefCell<BTreeMap<u8, Result<()>>>,
    nan: Vec<u8>,
    nan_count: u64,
    cost: u64,
}
struct Texture {
    gpu: wgpu::Texture,
    width: u32,
    height: u32,
    layers: u32,
    mips: u32,
    dimension: wgpu::TextureViewDimension,
    bytes: u64,
}
pub struct Presented {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    _lease: Option<std::sync::Arc<PresentationLease>>,
}
struct PresentationLease {
    bytes: u64,
    live: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
impl Drop for PresentationLease {
    fn drop(&mut self) {
        self.live
            .fetch_sub(self.bytes, std::sync::atomic::Ordering::Relaxed);
    }
}
pub struct Graphics {
    device: wgpu::Device,
    queue: wgpu::Queue,
    programs: Vec<Program>,
    buffers: BTreeMap<u64, Buffer>,
    textures: BTreeMap<u64, Texture>,
    used: u64,
    budget: u64,
    samples: u32,
    samplers: BTreeMap<u32, wgpu::Sampler>,
    targets: BTreeMap<u64, Target>,
    queries: BTreeMap<u64, Visibility>,
    depths: BTreeMap<u64, std::sync::Arc<Depth>>,
    sampler_ids: BTreeMap<u64, wgpu::Sampler>,
    readbacks: BTreeMap<u64, Readback>,
    anisotropy: bool,
    stencil_samples: bool,
    stencil32_samples: bool,
    retired: Vec<(u64, std::sync::mpsc::Receiver<()>)>,
    presentations: std::sync::Arc<std::sync::atomic::AtomicU64>,
    submissions: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    pipeline_hashes: Vec<String>,
    rectangle_pipelines: BTreeMap<u32, wgpu::RenderPipeline>,
    precise_occlusion: bool,
    templates: Vec<std::sync::Arc<variants::Template>>,
    variants: BTreeMap<u64, variants::Variant>,
    preparing: std::sync::Arc<std::sync::atomic::AtomicBool>,
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let b = self.0.get(..n).ok_or(Error::Invalid)?;
        self.0 = &self.0[n..];
        Ok(b)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn end(&self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
}
impl Graphics {
    pub fn new(
        state: &egui_wgpu::RenderState,
        specs: Vec<Pipeline>,
        budget: u64,
        samples: u32,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            state.adapter.get_info().backend == wgpu::Backend::Vulkan
                && state.adapter.get_info().device_type != wgpu::DeviceType::Cpu,
            "native rendering requires hardware Vulkan"
        );
        anyhow::ensure!(
            budget > 0 && matches!(samples, 1 | 2 | 4 | 8) && specs.len() <= 256,
            "invalid graphics budget/pipeline count"
        );
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Depth32Float,
        ] {
            anyhow::ensure!(
                state
                    .adapter
                    .get_texture_format_features(format)
                    .flags
                    .sample_count_supported(samples),
                "unsupported host attachment sample count"
            );
        }
        let device = state.device.clone();
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "android"))]
        // Trusted host inspection only. No raw object is returned or retained;
        // the patched backend reports a bit it actually enabled at creation.
        let precise_occlusion = unsafe {
            device.as_hal::<wgpu::hal::api::Vulkan, _, _>(|device| {
                device.is_some_and(|d| d.gamepkg_precise_occlusion())
            })
        };
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "android")))]
        let precise_occlusion = false;
        let queue = state.queue.clone();
        let mut programs = Vec::new();
        let mut templates = Vec::new();
        let pipeline_hashes = specs
            .iter()
            .map(|spec| spec.identity_with_samples(samples))
            .collect();
        for spec in specs {
            spec.validate_layout()?;
            anyhow::ensure!(
                state
                    .adapter
                    .get_texture_format_features(spec.raster.depth_format)
                    .flags
                    .sample_count_supported(samples),
                "unsupported verified depth sample count"
            );
            anyhow::ensure!(
                spec.raster.depth_format != wgpu::TextureFormat::Depth32FloatStencil8
                    || device
                        .features()
                        .contains(wgpu::Features::DEPTH32FLOAT_STENCIL8),
                "host must request float32 stencil support at device creation"
            );
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut entries = vec![wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(spec.uniform_bytes),
                },
                count: None,
            }];
            for (slot, dimension) in spec.textures.iter().enumerate() {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 1 + slot as u32 * 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: *dimension,
                        multisampled: false,
                    },
                    count: None,
                });
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding: 2 + slot as u32 * 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                });
            }
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("GamePKG native bindings"),
                entries: &entries,
            });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("GamePKG native layout"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
            let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("GamePKG verified native vertex"),
                source: wgpu::ShaderSource::Wgsl(spec.vertex.as_str().into()),
            });
            let fragment = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("GamePKG verified native fragment"),
                source: wgpu::ShaderSource::Wgsl(spec.fragment.as_str().into()),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("GamePKG native pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex,
                    entry_point: Some(&spec.vertex_entry),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: spec.stride,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &spec.attributes,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment,
                    entry_point: Some(&spec.fragment_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: spec.blend,
                        write_mask: spec.raster.color_writes,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: spec.cull,
                    topology: spec.raster.topology,
                    front_face: spec.raster.front_face,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: spec.raster.depth_format,
                    depth_write_enabled: spec.depth_write,
                    depth_compare: spec.depth_compare,
                    stencil: spec.raster.stencil.clone(),
                    bias: spec.raster.bias,
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                multiview: None,
                cache: None,
            });
            if let Some(error) = pollster::block_on(device.pop_error_scope()) {
                anyhow::bail!("invalid verified native pipeline: {error}");
            }
            let program = Program {
                pipeline,
                layout,
                stride: spec.stride,
                uniform: spec.uniform_bytes,
                textures: spec.textures.clone(),
                topology: spec.raster.topology,
                depth_format: spec.raster.depth_format,
                uniform_policy: spec.uniform_policy.clone(),
            };
            templates.push(std::sync::Arc::new(variants::Template {
                vertex,
                fragment,
                layout: pipeline_layout,
                vertex_entry: spec.vertex_entry.clone(),
                fragment_entry: spec.fragment_entry.clone(),
                attributes: spec.attributes.clone(),
                initial: program.clone(),
                state: variants::Configuration::from(&spec),
                identity: spec.identity(),
            }));
            programs.push(program);
        }
        Ok(Self {
            device,
            queue,
            programs,
            buffers: BTreeMap::new(),
            textures: BTreeMap::new(),
            used: 0,
            budget,
            samples,
            samplers: BTreeMap::new(),
            targets: BTreeMap::new(),
            queries: BTreeMap::new(),
            depths: BTreeMap::new(),
            sampler_ids: BTreeMap::new(),
            readbacks: BTreeMap::new(),
            anisotropy: state
                .adapter
                .get_downlevel_capabilities()
                .flags
                .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING),
            stencil_samples: state
                .adapter
                .get_texture_format_features(wgpu::TextureFormat::Depth24PlusStencil8)
                .flags
                .sample_count_supported(samples),
            stencil32_samples: state
                .device
                .features()
                .contains(wgpu::Features::DEPTH32FLOAT_STENCIL8)
                && state
                    .adapter
                    .get_texture_format_features(wgpu::TextureFormat::Depth32FloatStencil8)
                    .flags
                    .sample_count_supported(samples),
            retired: Vec::new(),
            presentations: Default::default(),
            submissions: Default::default(),
            pipeline_hashes,
            rectangle_pipelines: BTreeMap::new(),
            precise_occlusion,
            templates,
            variants: BTreeMap::new(),
            preparing: Default::default(),
            wake: std::sync::Arc::new(|| {}),
        })
    }
    pub fn pipeline_hashes(&self) -> &[String] {
        &self.pipeline_hashes
    }
    fn reserve(&self, n: u64) -> Result<()> {
        if self.buffers.len()
            + self.textures.len()
            + self.queries.len()
            + self.retired.len()
            + self.depths.len()
            + self.sampler_ids.len()
            + self.readbacks.len()
            + self.variants.len()
            >= MAX_GPU_HANDLES
            || n > self.budget.saturating_sub(self.used).saturating_sub(
                self.presentations
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
        {
            Err(Error::Busy)
        } else {
            Ok(())
        }
    }
    pub fn call(&mut self, operation: u32, request: &[u8], capacity: usize) -> Result<Vec<u8>> {
        self.reap();
        if (127..=128).contains(&operation) {
            return self.variant_call(operation, request, capacity);
        }
        if (106..=125).contains(&operation) || matches!(operation, 129 | 135) {
            return self.retained_call(operation, request, capacity);
        }
        let mut r = Reader(request);
        match operation {
            100 => {
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let kind = r.u32()?;
                let bytes = r.0;
                if bytes.is_empty()
                    || bytes.len() % 4 != 0
                    || bytes.len() as u64 > self.device.limits().max_buffer_size
                {
                    return Err(Error::Invalid);
                }
                let usage = match kind {
                    0 => wgpu::BufferUsages::VERTEX,
                    1 => wgpu::BufferUsages::INDEX,
                    2 if bytes.len() <= 65536 && bytes.len() % 16 == 0 => {
                        wgpu::BufferUsages::UNIFORM
                    }
                    _ => return Err(Error::Invalid),
                };
                let mut nan = if kind == 0 {
                    vec![0u8; bytes.len().div_ceil(32)]
                } else {
                    Vec::new()
                };
                let mut nan_count = 0;
                if kind == 0 {
                    for (word, data) in bytes.chunks_exact(4).enumerate() {
                        if f32::from_le_bytes(data.try_into().unwrap()).is_nan() {
                            nan[word / 8] |= 1 << (word % 8);
                            nan_count += 1;
                        }
                    }
                }
                let cost = bytes.len() as u64 * if kind != 0 { 2 } else { 1 } + nan.len() as u64;
                self.reserve(cost)?;
                let id = identifier()?;
                let gpu = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("GamePKG native buffer"),
                        contents: bytes,
                        usage: usage | wgpu::BufferUsages::COPY_DST,
                    });
                self.buffers.insert(
                    id,
                    Buffer {
                        gpu,
                        kind,
                        len: bytes.len() as u64,
                        shadow: if kind != 0 {
                            bytes.to_vec()
                        } else {
                            Vec::new()
                        },
                        cost,
                        validation: Default::default(),
                        nan,
                        nan_count,
                    },
                );
                self.used += cost;
                Ok(id.to_le_bytes().to_vec())
            }
            101 => {
                let id = r.u64()?;
                let offset = r.u64()?;
                let bytes = r.0;
                let buffer = self.buffers.get(&id).ok_or(Error::Denied)?;
                if offset % 4 != 0
                    || bytes.len() % 4 != 0
                    || offset
                        .checked_add(bytes.len() as u64)
                        .is_none_or(|end| end > buffer.len)
                {
                    return Err(Error::Invalid);
                }
                if bytes.is_empty() {
                    return Ok(Vec::new());
                }
                self.reserve(bytes.len() as u64)?;
                self.submission_room()?;
                let buffer = self.buffers.get_mut(&id).unwrap();
                if buffer.kind == 0 {
                    for (word, data) in bytes.chunks_exact(4).enumerate() {
                        let at = offset as usize / 4 + word;
                        let bit = 1u8 << (at % 8);
                        let was = buffer.nan[at / 8] & bit != 0;
                        let now = f32::from_le_bytes(data.try_into().unwrap()).is_nan();
                        buffer.nan_count = buffer.nan_count - u64::from(was) + u64::from(now);
                        buffer.nan[at / 8] =
                            (buffer.nan[at / 8] & !bit) | (u8::from(now) << (at % 8));
                    }
                }
                if buffer.kind != 0 {
                    buffer.shadow[offset as usize..offset as usize + bytes.len()]
                        .copy_from_slice(bytes);
                    buffer.validation.get_mut().clear();
                }
                self.queue.write_buffer(&buffer.gpu, offset, bytes);
                self.submit(self.device.create_command_encoder(&Default::default()));
                self.used += bytes.len() as u64;
                self.retire(bytes.len() as u64);
                Ok(Vec::new())
            }
            102 => {
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let width = r.u32()?;
                let height = r.u32()?;
                let layers = r.u32()?;
                let mips = r.u32()?;
                let format = if r.0.is_empty() { 0 } else { r.u32()? };
                r.end()?;
                if format > 1 {
                    return Err(Error::Invalid);
                }
                let limit = self.device.limits().max_texture_dimension_2d.min(8192);
                if width == 0
                    || height == 0
                    || width > limit
                    || height > limit
                    || !matches!(layers, 1 | 6)
                    || (layers == 6 && width != height)
                    || mips == 0
                    || mips > 32 - width.max(height).leading_zeros()
                {
                    return Err(Error::Invalid);
                }
                let bytes = (0..mips)
                    .map(|m| {
                        u64::from((width >> m).max(1))
                            * u64::from((height >> m).max(1))
                            * u64::from(layers)
                            * 4
                    })
                    .sum();
                self.reserve(bytes)?;
                let id = identifier()?;
                let gpu = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("GamePKG native texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: layers,
                    },
                    mip_level_count: mips,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: if format == 0 {
                        wgpu::TextureFormat::Rgba8Unorm
                    } else {
                        wgpu::TextureFormat::Rgba8UnormSrgb
                    },
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                self.textures.insert(
                    id,
                    Texture {
                        gpu,
                        width,
                        height,
                        layers,
                        mips,
                        dimension: if layers == 6 {
                            wgpu::TextureViewDimension::Cube
                        } else {
                            wgpu::TextureViewDimension::D2
                        },
                        bytes,
                    },
                );
                self.used += bytes;
                Ok(id.to_le_bytes().to_vec())
            }
            103 => {
                let id = r.u64()?;
                if self.targets.contains_key(&id) {
                    return Err(Error::Denied);
                }
                let mip = r.u32()?;
                let layer = r.u32()?;
                let x = r.u32()?;
                let y = r.u32()?;
                let width = r.u32()?;
                let height = r.u32()?;
                let t = self.textures.get(&id).ok_or(Error::Denied)?;
                if mip >= t.mips
                    || layer >= t.layers
                    || width == 0
                    || height == 0
                    || x.checked_add(width)
                        .is_none_or(|end| end > (t.width >> mip).max(1))
                    || y.checked_add(height)
                        .is_none_or(|end| end > (t.height >> mip).max(1))
                    || u64::from(width) * u64::from(height) * 4 != r.0.len() as u64
                {
                    return Err(Error::Invalid);
                }
                self.reserve(r.0.len() as u64)?;
                self.submission_room()?;
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &t.gpu,
                        mip_level: mip,
                        origin: wgpu::Origin3d { x, y, z: layer },
                        aspect: wgpu::TextureAspect::All,
                    },
                    r.0,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
                self.submit(self.device.create_command_encoder(&Default::default()));
                self.used += r.0.len() as u64;
                self.retire(r.0.len() as u64);
                Ok(Vec::new())
            }
            104 => {
                let id = r.u64()?;
                r.end()?;
                if let Some(b) = self.buffers.remove(&id) {
                    self.retire(b.cost);
                } else if let Some(t) = self.textures.remove(&id) {
                    self.targets.remove(&id);
                    self.retire(t.bytes);
                } else if self.queries.remove(&id).is_some() {
                    self.retire(512);
                } else if let Some(readback) = self.readbacks.remove(&id) {
                    self.retire(readback.cost);
                } else if self.depths.remove(&id).is_some() {
                } else if self.sampler_ids.remove(&id).is_some() {
                    self.retire(256);
                } else if self.variants.remove(&id).is_some() {
                } else {
                    return Err(Error::Denied);
                }
                Ok(Vec::new())
            }
            _ => Err(Error::Unsupported),
        }
    }
    pub fn render(&mut self, request: &[u8]) -> Result<Presented> {
        self.render_inner(request, None, 3, None, 0)
    }
    fn render_inner(
        &mut self,
        request: &[u8],
        target: Option<u64>,
        loads: u32,
        query: Option<u64>,
        extended: u8,
    ) -> Result<Presented> {
        self.reap();
        self.submission_room()?;
        let mut r = Reader(request);
        let width = r.u32()?;
        let height = r.u32()?;
        let clear = r.u32()?;
        let depth = f32::from_bits(r.u32()?);
        let count = r.u32()?;
        let limit = self.device.limits().max_texture_dimension_2d.min(8192);
        if width == 0
            || height == 0
            || width > limit
            || height > limit
            || !depth.is_finite()
            || !(0.0..=1.0).contains(&depth)
            || count > 4096
        {
            return Err(Error::Invalid);
        }
        let canvas = u64::from(width) * u64::from(height) * (4 + 8 * u64::from(self.samples));
        if target.is_none() {
            self.reserve(canvas)?;
        }
        struct Draw<'a> {
            program: &'a Program,
            vertex: &'a Buffer,
            index: &'a Buffer,
            bind: wgpu::BindGroup,
            start: u32,
            count: u32,
            base: i32,
            scissor: [u32; 4],
            blend: wgpu::Color,
            stencil: u32,
            viewport: [f32; 6],
        }
        let mut draws = Vec::new();
        let mut references = 0u64;
        for _ in 0..count {
            let pipeline = r.u32()?;
            let program = if extended != 0 && pipeline == u32::MAX {
                let id = r.u64()?;
                let variant = self.variants.get(&id).ok_or(Error::Denied)?;
                if variant.failed {
                    return Err(Error::Failed);
                }
                variant.program.as_ref().ok_or(Error::Busy)?
            } else {
                self.programs.get(pipeline as usize).ok_or(Error::Invalid)?
            };
            let mut components = [0f64; 4];
            let stencil = if extended != 0 {
                for v in &mut components {
                    let f = f32::from_bits(r.u32()?);
                    if !f.is_finite() || !(0.0..=1.0).contains(&f) {
                        return Err(Error::Invalid);
                    }
                    *v = f64::from(f);
                }
                let value = r.u32()?;
                if value > 255 {
                    return Err(Error::Invalid);
                }
                value
            } else {
                0
            };
            let mut viewport = [0.0, 0.0, width as f32, height as f32, 0.0, 1.0];
            if extended & 2 != 0 {
                for value in &mut viewport {
                    *value = f32::from_bits(r.u32()?);
                }
                if viewport.iter().any(|v| !v.is_finite())
                    || viewport[0] < 0.0
                    || viewport[1] < 0.0
                    || viewport[2] <= 0.0
                    || viewport[3] <= 0.0
                    || viewport[0] + viewport[2] > width as f32
                    || viewport[1] + viewport[3] > height as f32
                    || !(0.0..=1.0).contains(&viewport[4])
                    || !(0.0..=1.0).contains(&viewport[5])
                {
                    return Err(Error::Invalid);
                }
            }
            let depth_format = target
                .and_then(|id| self.targets.get(&id))
                .map(|t| t.depth.format)
                .unwrap_or(wgpu::TextureFormat::Depth32Float);
            if program.depth_format != depth_format {
                return Err(Error::Invalid);
            }
            let vertex = self.buffers.get(&r.u64()?).ok_or(Error::Denied)?;
            let index = self.buffers.get(&r.u64()?).ok_or(Error::Denied)?;
            let uniform = self.buffers.get(&r.u64()?).ok_or(Error::Denied)?;
            let start = r.u32()?;
            let count = r.u32()?;
            references = references
                .checked_add(u64::from(count))
                .ok_or(Error::Invalid)?;
            if references > 1_048_576 {
                return Err(Error::Busy);
            }
            let base = r.u32()? as i32;
            let scissor = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
            let slots = r.u32()?;
            if vertex.kind != 0
                || index.kind != 1
                || uniform.kind != 2
                || uniform.len < program.uniform
                || slots as usize != program.textures.len()
                || count == 0
                || count
                    % match program.topology {
                        wgpu::PrimitiveTopology::PointList => 1,
                        wgpu::PrimitiveTopology::LineList => 2,
                        _ => 3,
                    }
                    != 0
                || start
                    .checked_add(count)
                    .is_none_or(|end| u64::from(end) * 4 > index.len)
                || scissor[2] == 0
                || scissor[3] == 0
                || scissor[0]
                    .checked_add(scissor[2])
                    .is_none_or(|end| end > width)
                || scissor[1]
                    .checked_add(scissor[3])
                    .is_none_or(|end| end > height)
            {
                return Err(Error::Invalid);
            }
            if let UniformPolicy::Nv2aV1 { cube } = program.uniform_policy {
                if vertex.nan_count != 0 {
                    return Err(Error::Invalid);
                }
                let key = cube
                    .iter()
                    .enumerate()
                    .fold(0u8, |n, (i, c)| n | (u8::from(*c) << i));
                let cached = uniform.validation.borrow().get(&key).copied();
                let result = cached.unwrap_or_else(|| {
                    crate::shaders::nv2a::validate(
                        &uniform.shadow[..program.uniform as usize],
                        cube,
                    )
                    .map_err(|e| {
                        if e.unsupported {
                            Error::Unsupported
                        } else {
                            Error::Invalid
                        }
                    })
                });
                if cached.is_none() {
                    uniform.validation.borrow_mut().insert(key, result);
                }
                result?;
            }
            for value in index.shadow
                [start as usize * 4..(u64::from(start) + u64::from(count)) as usize * 4]
                .chunks_exact(4)
            {
                let at = i64::from(u32::from_le_bytes(value.try_into().unwrap())) + i64::from(base);
                if at < 0 || at as u64 >= vertex.len / program.stride {
                    return Err(Error::Invalid);
                }
            }
            let mut views = Vec::new();
            let mut samplers = Vec::new();
            for expected in &program.textures {
                let texture_id = r.u64()?;
                if Some(texture_id) == target {
                    return Err(Error::Invalid);
                }
                let texture = self.textures.get(&texture_id).ok_or(Error::Denied)?;
                let filter = r.u32()?;
                if filter == 0x8000_0000 {
                    let id = r.u64()?;
                    if texture.dimension != *expected {
                        return Err(Error::Invalid);
                    }
                    views.push(texture.gpu.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(*expected),
                        ..Default::default()
                    }));
                    samplers.push(self.sampler_ids.get(&id).ok_or(Error::Denied)?.clone());
                    continue;
                }
                if texture.dimension != *expected
                    || filter > 127
                    || [1u32, 3, 5]
                        .iter()
                        .any(|shift| (filter >> shift) & 3u32 == 3)
                {
                    return Err(Error::Invalid);
                }
                views.push(texture.gpu.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(*expected),
                    ..Default::default()
                }));
                let sampler = self.samplers.entry(filter).or_insert_with(|| {
                    let address = |shift: u32| match (filter >> shift) & 3u32 {
                        1 => wgpu::AddressMode::Repeat,
                        2 => wgpu::AddressMode::MirrorRepeat,
                        _ => wgpu::AddressMode::ClampToEdge,
                    };
                    let mode = if filter & 1 != 0 {
                        wgpu::FilterMode::Linear
                    } else {
                        wgpu::FilterMode::Nearest
                    };
                    self.device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("GamePKG bounded native sampler"),
                        address_mode_u: address(1),
                        address_mode_v: address(3),
                        address_mode_w: address(5),
                        mag_filter: mode,
                        min_filter: mode,
                        mipmap_filter: mode,
                        ..Default::default()
                    })
                });
                samplers.push(sampler.clone());
            }
            let mut bindings = vec![wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform.gpu,
                    offset: 0,
                    size: std::num::NonZeroU64::new(program.uniform),
                }),
            }];
            for (slot, view) in views.iter().enumerate() {
                bindings.push(wgpu::BindGroupEntry {
                    binding: 1 + slot as u32 * 2,
                    resource: wgpu::BindingResource::TextureView(view),
                });
                bindings.push(wgpu::BindGroupEntry {
                    binding: 2 + slot as u32 * 2,
                    resource: wgpu::BindingResource::Sampler(&samplers[slot]),
                });
            }
            let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("GamePKG native draw bindings"),
                layout: &program.layout,
                entries: &bindings,
            });
            draws.push(Draw {
                program,
                vertex,
                index,
                bind,
                start,
                count,
                base,
                scissor,
                viewport,
                blend: wgpu::Color {
                    r: components[0],
                    g: components[1],
                    b: components[2],
                    a: components[3],
                },
                stencil,
            });
        }
        r.end()?;
        let make = |format, samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("GamePKG native canvas"),
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
        let color = if let Some(id) = target {
            self.textures.get(&id).ok_or(Error::Denied)?.gpu.clone()
        } else {
            make(
                wgpu::TextureFormat::Rgba8Unorm,
                1,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
            )
        };
        let view = color.create_view(&Default::default());
        let multisample = if let Some(id) = target {
            self.targets
                .get(&id)
                .ok_or(Error::Denied)?
                .multisample
                .clone()
        } else {
            (self.samples > 1).then(|| {
                make(
                    wgpu::TextureFormat::Rgba8Unorm,
                    self.samples,
                    wgpu::TextureUsages::RENDER_ATTACHMENT,
                )
            })
        };
        let msview = multisample
            .as_ref()
            .map(|t| t.create_view(&Default::default()));
        let depth_texture = if let Some(id) = target {
            self.targets
                .get(&id)
                .ok_or(Error::Denied)?
                .depth
                .gpu
                .clone()
        } else {
            make(
                wgpu::TextureFormat::Depth32Float,
                self.samples,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            )
        };
        let depth_view = depth_texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let visibility = query
            .map(|id| self.queries.get(&id).ok_or(Error::Denied))
            .transpose()?;
        if visibility.is_some_and(|q| q.pending.is_some() || q.result.is_some()) {
            return Err(Error::Busy);
        }
        let c = clear.to_be_bytes();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("GamePKG native indexed frame"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: msview.as_ref().unwrap_or(&view),
                    resolve_target: msview.as_ref().map(|_| &view),
                    ops: wgpu::Operations {
                        load: if loads & 1 == 0 {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: f64::from(c[0]) / 255.0,
                                g: f64::from(c[1]) / 255.0,
                                b: f64::from(c[2]) / 255.0,
                                a: f64::from(c[3]) / 255.0,
                            })
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if loads & 2 == 0 {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(depth)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: target
                        .and_then(|id| self.targets.get(&id))
                        .filter(|t| t.depth.format.has_stencil_aspect())
                        .map(|_| wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                }),
                timestamp_writes: None,
                occlusion_query_set: visibility.map(|q| &q.set),
            });
            if visibility.is_some() {
                pass.begin_occlusion_query(0);
            }
            for d in &draws {
                pass.set_pipeline(&d.program.pipeline);
                pass.set_blend_constant(d.blend);
                pass.set_stencil_reference(d.stencil);
                pass.set_bind_group(0, &d.bind, &[]);
                pass.set_vertex_buffer(0, d.vertex.gpu.slice(..));
                pass.set_index_buffer(d.index.gpu.slice(..), wgpu::IndexFormat::Uint32);
                pass.set_scissor_rect(d.scissor[0], d.scissor[1], d.scissor[2], d.scissor[3]);
                pass.set_viewport(
                    d.viewport[0],
                    d.viewport[1],
                    d.viewport[2],
                    d.viewport[3],
                    d.viewport[4],
                    d.viewport[5],
                );
                pass.draw_indexed(d.start..d.start + d.count, d.base, 0..1);
            }
            if visibility.is_some() {
                pass.end_occlusion_query();
            }
        }
        if let Some(q) = visibility {
            encoder.resolve_query_set(&q.set, 0..1, &q.resolve, 0);
            encoder.copy_buffer_to_buffer(&q.resolve, 0, &q.read, 0, 8);
        }
        self.submit(encoder);
        if let Some(id) = query {
            let q = self.queries.get_mut(&id).unwrap();
            let (send, receive) = std::sync::mpsc::sync_channel(1);
            q.read
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = send.send(result.is_ok());
                });
            q.pending = Some(receive);
        }
        // egui decodes sRGB textures before its own framebuffer conversion.
        // Use a compatible sRGB view so compositing preserves the native
        // framebuffer's stored RGB bytes instead of applying gamma twice.
        let presentation = color.create_view(&wgpu::TextureViewDescriptor {
            format: Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            ..Default::default()
        });
        Ok(Presented {
            texture: color,
            view: presentation,
            width,
            height,
            _lease: if target.is_none() {
                let color_cost = u64::from(width) * u64::from(height) * 4;
                self.used += canvas - color_cost;
                self.retire(canvas - color_cost);
                Some(self.presentation_lease(color_cost))
            } else {
                None
            },
        })
    }
}
