//! Reviewed NVI1 recipe metadata. Slang and the game renderer remain port inputs.
use super::{PipelineRecipe, UniformPolicy};
use crate::manifest::Shader;
use eframe::wgpu;

// Reviewed ports commit de70bb05b3e37058ca4656f2559f34397d573a85.
// The source kit must provide these exact self-contained Slang files.
const TEMPLATE_SHA256: [&str; 16] = [
    "fbe4703a9b113f56070db7d333e83563a03812032393cda2823e82d6ac1c27b6",
    "dc62f5e037edc03a9906b905fa62858e6550b8f4daa03ebc654e05ed8dd14b1c",
    "fae96458424a0c7a46a6a244d22c6facdf27e14871239a9489202641f6ea5516",
    "36933b7d7bd606c1424dbbbaa3bfaad9715567eabff85977e7fdf7d9a2b2155a",
    "5e62eaa90553e874bb9ec7737cdfb7bf08e376522a3135e8408beabb4212f8f8",
    "ebec2f90f1cd38d74aa93ca8d0072837b4883236ed3d0e4297b35de91ddd5920",
    "4e60e0b4d4f9d58bf0f893ebccb041d231c6310542ffa4fff7c86a1e124718e7",
    "cfe59387ac22a55d2d62683f13cc30824d85618613eee0474ec73004ee60949f",
    "2b1f69cbf5ed5533a266c659693e21890303e1cf31ca57758db18870c4d69aa6",
    "8da9a33c82e9419e6f1c5fd0f608d03e2ee092279cff355ad6ca4d89c073dc24",
    "b577bdb612ad544c22e5b3ad40b7c14caa5bc9e1dee5fcfd1d48667c3ddf74f4",
    "c7957a72920e32d5d455557e8abc451a60f2490942f02afca5ff422cfc17acd2",
    "6422cbf8a6e25a1a2dbce5675f04ed1142135177adc97628c6995807d1914b65",
    "e7ab560b1e85d2397edb8924692f75bf3982da4b4b27a931a848dd65d020ecaa",
    "ca13c33e98f624c7584b2ed94520882209dd53ac2ed06fb4a9d51470d83f9c6e",
    "cfffce1d5936391e4801d641314a21216c9eff65b355b7738195687133624ab4",
];

/// Pipeline index equals the four texture-dimension bits (one bit per cube
/// slot). This is the base SDR layout. Games select reviewed render-state
/// variants through the existing service.
pub fn takedawn_nvi1_recipes() -> Vec<PipelineRecipe> {
    TEMPLATE_SHA256
        .iter()
        .enumerate()
        .map(|(mask, &hash)| {
            let source = format!("shaders/template_{mask:02}.slang");
            let cube = std::array::from_fn(|slot| mask & (1 << slot) != 0);
            PipelineRecipe {
                vertex: Shader {
                    source: source.clone(),
                    entry: "vertexMain".into(),
                    stage: "vertex".into(),
                },
                fragment: Shader {
                    source,
                    entry: "fragmentMain".into(),
                    stage: "fragment".into(),
                },
                source_hashes: Some([hash; 2]),
                stride: 256,
                attributes: (0..16)
                    .map(|location| wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 16 * u64::from(location),
                        shader_location: location,
                    })
                    .collect(),
                uniform_bytes: crate::shaders::nv2a::BYTES as u64,
                textures: cube
                    .map(|is_cube| {
                        if is_cube {
                            wgpu::TextureViewDimension::Cube
                        } else {
                            wgpu::TextureViewDimension::D2
                        }
                    })
                    .into(),
                blend: None,
                cull: None,
                depth_write: false,
                depth_compare: wgpu::CompareFunction::Always,
                raster: Default::default(),
                uniform_policy: UniformPolicy::Nv2aV1 { cube },
            }
        })
        .collect()
}
