//! Native compilation of the embedded reviewed engine; no player Python tooling.
use anyhow::{bail, ensure, Context, Result};
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt, path::Path};

#[derive(serde::Deserialize)]
struct Lock {
    revision: u32,
    files: BTreeMap<String, String>,
}
fn patch_once(text: &mut String, before: &str, after: &str) -> Result<()> {
    ensure!(
        text.matches(before).count() == 1,
        "Cube 2 adaptation anchor changed"
    );
    *text = text.replacen(before, after, 1);
    Ok(())
}
pub fn verify_sources() -> Result<()> {
    let lock: Lock = serde_json::from_str(include_str!("../../engines/cube2/upstream-lock.json"))?;
    ensure!(lock.revision == 6889, "unsupported Cube 2 source revision");
    ensure!(
        lock.files.len() == super::sources::FILES.len(),
        "Cube 2 source set changed"
    );
    for &(name, bytes) in super::sources::FILES {
        ensure!(
            crate::archive::valid_path(name)
                && lock.files.get(name) == Some(&crate::verify::digest(bytes)),
            "Cube 2 source integrity failed: {name}"
        );
    }
    Ok(())
}

/// Build only the embedded source kit inside compiler namespaces and limits.
/// Runtime registration, data selection and cache receipts are separate gates.
pub fn compile() -> Result<Vec<u8>> {
    compile_with_options(true)
}
pub fn compile_with_options(optimize: bool) -> Result<Vec<u8>> {
    verify_sources()?;
    let stage = tempfile::tempdir()?;
    for &(name, bytes) in super::sources::FILES {
        let path = stage.path().join(name);
        fs::create_dir_all(path.parent().context("invalid Cube 2 source path")?)?;
        fs::write(path, bytes)?;
    }
    let script = stage.path().join("enet/check_cflags.sh");
    fs::set_permissions(script, fs::Permissions::from_mode(0o700))?;
    fs::write(
        stage.path().join("engine/gamepkg_bridge.h"),
        include_bytes!("../../engines/cube2/adapters/frame_bridge.h"),
    )?;
    fs::write(
        stage.path().join("engine/syscall_guard.h"),
        include_bytes!("../../engines/cube2/adapters/syscall_guard.h"),
    )?;
    fs::write(
        stage.path().join("engine/persistence.h"),
        include_bytes!("../../engines/cube2/adapters/persistence.h"),
    )?;
    let main = stage.path().join("engine/main.cpp");
    fs::write(
        stage.path().join("enet/gamepkg_network.h"),
        include_bytes!("../../engines/cube2/adapters/network.h"),
    )?;
    let unix = stage.path().join("enet/unix.c");
    let text = fs::read_to_string(&unix)?;
    let anchor = "int\nenet_address_set_host_ip (ENetAddress * address, const char * name)\n{";
    ensure!(
        text.matches(anchor).count() == 1,
        "Cube 2 ENet adaptation anchor changed"
    );
    let prefix = text
        .split_once(anchor)
        .context("missing ENet transport anchor")?
        .0;
    fs::write(
        unix,
        format!("{prefix}#include \"gamepkg_network.h\"\n#endif\n"),
    )?;
    let mut text = fs::read_to_string(&main)?;
    for (before, after) in [
        (
            "void quit()                     // normal exit\n{",
            "void quit()                     // normal exit\n{\n    gamepkg_quit();",
        ),
        (
            "#include \"engine.h\"",
            "#include \"engine.h\"\n#include \"gamepkg_bridge.h\"",
        ),
        (
            "int main(int argc, char **argv)\n{",
            "int main(int argc, char **argv)\n{\n    gamepkg_prepare();",
        ),
        (
            "        swapbuffers();\n        renderedframe = inbetweenframes = true;",
            "        gamepkg_present();\n        swapbuffers();\n        renderedframe = inbetweenframes = true;",
        ),
        (
            "    for(;;)\n    {\n        static int frames = 0;",
            "    for(;;)\n    {\n        gamepkg_input();\n        static int frames = 0;",
        ),
    ] {
        patch_once(&mut text, before, after)?;
    }
    fs::write(main, text)?;
    let tools = stage.path().join("shared/tools.h");
    let mut text = fs::read_to_string(&tools)?;
    patch_once(&mut text,
        "inline void *operator new(size_t, void *p) { return p; }\ninline void *operator new[](size_t, void *p) { return p; }\ninline void operator delete(void *, void *) {}\ninline void operator delete[](void *, void *) {}",
        "#include <new>")?;
    fs::write(tools, text)?;
    let sound = stage.path().join("engine/sound.cpp");
    let mut text = fs::read_to_string(&sound)?;
    patch_once(
        &mut text,
        "Mix_OpenAudio(soundfreq, MIX_DEFAULT_FORMAT, 2, soundbufferlen)",
        "Mix_OpenAudio(48000, AUDIO_S16SYS, 2, soundbufferlen)",
    )?;
    patch_once(
        &mut text,
        "Mix_AllocateChannels(soundchans);",
        "extern void gamepkg_audio_hook(); gamepkg_audio_hook(); Mix_AllocateChannels(soundchans);",
    )?;
    fs::write(sound, text)?;
    let mut command = crate::compiler::command(128 * 1048576);
    command.args([
        "bwrap",
        "--unshare-all",
        "--die-with-parent",
        "--new-session",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
    ]);
    for path in ["/lib", "/lib64"] {
        if Path::new(path).exists() {
            command.args(["--ro-bind", path, path]);
        }
    }
    command.arg("--bind").arg(stage.path()).arg("/work").args([
        "--symlink",
        "usr/bin",
        "/bin",
        "--chdir",
        "/work",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
        "--setenv",
        "PATH",
        "/usr/bin:/bin",
        "--",
        "/usr/bin/make",
        "-j2",
        "client",
        "CC=/usr/bin/gcc",
        "CXX=/usr/bin/g++",
        "AR=/usr/bin/ar",
    ]);
    let cpu = if matches!(std::env::consts::ARCH, "arm" | "aarch64") {
        "-mcpu=native"
    } else {
        "-march=native"
    };
    command.arg(if optimize {
        format!("CXXFLAGS=-O3 {cpu} -fomit-frame-pointer -ffast-math -pthread")
    } else {
        "CXXFLAGS=-O0 -g0 -pthread".into()
    });
    let output = crate::compiler::run(&mut command)?;
    if !output.status.success() {
        bail!(
            "isolated Cube 2 build failed: {}\n{}",
            output.stdout,
            output.stderr
        );
    }
    let binary =
        crate::files::read_regular(&stage.path().join("sauer_client"), 32 * 1048576, false)?;
    ensure!(
        binary.starts_with(b"\x7fELF"),
        "Cube 2 compiler returned an invalid native binary"
    );
    Ok(binary)
}
/// Build an authenticated package once; playback verifies the cached receipt.
pub fn compile_package(root: &Path, id: &str) -> Result<()> {
    use std::io::Write;
    let _guard = crate::build::BuildChange::acquire(root, id)?;
    let original = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        crate::archive::MAX_TOTAL,
        false,
    )?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&original)?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    ensure!(
        manifest.builtin_engine.as_deref() == Some(super::PROFILE),
        "not a Cube 2 package"
    );
    super::validate_recipe(&mut package)?;
    let settings = crate::settings::load()?;
    let bytes = compile_with_options(settings.optimize_builds)?;
    crate::native::validate_executable(&bytes)?;
    ensure!(
        crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false
        )? == original,
        "The installed package changed during the build"
    );
    let target = super::cache_path(root, &crate::verify::digest(&original));
    fs::create_dir_all(target.parent().context("invalid Cube 2 build path")?)?;
    let mut file = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(&target).map_err(|e| e.error)?;
    crate::integrity::record_build(&target, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_source_set_matches_pinned_revision() {
        verify_sources().unwrap();
    }
    #[test]
    fn adaptations_fail_closed_when_upstream_anchors_change() {
        for source in ["missing", "before before"] {
            assert!(patch_once(&mut source.to_owned(), "before", "after").is_err());
        }
        let mut source = "before".to_owned();
        patch_once(&mut source, "before", "after").unwrap();
        assert_eq!(source, "after");
    }
    #[test]
    #[ignore = "requires C++/SDL development libraries and compiler namespaces"]
    fn compile_native_engine_without_python_or_network() {
        let bytes = compile().unwrap();
        assert!(bytes.len() > 100_000);
        assert!(bytes.starts_with(b"\x7fELF"));
    }
}
