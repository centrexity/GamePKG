//! Reviewed Cube 2 native profile. Host implementation, never a guest dependency.
#[cfg(target_os = "linux")]
pub mod build;
#[cfg(target_os = "linux")]
mod data;
#[cfg(target_os = "linux")]
mod discovery;
#[cfg(target_os = "linux")]
mod network;
pub mod protocol;
#[cfg(target_os = "linux")]
mod runner;
#[cfg(target_os = "linux")]
mod save;
#[cfg(target_os = "linux")]
pub mod session;
#[cfg(target_os = "linux")]
mod sources;

pub const PROFILE: &str = "cube2-r6889";
pub const MEMORY: u64 = 2 * 1024 * 1024 * 1024;
pub const LAUNCHER: &str =
    "pub fn game_tick(input: u32, state: u64) -> u64 { gamepkg::cube2_tick(input, state) }\n";

pub(crate) fn save_path(
    root: &std::path::Path,
    id: &str,
    creator: &str,
    schema: u32,
) -> std::path::PathBuf {
    let scope = format!("GamePKG Cube 2 saved files v1\0{id}\0{creator}\0{schema}");
    root.join("saves")
        .join(format!("{}.cube2", crate::verify::digest(scope.as_bytes())))
}

pub fn cache_path(root: &std::path::Path, digest: &str) -> std::path::PathBuf {
    let fingerprint = crate::verify::digest(
        concat!(
            include_str!("../../engines/cube2/upstream-lock.json"),
            include_str!("../../engines/cube2/adapters/frame_bridge.h"),
            include_str!("../../engines/cube2/adapters/audio.h"),
            include_str!("../../engines/cube2/adapters/persistence.h"),
            include_str!("../../engines/cube2/adapters/network.h"),
            include_str!("../../engines/cube2/adapters/syscall_guard.h"),
            include_str!("build.rs")
        )
        .as_bytes(),
    );
    crate::native::path_for_digest(root, digest)
        .with_file_name(format!("cube2-{}", &fingerprint[..16]))
}

pub(crate) fn validate_recipe(package: &mut crate::archive::Package) -> anyhow::Result<()> {
    anyhow::ensure!(
        package.read("source/src/lib.rs")? == LAUNCHER.as_bytes(),
        "Cube 2 accepts only its reviewed engine launcher"
    );
    anyhow::ensure!(
        !package
            .entries
            .keys()
            .any(|name| name.starts_with("assets/") || name.starts_with("shaders/")),
        "Cube 2 media and shaders must come from the selected original installation"
    );
    for name in package
        .entries
        .keys()
        .filter(|name| name.starts_with("source/"))
    {
        anyhow::ensure!(
            matches!(
                name.as_str(),
                "source/Cargo.toml" | "source/Cargo.lock" | "source/src/lib.rs"
            ),
            "Cube 2 cannot add package-supplied engine or Rust source"
        );
    }
    let cargo: toml::Value =
        toml::from_str(std::str::from_utf8(&package.read("source/Cargo.toml")?)?)?;
    crate::dependencies::validate(&cargo, &package.read("source/Cargo.lock")?)
}
#[cfg(target_os = "linux")]
pub(crate) mod player;
