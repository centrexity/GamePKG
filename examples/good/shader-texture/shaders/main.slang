#if GAMEPKG_SEPARATE_SAMPLER
Texture2D<float4> gamepkgTexture;
SamplerState gamepkgSampler;
float4 sampleImage(float2 uv) { return gamepkgTexture.Sample(gamepkgSampler, uv); }
#else
Sampler2D gamepkgTexture;
float4 sampleImage(float2 uv) { return gamepkgTexture.Sample(uv); }
#endif
[shader("fragment")]
float4 blueMain(float4 position : SV_Position) : SV_Target { return sampleImage(position.xy / float2(640, 480)); }
[shader("fragment")]
float4 greenMain(float4 position : SV_Position) : SV_Target { return sampleImage(position.xy / float2(640, 480)); }
