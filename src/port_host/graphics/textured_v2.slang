// GamePKG host-reviewed textured-v2 raster template.
// Self-contained: no includes, no platform assets, no game-supplied source.
// The signed package must carry these exact bytes under shaders/textured_v2.slang.
// One sampled 2D texture/sampler pair, one tint uniform, no extra texture work.

struct Uniforms {
    float4 tint;
    float4 reserved0;
    float4 reserved1;
    float4 reserved2;
};

[[vk::binding(0, 0)]] ConstantBuffer<Uniforms> gamepkg_uniforms;
[[vk::binding(1, 0)]] Texture2D<float4> gamepkg_texture;
[[vk::binding(2, 0)]] SamplerState gamepkg_sampler;

// Explicit Vulkan locations keep Slang's WGSL vertex-input order identical to
// the reviewed host recipe. HLSL semantics are only stage compatibility hints.
struct VertexInput {
    [[vk::location(0)]] float4 position;
    [[vk::location(1)]] float2 uv;
    [[vk::location(2)]] float4 color;
};

struct VertexOutput {
    [[vk::location(0)]] float4 position : SV_Position;
    [[vk::location(1)]] float2 uv : TEXCOORD0;
    [[vk::location(2)]] float4 color : COLOR;
};

[shader("vertex")]
VertexOutput vertexMain(VertexInput input) {
    VertexOutput output;
    output.position = input.position;
    output.uv = input.uv;
    output.color = saturate(input.color);
    return output;
}

[shader("fragment")]
float4 fragmentMain(VertexOutput input) : SV_Target {
    float4 texel = gamepkg_texture.Sample(gamepkg_sampler, input.uv);
    return saturate(texel * input.color * gamepkg_uniforms.tint);
}
