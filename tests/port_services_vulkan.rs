#![cfg(feature = "gui")]
use eframe::{egui, egui_wgpu, wgpu};
use egui::epaint::mutex::RwLock;
use gamepkg::port_host::{desktop::Desktop, Backend, Error, Session, WindowCommand};
use std::sync::Arc;

#[path = "../engines/native-services/client.rs"]
#[allow(dead_code)]
mod local_client;

#[test]
#[ignore = "requires a real Vulkan hardware device; paints offscreen without opening a window"]
fn shared_desktop_presents_textures_and_verified_shaders_on_hardware_vulkan() {
    let _exclusive = local_client::TEST_BINDING_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("hardware Vulkan adapter");
    let info = adapter.get_info();
    assert_ne!(
        info.device_type,
        wgpu::DeviceType::Cpu,
        "software Vulkan is not a hardware test"
    );
    eprintln!("Vulkan pixel checks on {}", info.name);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let state = egui_wgpu::RenderState {
        adapter,
        available_adapters: Vec::new(),
        device: device.clone(),
        queue: queue.clone(),
        target_format: format,
        renderer: Arc::new(RwLock::new(egui_wgpu::Renderer::new(
            &device, format, None, 1, false,
        ))),
    };
    let context = egui::Context::default();
    context.set_pixels_per_point(1.0);
    let source =
        "@fragment fn fragmentMain()->@location(0) vec4<f32>{return vec4<f32>(0.25,0.5,0.75,1.0);}"
            .to_owned();
    let mut desktop = Desktop::new(context.clone(), &state, &[source], None).unwrap();
    assert_eq!(desktop.audio(&[0, 0], 2), Err(Error::Unsupported));
    desktop.window(WindowCommand::Fullscreen(true)).unwrap();
    desktop.window(WindowCommand::CursorVisible(false)).unwrap();
    desktop.window(WindowCommand::CursorLocked(true)).unwrap();
    let permissions = toml::from_str::<gamepkg::manifest::Manifest>(include_str!(
        "../examples/minimal/manifest.toml"
    ))
    .unwrap()
    .permissions;
    let mut host = Session::new(permissions, desktop);
    let (connection, pending) =
        gamepkg::port_host::broker::channel_with_services(host.worker_services(), || {});
    local_client::connect_local(move |op, bytes, capacity| {
        connection
            .call(op, bytes, capacity)
            .map_err(|error| error as i32)
    })
    .unwrap();
    let game = std::thread::spawn(|| {
        let image = local_client::image_create(1, 1, &[10, 20, 30, 255]).unwrap();
        let frame = [
            local_client::ImageDraw {
                image,
                rect: [0.0, 0.0, 32.0, 32.0],
                linear: false,
                shader: None,
            },
            local_client::ImageDraw {
                image,
                rect: [32.0, 0.0, 32.0, 32.0],
                linear: false,
                shader: Some(0),
            },
        ];
        local_client::present(0x000000ff, &frame).unwrap();
        assert_eq!(local_client::present(0x000000ff, &frame), Err(-2));
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !game.is_finished() {
        pending.dispatch(&mut host, 8);
        if std::time::Instant::now() >= deadline {
            drop(pending);
            let _ = game.join();
            local_client::disconnect();
            panic!("in-process Vulkan request did not complete");
        }
        std::thread::yield_now();
    }
    local_client::disconnect();
    game.join().unwrap();
    // Two paints: unsolicited redraws retain the previous game image.
    for pass in 0..4 {
        if pass == 2 {
            host.backend()
                .install_graphics(Vec::new(), 16 * 1024 * 1024, 1)
                .unwrap();
            let native = [64u32, 64, 0x204060ff, 1f32.to_bits(), 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            host.call(105, &native, 0).unwrap();
        }
        let output = context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(64.0, 64.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| host.backend().paint(ui));
            },
        );
        if pass == 0 {
            let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
            assert!(commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::Fullscreen(true))));
        }
        let primitives = context.tessellate(output.shapes, output.pixels_per_point);
        let size = wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        };
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("port offscreen pixels"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("port pixel readback"),
            size: 64 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [64, 64],
            pixels_per_point: 1.0,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut renderer = state.renderer.write();
            for (id, delta) in &output.textures_delta.set {
                renderer.update_texture(&device, &queue, *id, delta);
            }
            let callbacks =
                renderer.update_buffers(&device, &queue, &mut encoder, &primitives, &screen);
            assert!(callbacks.is_empty());
            let view = target.create_view(&Default::default());
            let mut render = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("port offscreen render"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            renderer.render(&mut render, &primitives, &screen);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(64),
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (send, done) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::Maintain::Wait);
        done.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range();
        if pass >= 2 {
            assert_eq!(
                &pixels[8 * 256 + 8 * 4..8 * 256 + 8 * 4 + 4],
                &[32, 64, 96, 255],
                "native 3D compositor pass {pass}: no gamma shift"
            );
            continue;
        }
        assert_eq!(
            &pixels[8 * 256 + 8 * 4..8 * 256 + 8 * 4 + 4],
            &[10, 20, 30, 255],
            "texture pass {pass}"
        );
        let actual = &pixels[8 * 256 + 40 * 4..8 * 256 + 40 * 4 + 4];
        for (a, b) in actual.iter().zip([64u8, 128, 191, 255]) {
            assert!(a.abs_diff(b) <= 1, "shader pass {pass}: {actual:?}");
        }
        assert_eq!(
            &pixels[48 * 256 + 8 * 4..48 * 256 + 8 * 4 + 4],
            &[0, 0, 0, 255]
        );
    }
}

fn read_native_pixels(
    state: &egui_wgpu::RenderState,
    image: &gamepkg::port_host::graphics::Presented,
) -> Vec<u8> {
    assert_eq!([image.width, image.height], [64, 64]);
    let device = &state.device;
    let queue = &state.queue;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("native 3D pixel checks"),
        size: 64 * 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &image.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(64),
            },
        },
        wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (send, done) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    device.poll(wgpu::Maintain::Wait);
    done.recv().unwrap().unwrap();
    let pixels = buffer.slice(..).get_mapped_range().to_vec();
    pixels
}

