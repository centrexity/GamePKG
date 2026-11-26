//! Host-reviewed `textured-v1` recipe for native service packages.
//!
//! The signed package carries the exact Slang bytes pinned below. Games still
//! supply only opaque buffer/texture IDs and draw data; this catalog is the
//! only way a service-runtime package can obtain a textured host pipeline.
use super::{PipelineRecipe, UniformPolicy};
use crate::manifest::{
    Shader, TEXTURED_V1_FRAGMENT_ENTRY, TEXTURED_V1_SHADER_SOURCE, TEXTURED_V1_VERTEX_ENTRY,
};
use eframe::wgpu;

/// Exact host-reviewed Slang source. `textured_v1_shaders()` names this file;
/// `PipelineRecipe::source_hashes` pins its bytes during build preparation.
pub const TEXTURED_V1_SHADER: &str = include_str!("textured_v1.slang");
pub const TEXTURED_V1_SHA256: &str =
    "4f094cf63c8fcd377f6a8f7959941dcd5689fb091d46f6004a78c2bb73529e3f";

pub fn textured_v1_shaders() -> [Shader; 2] {
    [
        Shader {
            source: TEXTURED_V1_SHADER_SOURCE.into(),
            entry: TEXTURED_V1_VERTEX_ENTRY.into(),
            stage: "vertex".into(),
        },
        Shader {
            source: TEXTURED_V1_SHADER_SOURCE.into(),
            entry: TEXTURED_V1_FRAGMENT_ENTRY.into(),
            stage: "fragment".into(),
        },
    ]
}

/// One filterable RGBA8 D2 texture/sampler pair, fixed vertex layout, and a
/// 64-byte uniform block. Material/TEV-like inputs are explicit vertex data.
pub fn textured_v1_recipe() -> PipelineRecipe {
    let [vertex, fragment] = textured_v1_shaders();
    let attribute = |shader_location, format, offset| wgpu::VertexAttribute {
        format,
        offset,
        shader_location,
    };
    PipelineRecipe {
        vertex,
        fragment,
        source_hashes: Some([TEXTURED_V1_SHA256; 2]),
        stride: 64,
        attributes: vec![
            attribute(0, wgpu::VertexFormat::Float32x4, 0),
            attribute(1, wgpu::VertexFormat::Float32x2, 16),
            attribute(2, wgpu::VertexFormat::Float32x4, 32),
            attribute(3, wgpu::VertexFormat::Float32x4, 48),
        ],
        uniform_bytes: 64,
        textures: vec![wgpu::TextureViewDimension::D2],
        blend: None,
        cull: None,
        depth_write: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        raster: Default::default(),
        uniform_policy: UniformPolicy::Opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_source_matches_host_pin() {
        assert_eq!(
            crate::verify::digest(TEXTURED_V1_SHADER.as_bytes()),
            TEXTURED_V1_SHA256
        );
    }

    #[test]
    fn recipe_uses_the_reviewed_bounded_layout() {
        let recipe = textured_v1_recipe();
        assert_eq!(recipe.vertex.stage, "vertex");
        assert_eq!(recipe.fragment.stage, "fragment");
        assert_eq!(recipe.vertex.entry, TEXTURED_V1_VERTEX_ENTRY);
        assert_eq!(recipe.fragment.entry, TEXTURED_V1_FRAGMENT_ENTRY);
        assert_eq!(recipe.stride, 64);
        assert_eq!(recipe.uniform_bytes, 64);
        assert_eq!(recipe.textures, vec![wgpu::TextureViewDimension::D2]);
        assert_eq!(recipe.attributes.len(), 4);
        assert_eq!(
            recipe
                .attributes
                .iter()
                .map(|a| (a.shader_location, a.offset, a.format))
                .collect::<Vec<_>>(),
            vec![
                (0, 0, wgpu::VertexFormat::Float32x4),
                (1, 16, wgpu::VertexFormat::Float32x2),
                (2, 32, wgpu::VertexFormat::Float32x4),
                (3, 48, wgpu::VertexFormat::Float32x4),
            ]
        );
    }
}
