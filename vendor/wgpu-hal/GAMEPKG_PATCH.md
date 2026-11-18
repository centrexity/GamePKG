# GamePKG precise Vulkan occlusion extension

Base: crates.io wgpu-hal **24.0.4**, registry archive SHA-256
`f112f464674ca69f3533248508ee30cb84c67cf06c25ff6800685f5e0294e259`.
Upstream MIT/Apache licenses are retained. This is trusted host code, not a game
dependency or a permission to link any graphics library from game units.

wgpu 24's public Occlusion query contract promises boolean visibility. Its
Vulkan backend neither enables occlusionQueryPrecise nor requests PRECISE. A
positive/zero test cannot establish exact counts. GamePKG requires actual
passing-sample counts for migrated ports, so this small host patch:

* Queries physical support and enables occlusionQueryPrecise during the normal
  HAL-owned Vulkan device creation, only when supported.
* Tracks the actually enabled bit on DeviceShared. Raw externally supplied
  devices remain unqualified, even on hardware that supports the feature.
* Records query type and adds PRECISE only to occlusion queries on qualified
  devices; timestamp/statistics queries retain their original flags.
* Exposes only a boolean host-inspection method, gamepkg_precise_occlusion().
  No new raw GPU capability crosses the game-facing service boundary.

Changed implementation files: src/vulkan/{adapter,command,device,mod}.rs. The
two existing elided return lifetimes in vulkan/device.rs and gles/egl.rs are
also made explicit for current Rust diagnostics, without behavior changes. The
SDK identity includes all four files and the root dependency manifest/lock.
The service refuses count-query creation with Unsupported on unqualified
devices. The real offscreen test requires exactly 16,384 samples for a full
64x64 quad at 4x MSAA, alongside hidden-zero and visible-positive controls.

No rendering-state, raster, blending, postprocessing or default changes are
introduced by this patch. When updating wgpu, explicitly review its precise
query behavior and rerun the exact-count test; do not silently drop this guard.
