//! Signed shader preparation and the private native pipeline payload.
use super::{Pipeline, RasterState, UniformPolicy};
use crate::{archive::Package, manifest::Shader, port_host::admission::CheckedSources};
use anyhow::{ensure, Context, Result};
use eframe::wgpu;
use std::path::Path;

/// Reviewed host layout and exact stage selectors. A signed package only
/// supplies the Slang files named by these selectors.
pub struct PipelineRecipe {
    pub vertex: Shader,
    pub fragment: Shader,
    /// Optional host pins for fixed program catalogs. In vertex, fragment order.
    pub source_hashes: Option<[&'static str; 2]>,
    pub stride: u64,
    pub attributes: Vec<wgpu::VertexAttribute>,
    pub uniform_bytes: u64,
    pub textures: Vec<wgpu::TextureViewDimension>,
    pub blend: Option<wgpu::BlendState>,
    pub cull: Option<wgpu::Face>,
    pub depth_write: bool,
    pub depth_compare: wgpu::CompareFunction,
    pub raster: RasterState,
    pub uniform_policy: UniformPolicy,
}

/// Built only from a checked package snapshot and a host-reviewed recipe.
/// Neither bootstrap arguments nor game-facing APIs can construct this type.
pub struct VerifiedPipelines {
    source_kit: String,
    preparation: String,
    samples: u32,
    pipelines: Vec<Pipeline>,
}

impl VerifiedPipelines {
    pub fn prepare(
        cache: &Path,
        sources: &CheckedSources,
        package: &Package,
        settings: &crate::settings::Settings,
        recipes: Vec<PipelineRecipe>,
        samples: u32,
    ) -> Result<Self> {
        ensure!(
            !recipes.is_empty() && recipes.len() <= 256 && matches!(samples, 1 | 2 | 4 | 8),
            "invalid native pipeline count or samples"
        );
        ensure!(
            recipes.iter().all(
                |recipe| recipe.vertex.stage == "vertex" && recipe.fragment.stage == "fragment"
            ),
            "invalid reviewed native pipeline recipe"
        );
        // Verify and compile the same private bytes, including shader includes.
        let mut snapshot = tempfile::NamedTempFile::new()?;
        package.copy_verified_archive(&mut snapshot)?;
        let mut package = Package::open(snapshot.path())?;
        let checked = CheckedSources::verify(&mut package)?;
        let package_digest = package.archive_digest()?;
        ensure!(
            checked.identity() == sources.identity() && checked.files().eq(sources.files()),
            "shader package differs from checked game source kit"
        );
        for recipe in &recipes {
            if let Some(pins) = recipe.source_hashes {
                for (shader, expected) in [&recipe.vertex, &recipe.fragment].into_iter().zip(pins) {
                    ensure!(
                        crate::verify::digest(&package.read(&shader.source)?) == expected,
                        "shader source differs from reviewed template: {}",
                        shader.source
                    );
                }
            }
        }
        let selections = recipes
            .iter()
            .flat_map(|recipe| [&recipe.vertex, &recipe.fragment])
            .cloned()
            .collect::<Vec<_>>();
        let (stages, preparation) = crate::shaders::compile_native_sources_with_identity(
            cache,
            &package_digest,
            &mut package,
            &selections,
            settings,
        )?;
        let mut stages = stages.into_iter();
        let mut pipelines = Vec::with_capacity(recipes.len());
        for recipe in recipes {
            let vertex = stages.next().context("missing native vertex stage")?;
            let fragment = stages.next().context("missing native fragment stage")?;
            let pipeline = Pipeline {
                vertex: vertex.source,
                vertex_entry: vertex.entry,
                fragment: fragment.source,
                fragment_entry: fragment.entry,
                stride: recipe.stride,
                attributes: recipe.attributes,
                uniform_bytes: recipe.uniform_bytes,
                textures: recipe.textures,
                blend: recipe.blend,
                cull: recipe.cull,
                depth_write: recipe.depth_write,
                depth_compare: recipe.depth_compare,
                raster: recipe.raster,
                uniform_policy: recipe.uniform_policy,
            };
            pipeline.validate_layout()?;
            pipelines.push(pipeline);
        }
        Ok(Self {
            source_kit: sources.identity().into(),
            preparation,
            samples,
            pipelines,
        })
    }

    pub(crate) fn into_payload(self, sources: &CheckedSources) -> Result<PipelinePayload> {
        ensure!(
            self.source_kit == sources.identity(),
            "native pipelines belong to another source kit"
        );
        let encoded = serde_json::to_vec(&self.pipelines)?;
        ensure!(
            encoded.len() as u64 <= crate::archive::MAX_TOTAL,
            "compiled pipeline payload exceeds package budget"
        );
        let hashes = self
            .pipelines
            .iter()
            .map(|p| p.identity_with_samples(self.samples))
            .collect();
        Ok(PipelinePayload {
            encoded,
            hashes,
            samples: self.samples,
            preparation: vec![self.preparation],
        })
    }
}

pub(crate) struct PipelinePayload {
    pub encoded: Vec<u8>,
    pub hashes: Vec<String>,
    pub samples: u32,
    pub preparation: Vec<String>,
}
