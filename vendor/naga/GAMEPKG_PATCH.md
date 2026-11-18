# GamePKG nonfinite shader classification

Base: Naga 24.0.0, registry archive SHA-256
`e380993072e52eef724eddfcde0ed013b0c023c3f0417336ed041aa9f076994e`.
Upstream MIT/Apache licenses retained. This is a host dependency, not a game
dependency or an exception to source/object admission.

The WGSL writer now lowers validated f32 scalar/vector `IsNan` and `IsInf`
expressions using bitcasts and integer magnitude masks. NaN means magnitude
bits above 0x7f800000; infinity means equality. Sign, NaN payload and quiet/
signaling distinctions do not change classification. Unsupported widths still
fail explicitly. Ordinary floating self-comparison is deliberately avoided.
All and Any retain their original behavior. The app reparses and validates the
emitted WGSL before installation. The SDK identity covers the writer patch.

Explicit elided lifetimes in upstream arena/compact/front/proc signatures only
address current compiler diagnostics; these do not change execution behavior.
No shader validation, resource limit, native source policy or game-object
enforcement is removed. Requalify this lowering when updating Naga.
