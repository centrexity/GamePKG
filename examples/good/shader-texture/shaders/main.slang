Sampler2D gamepkgTexture;
[shader("fragment")]
float4 blueMain(float4 position : SV_Position) : SV_Target { return gamepkgTexture.Sample(position.xy / float2(640, 480)); }
[shader("fragment")]
float4 greenMain(float4 position : SV_Position) : SV_Target { return gamepkgTexture.Sample(position.xy / float2(640, 480)); }