#[test]
#[ignore = "requires a real Vulkan hardware device; exercises indexed 3D, depth, MSAA and cubemap mip sampling"]
fn native_indexed_geometry_depth_msaa_cube_mips_and_invalid_resources() {
    use gamepkg::port_host::graphics::{Graphics, Pipeline};
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .unwrap();
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    eprintln!("Native 3D pixel checks on {}", adapter.get_info().name);
    let descriptor = wgpu::DeviceDescriptor {
        required_features: wgpu::Features::DEPTH32FLOAT_STENCIL8
            | (adapter.features() & wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER),
        ..Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&descriptor, None)).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let state = egui_wgpu::RenderState {
        adapter,
        available_adapters: Vec::new(),
        device: device.clone(),
        queue: queue.clone(),
        target_format: format,
        renderer: Arc::new(RwLock::new(egui_wgpu::Renderer::new(
            &device, format, None, 1, false,
        ))),
    };
    let vertex="struct Out {@builtin(position) pos:vec4<f32>,@location(0) color:vec4<f32>}; @vertex fn vs(@location(0) pos:vec3<f32>,@location(1) color:vec4<f32>)->Out {var o:Out;o.pos=vec4<f32>(pos,1.0);o.color=color;return o;}";
    let color="@group(0) @binding(0) var<uniform> tint:vec4<f32>; @fragment fn fs(@location(0) color:vec4<f32>)->@location(0) vec4<f32>{return tint*color;}";
    let cube="@group(0) @binding(0) var<uniform> direction:vec4<f32>; @group(0) @binding(1) var image:texture_cube<f32>; @group(0) @binding(2) var sample:sampler; @fragment fn fs()->@location(0) vec4<f32>{return textureSampleLevel(image,sample,direction.xyz,1.0);}";
    let tiled="@group(0) @binding(1) var image:texture_2d<f32>; @group(0) @binding(2) var sample:sampler; @fragment fn fs()->@location(0) vec4<f32>{return textureSampleLevel(image,sample,vec2<f32>(1.25,0.25),0.0);}";
    let spec = |fragment: &str,
                textures: Vec<wgpu::TextureViewDimension>,
                depth_write,
                depth_compare| Pipeline {
        vertex: vertex.into(),
        vertex_entry: "vs".into(),
        fragment: fragment.into(),
        fragment_entry: "fs".into(),
        stride: 28,
        attributes: vec![
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 12,
                shader_location: 1,
            },
        ],
        uniform_bytes: 16,
        textures,
        blend: None,
        cull: None,
        depth_write,
        depth_compare,
        raster: Default::default(),
        uniform_policy: Default::default(),
    };
    let extra = |kind: u32| {
        let mut p = spec(color, vec![], false, wgpu::CompareFunction::Always);
        if kind <= 6 {
            p.raster.depth_format = wgpu::TextureFormat::Depth24PlusStencil8;
        }
        if kind == 4 || kind == 5 {
            let face = wgpu::StencilFaceState {
                compare: if kind == 4 {
                    wgpu::CompareFunction::Always
                } else {
                    wgpu::CompareFunction::Equal
                },
                fail_op: wgpu::StencilOperation::Keep,
                depth_fail_op: wgpu::StencilOperation::Keep,
                pass_op: if kind == 4 {
                    wgpu::StencilOperation::Replace
                } else {
                    wgpu::StencilOperation::Keep
                },
            };
            p.raster.stencil = wgpu::StencilState {
                front: face,
                back: face,
                read_mask: 255,
                write_mask: if kind == 4 { 255 } else { 0 },
            };
            if kind == 4 {
                p.raster.color_writes = wgpu::ColorWrites::empty();
            }
        }
        if kind == 6 {
            p.depth_write = true;
            p.depth_compare = wgpu::CompareFunction::Less;
        }
        if kind == 7 {
            p.raster.color_writes = wgpu::ColorWrites::RED;
        }
        if kind == 8 {
            p.blend = Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Constant,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            });
        }
        if kind == 9 || kind == 10 {
            p.cull = Some(wgpu::Face::Back);
            p.raster.front_face = if kind == 9 {
                wgpu::FrontFace::Cw
            } else {
                wgpu::FrontFace::Ccw
            };
        }
        if kind == 11 {
            p.raster.topology = wgpu::PrimitiveTopology::LineList;
        }
        if kind == 12 {
            p.raster.topology = wgpu::PrimitiveTopology::PointList;
        }
        if kind == 13 {
            p.raster.depth_format = wgpu::TextureFormat::Depth32FloatStencil8;
            p.depth_write = true;
            p.depth_compare = wgpu::CompareFunction::Less;
        }
        if kind == 14 {
            // A layout witness, NOT an NV2A interpreter or fidelity fixture.
            p.vertex =
                "@vertex fn vs(@location(0) p:vec4<f32>)->@builtin(position) vec4<f32>{return p;}"
                    .into();
            p.fragment =
                "@fragment fn fs()->@location(0) vec4<f32>{return vec4<f32>(1.,0.,1.,1.);}".into();
            p.uniform_bytes = gamepkg::shaders::nv2a::BYTES as u64;
            p.stride = 256;
            p.attributes = (0..16)
                .map(|i| wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: i as u64 * 16,
                    shader_location: i,
                })
                .collect();
            p.textures = vec![wgpu::TextureViewDimension::D2; 4];
            p.uniform_policy =
                gamepkg::port_host::graphics::UniformPolicy::Nv2aV1 { cube: [false; 4] };
        }
        p
    };
    let mut gpu = Graphics::new(
        &state,
        vec![
            spec(color, vec![], true, wgpu::CompareFunction::Less),
            spec(
                cube,
                vec![wgpu::TextureViewDimension::Cube],
                true,
                wgpu::CompareFunction::Less,
            ),
            spec(color, vec![], false, wgpu::CompareFunction::Always),
            spec(
                tiled,
                vec![wgpu::TextureViewDimension::D2],
                true,
                wgpu::CompareFunction::Less,
            ),
            extra(4),
            extra(5),
            extra(6),
            extra(7),
            extra(8),
            extra(9),
            extra(10),
            extra(11),
            extra(12),
            extra(13),
            extra(14),
        ],
        16 * 1024 * 1024,
        4,
    )
    .unwrap();
    // Logical handle capacity and byte admission are independent under MSAA.
    let mut many_handles = Graphics::new(&state, Vec::new(), 64 * 1024, 4).unwrap();
    let small_buffer = [1, 0, 0, 0, 0, 0, 0, 0];
    for _ in 0..4096 {
        assert_eq!(many_handles.call(100, &small_buffer, 8).unwrap().len(), 8);
    }
    assert_eq!(many_handles.call(100, &small_buffer, 8), Err(Error::Busy));
    drop(many_handles);
    assert_eq!(gpu.pipeline_hashes().len(), 15);
    assert!(gpu.pipeline_hashes().iter().all(|hash| hash.len() == 64));
    // A held displayed snapshot consumes its own budget even after GPU work
    // completes. Admission cannot repeatedly allocate uncharged presentations.
    let mut limited = Graphics::new(&state, Vec::new(), 64 * 64 * 40, 4).unwrap();
    let held_target = u64::from_le_bytes(
        limited
            .call(106, &[64u32.to_le_bytes(), 64u32.to_le_bytes()].concat(), 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let held = limited.present_target(&held_target.to_le_bytes()).unwrap();
    read_native_pixels(&state, &held);
    assert!(matches!(
        limited.present_target(&held_target.to_le_bytes()),
        Err(Error::Busy)
    ));
    drop(held);
    limited.present_target(&held_target.to_le_bytes()).unwrap();
    let mut queries = Vec::new();
    for _ in 0..64 {
        queries.push(u64::from_le_bytes(
            gpu.call(111, &[], 8).unwrap().try_into().unwrap(),
        ));
    }
    assert_eq!(gpu.call(111, &[], 8), Err(Error::Busy));
    for query in queries {
        gpu.call(104, &query.to_le_bytes(), 0).unwrap();
    }
    let create = |gpu: &mut Graphics, kind: u32, bytes: &[u8]| {
        u64::from_le_bytes(
            gpu.call(100, &[kind.to_le_bytes().as_slice(), bytes].concat(), 8)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    };
    let mut vertices = Vec::new();
    for (z, color) in [
        (0.8f32, [1f32, 0.0, 0.0, 1.0]),
        (0.2f32, [0f32, 1.0, 0.0, 1.0]),
    ] {
        for (x, y) in [(-1f32, -1f32), (1.0, -1.0), (0.0, 1.0)] {
            for f in [x, y, z, color[0], color[1], color[2], color[3]] {
                vertices.extend(f.to_le_bytes());
            }
        }
    }
    let vertex_id = create(&mut gpu, 0, &vertices);
    let index_id = create(
        &mut gpu,
        1,
        &(0..6u32).flat_map(u32::to_le_bytes).collect::<Vec<_>>(),
    );
    let uniform_id = create(
        &mut gpu,
        2,
        &[1f32; 4]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let frame = |program: u32, starts: &[u32], texture: Option<u64>| {
        let mut bytes = Vec::new();
        for n in [64u32, 64, 0x010203ff, 1f32.to_bits(), starts.len() as u32] {
            bytes.extend(n.to_le_bytes());
        }
        for start in starts {
            bytes.extend(program.to_le_bytes());
            for id in [vertex_id, index_id, uniform_id] {
                bytes.extend(id.to_le_bytes());
            }
            for n in [*start, 3, 0, 0, 0, 64, 64, u32::from(texture.is_some())] {
                bytes.extend(n.to_le_bytes());
            }
            if let Some(texture) = texture {
                bytes.extend(texture.to_le_bytes());
                bytes.extend(1u32.to_le_bytes());
            }
        }
        bytes
    };
    let image = gpu.render(&frame(0, &[3, 0], None)).unwrap();
    let pixels = read_native_pixels(&state, &image);
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[0, 255, 0, 255],
        "front geometry survives later back geometry"
    );
    assert_eq!(&pixels[..4], &[1, 2, 3, 255]);
    // Negative control: removing depth protection must change that exact pixel.
    let no_depth = gpu.render(&frame(2, &[3, 0], None)).unwrap();
    let pixels = read_native_pixels(&state, &no_depth);
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[255, 0, 0, 255]
    );
    let texture = u64::from_le_bytes(
        gpu.call(
            102,
            &[2u32, 2, 6, 2]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    let colors = [
        [255u8, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 0, 255],
        [0, 255, 255, 255],
        [255, 0, 255, 255],
    ];
    for face in 0..6u32 {
        for mip in 0..2u32 {
            let side = 2 >> mip;
            let mut upload = texture.to_le_bytes().to_vec();
            for n in [mip, face, 0, 0, side, side] {
                upload.extend(n.to_le_bytes());
            }
            for _ in 0..side * side {
                upload.extend(if mip == 0 {
                    [5u8, 10, 15, 255]
                } else {
                    colors[face as usize]
                });
            }
            gpu.call(103, &upload, 0).unwrap();
        }
    }
    for (face, direction) in [
        [1f32, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ]
    .into_iter()
    .enumerate()
    {
        let mut update = [uniform_id.to_le_bytes(), 0u64.to_le_bytes()].concat();
        for f in [direction[0], direction[1], direction[2], 1.0] {
            update.extend(f.to_le_bytes());
        }
        gpu.call(101, &update, 0).unwrap();
        let image = gpu.render(&frame(1, &[0], Some(texture))).unwrap();
        let pixels = read_native_pixels(&state, &image);
        assert_eq!(
            &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
            &colors[face],
            "cubemap face {face}, explicit mip 1"
        );
    }
    for srgb in [false, true] {
        let texture = u64::from_le_bytes(
            gpu.call(
                102,
                &[2u32, 1, 1, 1, u32::from(srgb)]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
                8,
            )
            .unwrap()
            .try_into()
            .unwrap(),
        );
        let mut upload = texture.to_le_bytes().to_vec();
        for n in [0u32, 0, 0, 0, 2, 1] {
            upload.extend(n.to_le_bytes());
        }
        upload.extend(if srgb {
            [128u8, 128, 128, 255, 128, 128, 128, 255]
        } else {
            [255, 0, 0, 255, 0, 255, 0, 255]
        });
        gpu.call(103, &upload, 0).unwrap();
        for (sampler, expected) in [
            (0u32, [0u8, 255, 0, 255]),
            (2, [255, 0, 0, 255]),
            (4, [0, 255, 0, 255]),
        ] {
            let mut request = frame(3, &[0], Some(texture));
            let len = request.len();
            request[len - 4..].copy_from_slice(&sampler.to_le_bytes());
            let image = gpu.render(&request).unwrap();
            let pixels = read_native_pixels(&state, &image);
            assert_eq!(
                &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
                &if srgb { [55u8, 55, 55, 255] } else { expected },
                "sRGB {srgb}, sampler {sampler}"
            );
            request[len - 4..].copy_from_slice(&6u32.to_le_bytes());
            assert!(matches!(gpu.render(&request), Err(Error::Invalid)));
        }
    }
    // Ordered retained multipass, real occlusion, partial clears, self-copy,
    // resolved MSAA sampling and immutable presentation on the same hardware.
    gpu.call(
        101,
        &[
            uniform_id.to_le_bytes().as_slice(),
            &0u64.to_le_bytes(),
            &[1f32; 4]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        ]
        .concat(),
        0,
    )
    .unwrap();
    let target = |gpu: &mut Graphics| {
        u64::from_le_bytes(
            gpu.call(106, &[64u32.to_le_bytes(), 64u32.to_le_bytes()].concat(), 8)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    };
    let a = target(&mut gpu);
    let b = target(&mut gpu);
    let pass = |id: u64,
                mask: u32,
                program: u32,
                starts: &[u32],
                texture: Option<u64>,
                query: Option<u64>| {
        let mut request = id.to_le_bytes().to_vec();
        if let Some(q) = query {
            request.extend(q.to_le_bytes());
        }
        request.extend(mask.to_le_bytes());
        request.extend(&frame(program, starts, texture)[8..]);
        request
    };
    let snapshot = |gpu: &mut Graphics, id: u64| gpu.present_target(&id.to_le_bytes()).unwrap();
    gpu.call(108, &pass(a, 3, 0, &[3], None, None), 0).unwrap();
    let query = u64::from_le_bytes(gpu.call(111, &[], 8).unwrap().try_into().unwrap());
    // Back triangle is fully hidden by the retained front depth, across passes.
    gpu.call(115, &pass(a, 0, 0, &[0], None, Some(query)), 0)
        .unwrap();
    assert_eq!(
        gpu.call(115, &pass(a, 0, 0, &[0], None, Some(query)), 0),
        Err(Error::Busy)
    );
    let poll = |gpu: &mut Graphics| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match gpu.call(113, &query.to_le_bytes(), 8) {
                Ok(value) => break u64::from_le_bytes(value.try_into().unwrap()),
                Err(Error::Busy) => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
                other => panic!("query failed: {other:?}"),
            }
        }
    };
    assert_eq!(
        poll(&mut gpu),
        0,
        "hidden geometry has zero actual passing samples"
    );
    let green = snapshot(&mut gpu, a);
    let pixels = read_native_pixels(&state, &green);
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[0, 255, 0, 255]
    );
    // Clearing depth makes the exact same geometry visible: real positive control.
    gpu.call(115, &pass(a, 2, 0, &[0], None, Some(query)), 0)
        .unwrap();
    assert!(poll(&mut gpu) > 0);
    // A boolean query would pass both controls above. Require an exact known
    // count too: a full 64x64 quad at 4x MSAA has 16,384 passing samples.
    let mut quad = Vec::new();
    for (x, y) in [(-1f32, -1f32), (1., -1.), (1., 1.), (-1., 1.)] {
        for f in [x, y, 0.5, 1., 1., 1., 1.] {
            quad.extend(f.to_le_bytes());
        }
    }
    let qvertex = create(&mut gpu, 0, &quad);
    let qindex = create(
        &mut gpu,
        1,
        &[0u32, 1, 2, 0, 2, 3]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let mut exact = [b.to_le_bytes(), query.to_le_bytes()].concat();
    for n in [3u32, 0, 1f32.to_bits(), 1, 2] {
        exact.extend(n.to_le_bytes());
    }
    for id in [qvertex, qindex, uniform_id] {
        exact.extend(id.to_le_bytes());
    }
    for n in [0u32, 6, 0, 0, 0, 64, 64, 0] {
        exact.extend(n.to_le_bytes());
    }
    gpu.call(115, &exact, 0).unwrap();
    assert_eq!(
        poll(&mut gpu),
        64 * 64 * 4,
        "precise sample counts, not boolean visibility"
    );
    assert_eq!(
        &read_native_pixels(&state, &green)[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[0, 255, 0, 255],
        "accepted snapshot never changes"
    );
    let clear = [
        a.to_le_bytes().as_slice(),
        &1u32.to_le_bytes(),
        &[0u32, 0, 16, 16, 0x112233ff, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    gpu.call(110, &clear, 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(&pixels[..4], &[17, 34, 51, 255]);
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[255, 0, 0, 255]
    );
    let copy = [
        a.to_le_bytes().as_slice(),
        &a.to_le_bytes(),
        &[0u32, 0, 8, 8, 16, 16]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    gpu.call(107, &copy, 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(
        &pixels[20 * 256 + 20 * 4..20 * 256 + 20 * 4 + 4],
        &[17, 34, 51, 255],
        "overlap copy uses a snapshot"
    );
    // Prior target is a filterable texture; sampling output itself is denied.
    let mut sampled = pass(b, 3, 3, &[0], Some(a), None);
    sampled[12..16].copy_from_slice(&0x778899ffu32.to_le_bytes());
    gpu.call(108, &sampled, 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, b));
    assert_eq!(&pixels[..4], &[119, 136, 153, 255]);
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[1, 2, 3, 255],
        "later pass samples the resolved previous target"
    );
    assert_eq!(
        gpu.call(108, &pass(a, 3, 3, &[0], Some(a), None), 0),
        Err(Error::Invalid)
    );
    let before = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(
        gpu.call(108, &pass(a, 3, 999, &[0], None, None), 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        before,
        read_native_pixels(&state, &snapshot(&mut gpu, a)),
        "invalid pass has no effects"
    );
    // Color masks, winding and line/point lists are verified state variants.
    let mut masked = frame(7, &[3], None);
    masked[8..12].copy_from_slice(&0x112233ffu32.to_le_bytes());
    let pixels = read_native_pixels(&state, &gpu.render(&masked).unwrap());
    assert_eq!(
        &pixels[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[0, 34, 51, 255]
    );
    let cw = read_native_pixels(&state, &gpu.render(&frame(9, &[0], None)).unwrap());
    let ccw = read_native_pixels(&state, &gpu.render(&frame(10, &[0], None)).unwrap());
    let center = 32 * 256 + 32 * 4;
    assert_ne!(&cw[center..center + 4], &ccw[center..center + 4]);
    assert!(
        [&cw[center..center + 4], &ccw[center..center + 4]].contains(&[255, 0, 0, 255].as_slice())
    );
    assert!(matches!(
        gpu.render(&frame(11, &[0], None)),
        Err(Error::Invalid)
    ));
    let mut line = frame(11, &[1], None);
    line[52..56].copy_from_slice(&2u32.to_le_bytes());
    assert!(read_native_pixels(&state, &gpu.render(&line).unwrap())
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| *p != [1, 2, 3, 255]));
    gpu.render(&frame(12, &[0], None)).unwrap();
    let extended = |target: u64, mask: u32, raw: Vec<u8>, blend: [f32; 4], stencil: u32| {
        let mut bytes = [target.to_le_bytes(), 0u64.to_le_bytes()].concat();
        bytes.extend(mask.to_le_bytes());
        bytes.extend(&raw[8..20]);
        for draw in raw[20..].as_chunks::<60>().0 {
            bytes.extend(&draw[..4]);
            for f in blend {
                bytes.extend(f.to_le_bytes());
            }
            bytes.extend(stencil.to_le_bytes());
            bytes.extend(&draw[4..]);
        }
        bytes
    };
    let mut blended = frame(8, &[0], None);
    blended[24..32].copy_from_slice(&qvertex.to_le_bytes());
    blended[32..40].copy_from_slice(&qindex.to_le_bytes());
    blended[52..56].copy_from_slice(&6u32.to_le_bytes());
    gpu.call(123, &extended(a, 3, blended, [0.2, 0.4, 0.6, 1.], 0), 0)
        .unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(
        &pixels[center..center + 4],
        &[51, 102, 153, 255],
        "dynamic blend constant"
    );
    // Rendered cube-face/mip assembly preserves the other faces.
    let copy = [
        a.to_le_bytes().as_slice(),
        &texture.to_le_bytes(),
        &[32u32, 32, 1, 0, 0, 0, 1, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    gpu.call(116, &copy, 0).unwrap();
    for (direction, expected) in [
        ([1f32, 0., 0., 1.], [51, 102, 153, 255]),
        ([0., 1., 0., 1.], [0, 0, 255, 255]),
    ] {
        gpu.call(
            101,
            &[
                uniform_id.to_le_bytes().as_slice(),
                &0u64.to_le_bytes(),
                &direction
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
            ]
            .concat(),
            0,
        )
        .unwrap();
        let pixels =
            read_native_pixels(&state, &gpu.render(&frame(1, &[0], Some(texture))).unwrap());
        assert_eq!(&pixels[center..center + 4], &expected);
    }
    // Separate sampler filters and explicit LOD clamps work with opaque IDs.
    let sampler_request = [0u32, 0, 0, 0, 0, 0, 0f32.to_bits(), 0f32.to_bits(), 1]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let sampler = u64::from_le_bytes(
        gpu.call(122, &sampler_request, 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let mut sampled = frame(1, &[0], Some(texture));
    let end = sampled.len();
    sampled[end - 4..].copy_from_slice(&0x80000000u32.to_le_bytes());
    sampled.extend(sampler.to_le_bytes());
    let pixels = read_native_pixels(&state, &gpu.render(&sampled).unwrap());
    assert_eq!(&pixels[center..center + 4], &[5, 10, 15, 255]);
    gpu.call(104, &sampler.to_le_bytes(), 0).unwrap();
    assert!(matches!(gpu.render(&sampled), Err(Error::Denied)));
    let mut invalid_sampler = sampler_request.clone();
    invalid_sampler[24..28].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    assert_eq!(gpu.call(122, &invalid_sampler, 8), Err(Error::Invalid));
    gpu.call(
        101,
        &[
            uniform_id.to_le_bytes().as_slice(),
            &0u64.to_le_bytes(),
            &[1f32; 4]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        ]
        .concat(),
        0,
    )
    .unwrap();
    // Runtime state comes only from verified template indices and is prepared
    // off the owner thread. It never accepts another shader source/module.
    let (wake_send, wake_receive) = std::sync::mpsc::sync_channel(8);
    let owner = std::thread::current().id();
    gpu.set_wake(move || {
        let _ = wake_send.try_send(std::thread::current().id());
    });
    let mut descriptor = [0u32; 28];
    descriptor[0] = 0;
    descriptor[1] = 2;
    descriptor[4] = 1;
    descriptor[7] = 7;
    descriptor[13] = 7;
    descriptor[17] = 7;
    let encoded = |d: &[u32; 28]| d.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<_>>();
    let variant = u64::from_le_bytes(
        gpu.call(127, &encoded(&descriptor), 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let ready = |gpu: &mut Graphics, id: u64| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match gpu.call(128, &id.to_le_bytes(), 32) {
                Ok(hash) => break hash,
                Err(Error::Busy) => {
                    assert!(std::time::Instant::now() < deadline);
                    std::thread::yield_now();
                }
                other => panic!("variant {other:?}"),
            }
        }
    };
    let hash = ready(&mut gpu, variant);
    assert_eq!(hash.len(), 32);
    assert_ne!(
        wake_receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap(),
        owner
    );
    assert_eq!(
        gpu.call(127, &encoded(&descriptor), 8).unwrap(),
        variant.to_le_bytes()
    );
    let with_variant = |id: u64| {
        let mut request = extended(a, 3, frame(0, &[3], None), [0.; 4], 0);
        request[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        request.splice(36..36, id.to_le_bytes());
        request
    };
    gpu.call(123, &with_variant(variant), 0).unwrap();
    gpu.call(104, &variant.to_le_bytes(), 0).unwrap();
    assert_eq!(
        gpu.call(128, &variant.to_le_bytes(), 32),
        Err(Error::Denied)
    );
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(
        &pixels[center..center + 4],
        &[0, 2, 3, 255],
        "released in-flight variant retains exact state"
    );
    assert_eq!(gpu.call(123, &with_variant(variant), 0), Err(Error::Denied));
    descriptor[4] = 4;
    let other = u64::from_le_bytes(
        gpu.call(127, &encoded(&descriptor), 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    assert_ne!(ready(&mut gpu, other), hash);
    assert_ne!(
        wake_receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap(),
        owner
    );
    gpu.call(123, &with_variant(other), 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, a));
    assert_eq!(&pixels[center..center + 4], &[1, 2, 0, 255]);
    gpu.call(104, &other.to_le_bytes(), 0).unwrap();
    for (word, value) in [
        (0, u32::MAX),
        (1, 3),
        (4, 16),
        (6, 2),
        (9, f32::NAN.to_bits()),
        (11, 256),
        (22, 13),
    ] {
        let mut bad = descriptor;
        bad[word] = value;
        assert_eq!(gpu.call(127, &encoded(&bad), 8), Err(Error::Invalid));
    }
    descriptor[4] = 2;
    let canceled = u64::from_le_bytes(
        gpu.call(127, &encoded(&descriptor), 8)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    gpu.call(104, &canceled.to_le_bytes(), 0).unwrap();
    assert_eq!(
        gpu.call(128, &canceled.to_le_bytes(), 32),
        Err(Error::Denied)
    );
    assert_ne!(
        wake_receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap(),
        owner
    );
    // Two color targets share one independently owned depth/stencil identity.
    let depth = u64::from_le_bytes(
        gpu.call(
            118,
            &[64u32, 64, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    for target in [a, b] {
        gpu.call(
            119,
            &[target.to_le_bytes(), depth.to_le_bytes()].concat(),
            0,
        )
        .unwrap();
    }
    gpu.call(108, &pass(a, 3, 6, &[3], None, None), 0).unwrap();
    gpu.call(108, &pass(b, 1, 6, &[0], None, None), 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, b));
    assert_eq!(
        &pixels[center..center + 4],
        &[1, 2, 3, 255],
        "shared depth survives switching color targets"
    );
    let clear_stencil = |x: u32, width: u32| {
        [
            a.to_le_bytes().as_slice(),
            &[4u32, x, 0, width, 64, 0, 1f32.to_bits(), 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ]
        .concat()
    };
    gpu.call(120, &clear_stencil(0, 64), 0).unwrap();
    gpu.call(123, &extended(a, 0, frame(4, &[3], None), [0.; 4], 7), 0)
        .unwrap();
    gpu.call(123, &extended(b, 1, frame(5, &[0], None), [0.; 4], 7), 0)
        .unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, b));
    assert_eq!(&pixels[center..center + 4], &[255, 0, 0, 255]);
    gpu.call(120, &clear_stencil(0, 32), 0).unwrap();
    gpu.call(104, &depth.to_le_bytes(), 0).unwrap();
    assert_eq!(
        gpu.call(119, &[a.to_le_bytes(), depth.to_le_bytes()].concat(), 0),
        Err(Error::Denied)
    );
    gpu.call(123, &extended(b, 1, frame(5, &[0], None), [0.; 4], 7), 0)
        .unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, b));
    assert_eq!(
        &pixels[32 * 256 + 24 * 4..32 * 256 + 24 * 4 + 4],
        &[1, 2, 3, 255]
    );
    assert_eq!(
        &pixels[32 * 256 + 40 * 4..32 * 256 + 40 * 4 + 4],
        &[255, 0, 0, 255],
        "shared stencil survives partial clear and released depth ID"
    );
    // Readback is asynchronous, packed and retained even if the source is released.
    let read_request = [
        b.to_le_bytes().as_slice(),
        &[40u32, 31, 3, 2]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    ]
    .concat();
    let ticket = u64::from_le_bytes(gpu.call(124, &read_request, 8).unwrap().try_into().unwrap());
    assert_eq!(gpu.call(125, &ticket.to_le_bytes(), 4), Err(Error::Invalid));
    gpu.call(104, &b.to_le_bytes(), 0).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let bytes = loop {
        match gpu.call(125, &ticket.to_le_bytes(), 24) {
            Ok(bytes) => break bytes,
            Err(Error::Busy) => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            other => panic!("readback {other:?}"),
        }
    };
    assert_eq!(bytes, [255u8, 0, 0, 255].repeat(6));
    assert_eq!(gpu.call(125, &ticket.to_le_bytes(), 24), Err(Error::Denied));
    // Release of a submitted query/target is safe and immediately denies stale IDs.
    gpu.call(115, &pass(a, 2, 6, &[0], None, Some(query)), 0)
        .unwrap();
    gpu.call(104, &query.to_le_bytes(), 0).unwrap();
    assert_eq!(gpu.call(113, &query.to_le_bytes(), 8), Err(Error::Denied));
    gpu.call(104, &a.to_le_bytes(), 0).unwrap();
    assert!(matches!(
        gpu.present_target(&a.to_le_bytes()),
        Err(Error::Denied)
    ));
    assert_eq!(
        &read_native_pixels(&state, &green)[32 * 256 + 32 * 4..32 * 256 + 32 * 4 + 4],
        &[0, 255, 0, 255]
    );
    // Preferred float32+stencil is explicit, never silently downgraded.
    let c = target(&mut gpu);
    let z32 = u64::from_le_bytes(
        gpu.call(
            118,
            &[64u32, 64, 2]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    gpu.call(119, &[c.to_le_bytes(), z32.to_le_bytes()].concat(), 0)
        .unwrap();
    gpu.call(108, &pass(c, 3, 13, &[3, 0], None, None), 0)
        .unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, c));
    assert_eq!(&pixels[center..center + 4], &[0, 255, 0, 255]);
    gpu.call(
        120,
        &[
            c.to_le_bytes().as_slice(),
            &[4u32, 0, 0, 64, 64, 0, 1f32.to_bits(), 17]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        ]
        .concat(),
        0,
    )
    .unwrap();
    let mut pending = Vec::new();
    for _ in 0..8 {
        pending.push(u64::from_le_bytes(
            gpu.call(
                124,
                &[
                    c.to_le_bytes().as_slice(),
                    &[32u32, 32, 1, 1]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>(),
                ]
                .concat(),
                8,
            )
            .unwrap()
            .try_into()
            .unwrap(),
        ));
    }
    assert_eq!(
        gpu.call(
            124,
            &[
                c.to_le_bytes().as_slice(),
                &[32u32, 32, 1, 1]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            ]
            .concat(),
            8
        ),
        Err(Error::Busy)
    );
    for id in pending {
        gpu.call(104, &id.to_le_bytes(), 0).unwrap();
        assert_eq!(gpu.call(125, &id.to_le_bytes(), 4), Err(Error::Denied));
    }
    gpu.call(104, &z32.to_le_bytes(), 0).unwrap();
    gpu.call(104, &c.to_le_bytes(), 0).unwrap();
    // Typed template guards run before GPU execution, and cached validation is invalidated
    // by every uniform edit. Vertex NaN bookkeeping also recovers after edits.
    let mut words = vec![0u32; gamepkg::shaders::nv2a::BYTES / 4];
    words[..16].copy_from_slice(&[
        0x3149564e, 1, 13072, 0, 0, 0, 0x1e19, 6, 0, 0, 0, 0, 0, 8, 0, 0,
    ]);
    words[3264..].copy_from_slice(&[1f32.to_bits(), 1f32.to_bits(), 1f32.to_bits(), 0]);
    let nvu = create(
        &mut gpu,
        2,
        &words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .collect::<Vec<_>>(),
    );
    let mut vertices = vec![0f32; 3 * 64];
    for (i, (x, y)) in [(-1f32, -1f32), (1., -1.), (0., 1.)]
        .into_iter()
        .enumerate()
    {
        vertices[i * 64..i * 64 + 4].copy_from_slice(&[x, y, 0.5, 1.]);
    }
    let nvv = create(
        &mut gpu,
        0,
        &vertices
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    let dummy = u64::from_le_bytes(
        gpu.call(
            102,
            &[1u32, 1, 1, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            8,
        )
        .unwrap()
        .try_into()
        .unwrap(),
    );
    let mut nvi = frame(14, &[0], None);
    nvi[24..32].copy_from_slice(&nvv.to_le_bytes());
    nvi[40..48].copy_from_slice(&nvu.to_le_bytes());
    nvi[76..80].copy_from_slice(&4u32.to_le_bytes());
    for _ in 0..4 {
        nvi.extend(dummy.to_le_bytes());
        nvi.extend(0u32.to_le_bytes());
    }
    assert_eq!(
        &read_native_pixels(&state, &gpu.render(&nvi).unwrap())[center..center + 4],
        &[255, 0, 255, 255]
    );
    let write_word = |gpu: &mut Graphics, id: u64, offset: u64, value: u32| {
        gpu.call(
            101,
            &[
                id.to_le_bytes().as_slice(),
                &offset.to_le_bytes(),
                &value.to_le_bytes(),
            ]
            .concat(),
            0,
        )
        .unwrap()
    };
    write_word(&mut gpu, nvu, 12, 1);
    assert!(matches!(gpu.render(&nvi), Err(Error::Invalid)));
    let untouched = target(&mut gpu);
    assert_eq!(
        gpu.call(
            108,
            &[
                untouched.to_le_bytes().as_slice(),
                &3u32.to_le_bytes(),
                &nvi[8..]
            ]
            .concat(),
            0
        ),
        Err(Error::Invalid)
    );
    assert_eq!(
        &read_native_pixels(&state, &snapshot(&mut gpu, untouched))[center..center + 4],
        &[0, 0, 0, 0]
    );
    write_word(&mut gpu, nvu, 12, 0);
    gpu.render(&nvi).unwrap();
    write_word(&mut gpu, nvv, 0, f32::NAN.to_bits());
    assert!(matches!(gpu.render(&nvi), Err(Error::Invalid)));
    write_word(&mut gpu, nvv, 0, (-1f32).to_bits());
    gpu.render(&nvi).unwrap();
    write_word(&mut gpu, nvu, 64, f32::NAN.to_bits());
    assert!(matches!(gpu.render(&nvi), Err(Error::Invalid)));
    write_word(&mut gpu, nvu, 64, 0);
    write_word(&mut gpu, nvu, 12768, 2);
    assert!(matches!(gpu.render(&nvi), Err(Error::Unsupported)));
    write_word(&mut gpu, nvu, 12768, 0);
    gpu.render(&nvi).unwrap();
    for id in [nvu, nvv, dummy, untouched] {
        gpu.call(104, &id.to_le_bytes(), 0).unwrap();
    }
    // Viewport is a transform, not just a scissor. Two halves retain distinct
    // geometry, and a depth-range change must reverse the visibility control.
    gpu.call(
        101,
        &[
            uniform_id.to_le_bytes().as_slice(),
            &0u64.to_le_bytes(),
            &[1f32; 4]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        ]
        .concat(),
        0,
    )
    .unwrap();
    let viewport_target = target(&mut gpu);
    let viewport_pass = |mask: u32, raw: Vec<u8>, viewports: &[[f32; 6]]| {
        let raw = extended(viewport_target, mask, raw, [0.; 4], 0);
        let mut out = raw[..32].to_vec();
        assert_eq!((raw.len() - 32) / 80, viewports.len());
        for (draw, viewport) in raw[32..].as_chunks::<80>().0.iter().zip(viewports) {
            out.extend(&draw[..24]);
            out.extend(viewport.iter().flat_map(|f| f.to_le_bytes()));
            out.extend(&draw[24..]);
        }
        out
    };
    let halves = viewport_pass(
        3,
        frame(0, &[0, 3], None),
        &[[0., 0., 32., 64., 0., 1.], [32., 0., 32., 64., 0., 1.]],
    );
    gpu.call(129, &halves, 0).unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, viewport_target));
    assert_eq!(
        &pixels[32 * 256 + 16 * 4..32 * 256 + 16 * 4 + 4],
        &[255, 0, 0, 255]
    );
    assert_eq!(
        &pixels[32 * 256 + 48 * 4..32 * 256 + 48 * 4 + 4],
        &[0, 255, 0, 255]
    );
    gpu.call(
        129,
        &viewport_pass(3, frame(0, &[0], None), &[[0., 0., 64., 64., 0., 1.]]),
        0,
    )
    .unwrap();
    gpu.call(
        129,
        &viewport_pass(0, frame(0, &[3], None), &[[0., 0., 64., 64., 0.9, 1.]]),
        0,
    )
    .unwrap();
    let pixels = read_native_pixels(&state, &snapshot(&mut gpu, viewport_target));
    assert_eq!(
        &pixels[center..center + 4],
        &[255, 0, 0, 255],
        "depth-range transform rejects the otherwise nearer green triangle"
    );
    gpu.call(
        129,
        &viewport_pass(0, frame(0, &[3], None), &[[0., 0., 64., 64., 1., 0.]]),
        0,
    )
    .unwrap();
    let mut malformed = halves.clone();
    malformed[56..60].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    let before = read_native_pixels(&state, &snapshot(&mut gpu, viewport_target));
    assert_eq!(gpu.call(129, &malformed, 0), Err(Error::Invalid));
    assert_eq!(
        before,
        read_native_pixels(&state, &snapshot(&mut gpu, viewport_target))
    );
    for (border, expected) in [
        (0u32, [0, 0, 0, 0]),
        (1, [0, 0, 0, 255]),
        (2, [255, 255, 255, 255]),
    ] {
        let mut request = sampler_request.clone();
        request[12..16].copy_from_slice(&3u32.to_le_bytes());
        request.extend(border.to_le_bytes());
        if device
            .features()
            .contains(wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER)
        {
            let sampler =
                u64::from_le_bytes(gpu.call(135, &request, 8).unwrap().try_into().unwrap());
            let mut sampled = frame(3, &[0], Some(viewport_target));
            let end = sampled.len();
            sampled[end - 4..].copy_from_slice(&0x80000000u32.to_le_bytes());
            sampled.extend(sampler.to_le_bytes());
            assert_eq!(
                &read_native_pixels(&state, &gpu.render(&sampled).unwrap())[center..center + 4],
                &expected
            );
            gpu.call(104, &sampler.to_le_bytes(), 0).unwrap();
        } else {
            assert_eq!(gpu.call(135, &request, 8), Err(Error::Unsupported));
        }
    }
    gpu.call(104, &viewport_target.to_le_bytes(), 0).unwrap();
    // Small requests cannot amplify index validation into unbounded owner work.
    let huge = create(&mut gpu, 1, &vec![0u8; 1_048_578 * 4]);
    let mut amplified = frame(2, &[0], None);
    amplified[32..40].copy_from_slice(&huge.to_le_bytes());
    amplified[52..56].copy_from_slice(&1_048_578u32.to_le_bytes());
    assert!(matches!(gpu.render(&amplified), Err(Error::Busy)));
    gpu.call(104, &huge.to_le_bytes(), 0).unwrap();
    let mut invalid = [index_id.to_le_bytes(), 0u64.to_le_bytes()].concat();
    invalid.extend(u32::MAX.to_le_bytes());
    gpu.call(101, &invalid, 0).unwrap();
    assert!(matches!(
        gpu.render(&frame(0, &[0], None)),
        Err(Error::Invalid)
    ));
    assert_eq!(
        gpu.call(
            101,
            &[
                vertex_id.to_le_bytes().as_slice(),
                &u64::MAX.to_le_bytes(),
                &[0; 4]
            ]
            .concat(),
            0
        ),
        Err(Error::Invalid)
    );
    let mut upload = texture.to_le_bytes().to_vec();
    for n in [0u32, 6, 0, 0, 1, 1] {
        upload.extend(n.to_le_bytes());
    }
    upload.extend([0; 4]);
    assert_eq!(gpu.call(103, &upload, 0), Err(Error::Invalid));
    gpu.call(104, &vertex_id.to_le_bytes(), 0).unwrap();
    assert!(matches!(
        gpu.render(&frame(0, &[0], None)),
        Err(Error::Denied)
    ));
}
