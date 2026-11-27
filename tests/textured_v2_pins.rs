//! Maintainer-side provenance check for the pinned textured-v2 shader catalog.
#![cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
#[path = "support/adversarial.rs"]
#[allow(dead_code)]
mod fixture;

use gamepkg::{
    archive::Package,
    manifest::{Manifest, NativeGraphics},
    port_host::{
        admission::CheckedSources,
        graphics::{
            textured_v2_recipe, textured_v2_shaders, VerifiedPipelines, TEXTURED_V2_SHADER,
        },
    },
    settings::Settings,
};

#[test]
fn textured_v2_rejects_signed_shader_bytes_outside_the_host_pin() {
    let root = tempfile::tempdir().unwrap();
    let mut files = fixture::fixture("examples/minimal");
    let mut manifest = Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.native_services = true;
    manifest.native_graphics = Some(NativeGraphics::TexturedV2);
    manifest.shaders = textured_v2_shaders().to_vec();
    files.insert(
        "shaders/textured_v2.slang".into(),
        TEXTURED_V2_SHADER.as_bytes().to_vec(),
    );
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_textured_v2.rs").to_vec(),
    );

    let path = fixture::sign(&files, root.path());
    let mut package = Package::open(&path).unwrap();
    let sources = CheckedSources::verify(&mut package).unwrap();
    let mut wrong_pin = textured_v2_recipe();
    wrong_pin.source_hashes =
        Some(["0000000000000000000000000000000000000000000000000000000000000000"; 2]);

    let error = VerifiedPipelines::prepare(
        root.path(),
        &sources,
        &package,
        &Settings::default(),
        vec![wrong_pin],
        1,
    )
    .err()
    .expect("signed source must not override the reviewed textured-v2 pin");
    assert!(
        error
            .to_string()
            .contains("shader source differs from reviewed template"),
        "unexpected textured-v2 pin error: {error}"
    );
}
