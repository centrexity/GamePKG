[shader("fragment")]
float4 blueMain() : SV_Target { return float4(0.1, 0.2, 0.8, 1.0); }
[shader("fragment")]
float4 greenMain() : SV_Target { return float4(0.1, 0.8, 0.2, 1.0); }
