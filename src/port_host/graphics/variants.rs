//! Bounded asynchronous states derived only from already verified templates.
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, TryRecvError},
    Arc,
};
const VARIANT_BYTES: u64 = 1_048_576;
struct Preparing(Arc<AtomicBool>);
impl Drop for Preparing {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub(super) struct Template {
    pub vertex: wgpu::ShaderModule,
    pub fragment: wgpu::ShaderModule,
    pub layout: wgpu::PipelineLayout,
    pub vertex_entry: String,
    pub fragment_entry: String,
    pub attributes: Vec<wgpu::VertexAttribute>,
    pub initial: Program,
    pub state: Configuration,
    pub identity: String,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Configuration {
    blend: Option<wgpu::BlendState>,
    cull: Option<wgpu::Face>,
    depth_write: bool,
    depth_compare: wgpu::CompareFunction,
    raster: RasterState,
}
impl From<&Pipeline> for Configuration {
    fn from(p: &Pipeline) -> Self {
        Self {
            blend: p.blend,
            cull: p.cull,
            depth_write: p.depth_write,
            depth_compare: p.depth_compare,
            raster: p.raster.clone(),
        }
    }
}
struct Charge {
    bytes: u64,
    live: Arc<std::sync::atomic::AtomicU64>,
    queue: wgpu::Queue,
}
impl Drop for Charge {
    fn drop(&mut self) {
        let live = self.live.clone();
        let bytes = self.bytes;
        self.queue.on_submitted_work_done(move || {
            live.fetch_sub(bytes, Ordering::Relaxed);
        });
    }
}
pub(super) struct Variant {
    receive: Option<Receiver<std::result::Result<Program, ()>>>,
    pub program: Option<Program>,
    pub failed: bool,
    hash: [u8; 32],
    _charge: Arc<Charge>,
}
fn compare(v: u32) -> Result<wgpu::CompareFunction> {
    Ok(match v {
        0 => wgpu::CompareFunction::Never,
        1 => wgpu::CompareFunction::Less,
        2 => wgpu::CompareFunction::Equal,
        3 => wgpu::CompareFunction::LessEqual,
        4 => wgpu::CompareFunction::Greater,
        5 => wgpu::CompareFunction::NotEqual,
        6 => wgpu::CompareFunction::GreaterEqual,
        7 => wgpu::CompareFunction::Always,
        _ => return Err(Error::Invalid),
    })
}
fn stencil(v: u32) -> Result<wgpu::StencilOperation> {
    Ok(match v {
        0 => wgpu::StencilOperation::Keep,
        1 => wgpu::StencilOperation::Zero,
        2 => wgpu::StencilOperation::Replace,
        3 => wgpu::StencilOperation::IncrementClamp,
        4 => wgpu::StencilOperation::DecrementClamp,
        5 => wgpu::StencilOperation::Invert,
        6 => wgpu::StencilOperation::IncrementWrap,
        7 => wgpu::StencilOperation::DecrementWrap,
        _ => return Err(Error::Invalid),
    })
}
fn factor(v: u32) -> Result<wgpu::BlendFactor> {
    Ok(match v {
        0 => wgpu::BlendFactor::Zero,
        1 => wgpu::BlendFactor::One,
        2 => wgpu::BlendFactor::Src,
        3 => wgpu::BlendFactor::OneMinusSrc,
        4 => wgpu::BlendFactor::SrcAlpha,
        5 => wgpu::BlendFactor::OneMinusSrcAlpha,
        6 => wgpu::BlendFactor::Dst,
        7 => wgpu::BlendFactor::OneMinusDst,
        8 => wgpu::BlendFactor::DstAlpha,
        9 => wgpu::BlendFactor::OneMinusDstAlpha,
        10 => wgpu::BlendFactor::SrcAlphaSaturated,
        11 => wgpu::BlendFactor::Constant,
        12 => wgpu::BlendFactor::OneMinusConstant,
        _ => return Err(Error::Invalid),
    })
}
fn blend(r: &mut Reader<'_>) -> Result<wgpu::BlendComponent> {
    let src = r.u32()?;
    let dst = r.u32()?;
    let operation = match r.u32()? {
        0 => wgpu::BlendOperation::Add,
        1 => wgpu::BlendOperation::Subtract,
        2 => wgpu::BlendOperation::ReverseSubtract,
        3 => wgpu::BlendOperation::Min,
        4 => wgpu::BlendOperation::Max,
        _ => return Err(Error::Invalid),
    };
    if matches!(
        operation,
        wgpu::BlendOperation::Min | wgpu::BlendOperation::Max
    ) && (src != 1 || dst != 1)
    {
        return Err(Error::Invalid);
    }
    Ok(wgpu::BlendComponent {
        src_factor: factor(src)?,
        dst_factor: factor(dst)?,
        operation,
    })
}
fn boolean(v: u32) -> Result<bool> {
    match v {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::Invalid),
    }
}
fn face(r: &mut Reader<'_>) -> Result<wgpu::StencilFaceState> {
    Ok(wgpu::StencilFaceState {
        compare: compare(r.u32()?)?,
        fail_op: stencil(r.u32()?)?,
        depth_fail_op: stencil(r.u32()?)?,
        pass_op: stencil(r.u32()?)?,
    })
}
fn configuration(r: &mut Reader<'_>) -> Result<Configuration> {
    let topology = match r.u32()? {
        0 => wgpu::PrimitiveTopology::PointList,
        1 => wgpu::PrimitiveTopology::LineList,
        2 => wgpu::PrimitiveTopology::TriangleList,
        _ => return Err(Error::Invalid),
    };
    let cull = match r.u32()? {
        0 => None,
        1 => Some(wgpu::Face::Front),
        2 => Some(wgpu::Face::Back),
        _ => return Err(Error::Invalid),
    };
    let front_face = match r.u32()? {
        0 => wgpu::FrontFace::Ccw,
        1 => wgpu::FrontFace::Cw,
        _ => return Err(Error::Invalid),
    };
    let color_writes = wgpu::ColorWrites::from_bits(r.u32()?).ok_or(Error::Invalid)?;
    let depth_format = match r.u32()? {
        0 => wgpu::TextureFormat::Depth32Float,
        1 => wgpu::TextureFormat::Depth24PlusStencil8,
        2 => wgpu::TextureFormat::Depth32FloatStencil8,
        _ => return Err(Error::Invalid),
    };
    let depth_write = boolean(r.u32()?)?;
    let depth_compare = compare(r.u32()?)?;
    let constant = r.u32()? as i32;
    let mut slope_scale = f32::from_bits(r.u32()?);
    let mut clamp = f32::from_bits(r.u32()?);
    if slope_scale == 0. {
        slope_scale = 0.;
    }
    if clamp == 0. {
        clamp = 0.;
    }
    let read_mask = r.u32()?;
    let write_mask = r.u32()?;
    let front = face(r)?;
    let back = face(r)?;
    let blending = boolean(r.u32()?)?;
    let color = blend(r)?;
    let alpha = blend(r)?;
    // Disabled blending has one canonical encoding: six zero words.
    if !blending
        && (color
            != wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            }
            || alpha != color)
    {
        return Err(Error::Invalid);
    }
    let raster = RasterState {
        topology,
        front_face,
        color_writes,
        depth_format,
        bias: wgpu::DepthBiasState {
            constant,
            slope_scale,
            clamp,
        },
        stencil: wgpu::StencilState {
            front,
            back,
            read_mask,
            write_mask,
        },
    };
    if !raster.validate() {
        return Err(Error::Invalid);
    }
    r.end()?;
    Ok(Configuration {
        blend: blending.then_some(wgpu::BlendState { color, alpha }),
        cull,
        depth_write,
        depth_compare,
        raster,
    })
}
fn prepare(
    device: &wgpu::Device,
    t: &Template,
    c: &Configuration,
    samples: u32,
) -> std::result::Result<Program, ()> {
    if c == &t.state {
        return Ok(t.initial.clone());
    }
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("GamePKG verified-template state variant"),
        layout: Some(&t.layout),
        vertex: wgpu::VertexState {
            module: &t.vertex,
            entry_point: Some(&t.vertex_entry),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: t.initial.stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &t.attributes,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &t.fragment,
            entry_point: Some(&t.fragment_entry),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: c.blend,
                write_mask: c.raster.color_writes,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: c.raster.topology,
            front_face: c.raster.front_face,
            cull_mode: c.cull,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: c.raster.depth_format,
            depth_write_enabled: c.depth_write,
            depth_compare: c.depth_compare,
            stencil: c.raster.stencil.clone(),
            bias: c.raster.bias,
        }),
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        multiview: None,
        cache: None,
    });
    if pollster::block_on(device.pop_error_scope()).is_some() {
        return Err(());
    }
    Ok(Program {
        pipeline,
        topology: c.raster.topology,
        depth_format: c.raster.depth_format,
        ..t.initial.clone()
    })
}
impl Graphics {
    /// Host-only event-loop wake. Must not synchronously reenter Graphics.
    pub fn set_wake(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        self.wake = Arc::new(wake);
    }
    pub(super) fn poll_variants(&mut self) {
        for variant in self.variants.values_mut() {
            let Some(receive) = &variant.receive else {
                continue;
            };
            match receive.try_recv() {
                Ok(Ok(program)) => {
                    variant.program = Some(program);
                    variant.receive = None;
                }
                Ok(Err(())) | Err(TryRecvError::Disconnected) => {
                    variant.failed = true;
                    variant.receive = None;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }
    pub(super) fn variant_call(
        &mut self,
        op: u32,
        bytes: &[u8],
        capacity: usize,
    ) -> Result<Vec<u8>> {
        let mut r = Reader(bytes);
        if op == 128 {
            let id = r.u64()?;
            r.end()?;
            if capacity < 32 {
                return Err(Error::Invalid);
            }
            let v = self.variants.get(&id).ok_or(Error::Denied)?;
            if v.failed {
                return Err(Error::Failed);
            }
            if v.program.is_none() {
                return Err(Error::Busy);
            }
            return Ok(v.hash.to_vec());
        }
        if capacity < 8 {
            return Err(Error::Invalid);
        }
        let base = r.u32()?;
        let state = configuration(&mut r)?;
        let template = self
            .templates
            .get(base as usize)
            .ok_or(Error::Invalid)?
            .clone();
        if state.raster.depth_format == wgpu::TextureFormat::Depth24PlusStencil8
            && !self.stencil_samples
            || state.raster.depth_format == wgpu::TextureFormat::Depth32FloatStencil8
                && !self.stencil32_samples
        {
            return Err(Error::Unsupported);
        }
        let hash: [u8; 32] = hex::decode(crate::verify::digest(
            format!(
                "GamePKG variant v1|{}|{:?}|samples={}",
                template.identity, state, self.samples
            )
            .as_bytes(),
        ))
        .unwrap()
        .try_into()
        .unwrap();
        if let Some((&id, _)) = self.variants.iter().find(|(_, v)| v.hash == hash) {
            return Ok(id.to_le_bytes().to_vec());
        }
        if self.variants.len() >= 256 {
            return Err(Error::Busy);
        }
        self.reserve(VARIANT_BYTES)?;
        if self
            .preparing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return Err(Error::Busy);
        }
        let id = match identifier() {
            Ok(id) => id,
            Err(e) => {
                self.preparing.store(false, Ordering::Release);
                return Err(e);
            }
        };
        self.presentations
            .fetch_add(VARIANT_BYTES, Ordering::Relaxed);
        let charge = Arc::new(Charge {
            bytes: VARIANT_BYTES,
            live: self.presentations.clone(),
            queue: self.queue.clone(),
        });
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        self.variants.insert(
            id,
            Variant {
                receive: Some(receive),
                program: None,
                failed: false,
                hash,
                _charge: charge.clone(),
            },
        );
        let device = self.device.clone();
        let samples = self.samples;
        let preparing = self.preparing.clone();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("gamepkg-pipeline".into())
            .spawn(move || {
                let guard = Preparing(preparing);
                let result = prepare(&device, &template, &state, samples);
                let _ = send.send(result);
                drop(guard);
                wake();
                drop(charge);
            });
        if spawned.is_err() {
            self.variants.remove(&id);
            self.preparing.store(false, Ordering::Release);
            return Err(Error::Failed);
        }
        Ok(id.to_le_bytes().to_vec())
    }
}
