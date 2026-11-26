// GamePKG host-reviewed textured-v1 raster template.
// Self-contained: no includes, no platform assets, no game-supplied source.
// The signed package must carry this exact bytes under shaders/textured_v1.slang.

struct Uniforms {
    // rgb/alpha modulate the sampled texel; vertex_gain scales the per-vertex
    // material/TEV parameter, additive is a bounded final offset.
    float4 modulation;
    float4 vertex_gain;
    float4 additive;
    float4 reserved;
};

[[vk::binding(0, 0)]] ConstantBuffer<Uniforms> gamepkg_uniforms;
[[vk::binding(1, 0)]] Texture2D<float4> gamepkg_texture;
[[vk::binding(2, 0)]] SamplerState gamepkg_sampler;

// Only explicit Vulkan locations are used here. HLSL semantic names reorder
// Slang's WGSL vertex-input locations and would break the reviewed layout.
struct VertexInput {
    [[vk::location(0)]] float4 position;
    [[vk::location(1)]] float2 uv;
    [[vk::location(2)]] float4 color;
    [[vk::location(3)]] float4 params;
};

struct VertexOutput {
    [[vk::location(0)]] float4 position : SV_Position;
    [[vk::location(1)]] float2 uv : TEXCOORD0;
    [[vk::location(2)]] float4 color : COLOR;
    [[vk::location(3)]] float4 params : TEXCOORD1;
};

[shader("vertex")]
VertexOutput vertexMain(VertexInput input) {
    VertexOutput output;
    output.position = input.position;
    output.uv = input.uv;
    output.color = saturate(input.color);
    output.params = input.params;
    return output;
}

[shader("fragment")]
float4 fragmentMain(VertexOutput input) : SV_Target {
    float4 texel = gamepkg_texture.Sample(gamepkg_sampler, input.uv);
    float4 combined = texel * input.color * gamepkg_uniforms.modulation
        + input.params * gamepkg_uniforms.vertex_gain
        + gamepkg_uniforms.additive;
    return saturate(combined);
}
