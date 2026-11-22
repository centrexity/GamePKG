//! Host converter regression, not a game compute capability.
#![cfg(feature = "gui")]
use eframe::wgpu;
use wgpu::util::DeviceExt;

#[test]
#[ignore = "requires hardware Vulkan; offscreen converter classification test"]
fn converted_nonfinite_classification_preserves_ieee_bits_on_gpu() {
    let source = r#"
@group(0) @binding(0) var<storage,read_write> data:array<u32>;
@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id:vec3<u32>){
 let i=id.x*4u;
 let x=bitcast<vec4<f32>>(vec4(data[i],data[i+1u],data[i+2u],data[i+3u]));
 let n=select(vec4<u32>(0u),vec4<u32>(1u),x<vec4<f32>(0.0));
 let f=select(vec4<u32>(0u),vec4<u32>(1u),x>vec4<f32>(0.0));
 let s=select(0u,1u,x.x<0.0);
 let t=select(0u,1u,x.x>0.0);
 data[i]=n.x|(f.x<<1u)|(s<<2u)|(t<<3u);
 data[i+1u]=n.y|(f.y<<1u); data[i+2u]=n.z|(f.z<<1u); data[i+3u]=n.w|(f.w<<1u);
}"#;
    let mut module = naga::front::wgsl::parse_str(source).unwrap();
    let mut replaced = 0;
    for (_, expression) in module.entry_points[0].function.expressions.iter_mut() {
        if let naga::Expression::Binary { op, left, .. } = *expression {
            let fun = match op {
                naga::BinaryOperator::Less => naga::RelationalFunction::IsNan,
                naga::BinaryOperator::Greater => naga::RelationalFunction::IsInf,
                _ => continue,
            };
            *expression = naga::Expression::Relational {
                fun,
                argument: left,
            };
            replaced += 1;
        }
    }
    assert_eq!(replaced, 4);
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    );
    let info = validator.validate(&module).unwrap();
    let converted =
        naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
            .unwrap();
    validator
        .validate(&naga::front::wgsl::parse_str(&converted).unwrap())
        .unwrap();
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    assert_ne!(adapter.get_info().device_type, wgpu::DeviceType::Cpu);
    eprintln!(
        "Exact nonfinite classification on {}",
        adapter.get_info().name
    );
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .unwrap();
    let bits: [u32; 24] = [
        0, 0x80000000, 1, 0x80000001, 0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff, 0x7f800000,
        0xff800000, 0x7f800001, 0xff800001, 0x7fc00000, 0xffc00000, 0x7fffffff, 0xffffffff,
        0x00800000, 0x80800000, 0x007fffff, 0x807fffff, 0x7fa00001, 0xffa00001, 0x3f000000,
        0xbf000000,
    ];
    let bytes: Vec<u8> = bits.iter().flat_map(|x| x.to_le_bytes()).collect();
    let input = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: &bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes.len() as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(converted.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: input.as_entire_binding(),
        }],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.dispatch_workgroups(6, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&input, 0, &output, 0, bytes.len() as u64);
    queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    output
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    device.poll(wgpu::Maintain::Wait);
    receive.recv().unwrap().unwrap();
    let mapped = output.slice(..).get_mapped_range();
    for (i, (word, original)) in mapped.as_chunks::<4>().0.iter().zip(bits).enumerate() {
        let magnitude = original & 0x7fffffff;
        let expected =
            u32::from(magnitude > 0x7f800000) | (u32::from(magnitude == 0x7f800000) << 1);
        let expected = if i % 4 == 0 {
            expected | (expected << 2)
        } else {
            expected
        };
        assert_eq!(u32::from_le_bytes(*word), expected, "bits {original:08x}");
    }
}
