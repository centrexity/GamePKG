//! A private compiler exchange, separate from installable source packages.
use anyhow::{bail, Context, Result};
use std::{fs, io::Write, path::Path};

pub fn compile(
    store: &Path,
    output: &Path,
    backend: &str,
    optimize: bool,
    approved_crates: bool,
) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!("the isolated compiler worker currently requires Linux");
    }
    match fs::symlink_metadata(output) {
        Ok(_) => bail!("compiler exchange output must not already exist"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let bundle = crate::store::Bundle::open(store)?;
    if bundle.games().len() != 1 {
        bail!("compiler exchange requires exactly one verified game");
    }
    let manifest = &bundle.games()[0];
    let work = tempfile::tempdir()?;
    bundle.install(work.path(), true)?;
    let mut settings = crate::settings::load()?;
    settings.shader_backend = backend.to_owned();
    settings.optimize_builds = optimize;
    if !approved_crates {
        settings.approved_crates.clear();
    }
    settings.validate()?;
    crate::build::compile_with_settings(work.path(), &manifest.id, &settings)?;
    let archive = crate::files::read_regular(
        &work.path().join(format!("{}.gamepkg", manifest.id)),
        256 * 1024 * 1024,
        false,
    )?;
    let identity = crate::verify::digest(&archive);
    let parent = output
        .parent()
        .context("compiler output needs a parent folder")?;
    fs::create_dir_all(parent)?;
    let stage = tempfile::tempdir_in(parent)?;
    let mut total = 0;
    for name in ["builds", "shader-builds"] {
        copy_outputs(
            &work.path().join(name),
            &stage.path().join(name),
            &mut total,
        )?;
    }
    let info = serde_json::json!({"format_version":1,"id":manifest.id,"package_digest":identity,"backend":backend,"compiler_host_version":env!("CARGO_PKG_VERSION")});
    let mut file = fs::File::create(stage.path().join("exchange.json"))?;
    file.write_all(&serde_json::to_vec(&info)?)?;
    file.sync_all()?;
    // Renaming only happens after every output has been bounded and copied.
    publish_output(stage.path(), output)?;
    Ok(())
}
fn copy_outputs(source: &Path, destination: &Path, total: &mut u64) -> Result<()> {
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            let name = entry.file_name();
            let text = name.to_str().context("invalid cache directory")?;
            if text.len() != 64 || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("unexpected compiler cache directory");
            }
            copy_outputs(&entry.path(), &destination.join(name), total)?;
        } else {
            let bytes = crate::files::read_regular(&entry.path(), 32 * 1024 * 1024, false)?;
            *total = total
                .checked_add(bytes.len() as u64)
                .context("compiler output size overflow")?;
            if *total > 256 * 1024 * 1024 {
                bail!("compiler exchange exceeds 256 MiB");
            }
            let mut file = fs::File::create(destination.join(entry.file_name()))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn publish_output(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
    // Both C strings live through the call; the kernel publishes without replacing any target.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error())
            .context("unable to publish compiler output without replacing an existing target");
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
fn publish_output(_: &Path, _: &Path) -> Result<()> {
    bail!("the isolated compiler worker currently requires Linux")
}
#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn publication_preserves_a_target_created_during_compilation() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("compiled");
        let target = root.path().join("output");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("cache"), b"compiled").unwrap();
        std::fs::create_dir(&target).unwrap();
        assert!(super::publish_output(&source, &target).is_err());
        assert!(source.join("cache").exists());
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
        std::fs::remove_dir(&target).unwrap();
        super::publish_output(&source, &target).unwrap();
        assert_eq!(std::fs::read(target.join("cache")).unwrap(), b"compiled");
    }
}
