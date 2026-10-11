//! A private compiler exchange, separate from installable source packages.
use anyhow::{bail, Context, Result};
use std::{fs, io::Write, path::Path};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Exchange {
    format_version: u32,
    id: String,
    package_digest: String,
    backend: String,
    compiler_host_version: String,
}
pub fn prepare(root: &Path, id: &str, output: &Path) -> Result<()> {
    let _library = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    let bytes = crate::files::read_regular(
        &root.join(format!("{id}.gamepkg")),
        256 * 1024 * 1024,
        false,
    )?;
    let snapshot = tempfile::tempdir()?;
    let package = snapshot.path().join("source.gamepkg");
    fs::write(&package, &bytes)?;
    let verified = crate::verify::verify_embedded(&mut crate::archive::Package::open(&package)?)?;
    if toml::to_string(&verified)? != toml::to_string(&manifest)? {
        bail!("installed package changed while preparing the compiler request");
    }
    crate::store::export(root, &[package], output)
}

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
    let info = Exchange {
        format_version: 1,
        id: manifest.id.clone(),
        package_digest: identity,
        backend: backend.to_owned(),
        compiler_host_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    let mut file = fs::File::create(stage.path().join("exchange.json"))?;
    file.write_all(&serde_json::to_vec(&info)?)?;
    file.sync_all()?;
    // Renaming only happens after every output has been bounded and copied.
    publish_output(stage.path(), output)?;
    Ok(())
}
fn copy_outputs(source: &Path, destination: &Path, total: &mut u64) -> Result<()> {
    copy_outputs_inner(source, destination, total, 0, &mut 0)
}
fn copy_outputs_inner(
    source: &Path,
    destination: &Path,
    total: &mut u64,
    depth: usize,
    entries: &mut usize,
) -> Result<()> {
    if depth > 1 {
        bail!("unexpected nested compiler output directory");
    }
    plain_directory(source)?;
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        *entries += 1;
        if *entries > 128 {
            bail!("too many compiler output entries");
        }

        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            let name = entry.file_name();
            let text = name.to_str().context("invalid cache directory")?;
            if text.len() != 64 || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
                bail!("unexpected compiler cache directory");
            }
            copy_outputs_inner(
                &entry.path(),
                &destination.join(name),
                total,
                depth + 1,
                entries,
            )?;
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

/// Accept only a private worker response for the currently installed signed source.
pub fn accept(root: &Path, id: &str, exchange: &Path) -> Result<()> {
    let _library = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    let _build = crate::build::BuildChange::acquire(root, id)?;
    let installed = root.join(format!("{id}.gamepkg"));
    let archive = crate::files::read_regular(&installed, 256 * 1024 * 1024, false)?;
    let settings = crate::settings::load()?;
    settings.validate()?;
    plain_directory(exchange)?;
    let info: Exchange = serde_json::from_slice(&crate::files::read_regular(
        &exchange.join("exchange.json"),
        4096,
        false,
    )?)?;
    if info.format_version != 1
        || info.id != id
        || info.package_digest != crate::verify::digest(&archive)
        || info.backend != settings.shader_backend
        || info.compiler_host_version != env!("CARGO_PKG_VERSION")
    {
        bail!("compiler exchange does not match the installed package, backend or host version");
    }
    // Copy once into a private snapshot, then validate the exact bytes to be published.
    let snapshot = tempfile::tempdir()?;
    let mut total = 0;
    for name in ["builds", "shader-builds"] {
        copy_outputs(
            &exchange.join(name),
            &snapshot.path().join(name),
            &mut total,
        )?;
    }
    let source_build = snapshot.path().join("builds").join(&info.package_digest);
    let wasm = source_build.join("game.wasm");
    let bytes = crate::files::read_regular(&wasm, 32 * 1024 * 1024, false)?;
    crate::integrity::verify_build(&wasm, &bytes)?;
    let receipt_name = format!("shaders-{}.json", settings.shader_backend);
    let shader_receipt =
        crate::files::read_regular(&source_build.join(&receipt_name), 64 * 1024, false)?;
    require_entries(
        &snapshot.path().join("builds"),
        std::slice::from_ref(&info.package_digest),
    )?;
    require_entries(
        &source_build,
        &[
            "game.wasm".into(),
            "game.sha256".into(),
            receipt_name.clone(),
        ],
    )?;
    let component = bytes.starts_with(b"\0asm\x0d\0\x01\0");
    if component != (manifest.runtime_profile == "component-v1")
        || !crate::build::artifact_has_tick(&bytes)
    {
        bail!("compiler exchange has no matching playable entry point");
    }
    let (source_cache, _) = crate::shaders::playback_cache(
        snapshot.path(),
        &archive,
        &settings,
        manifest.shaders.len().max(1),
    )?;
    require_entries(
        &snapshot.path().join("shader-builds"),
        &[source_cache
            .file_name()
            .unwrap()
            .to_str()
            .context("invalid cache name")?
            .to_owned()],
    )?;
    let mut shader_names = vec!["manifest.toml".to_owned()];
    for index in 0..manifest.shaders.len().max(1) {
        shader_names.push(format!("{index}.shader"));
        shader_names.push(format!("{index}.reflection.json"));
    }
    require_entries(&source_cache, &shader_names)?;
    crate::shaders::load_fragment_sources(snapshot.path(), &archive, &manifest, &settings)?;
    let shader_manifest =
        crate::files::read_regular(&source_cache.join("manifest.toml"), 64 * 1024, false)?;
    if shader_manifest != toml::to_string(&manifest)?.as_bytes() {
        bail!("compiler exchange shader manifest differs from the installed signed manifest");
    }
    if crate::verify::digest(&crate::files::read_regular(
        &installed,
        256 * 1024 * 1024,
        false,
    )?) != info.package_digest
    {
        bail!("installed package changed during compiler exchange validation");
    }
    let builds = root.join("builds");
    prepare_directory(&builds)?;
    let destination = builds.join(&info.package_digest);
    prepare_directory(&destination)?;
    let shader_root = root.join("shader-builds");
    prepare_directory(&shader_root)?;
    let cache = shader_root.join(source_cache.file_name().context("invalid shader cache")?);
    if cache.try_exists()? {
        plain_directory(&cache)?;
        for entry in fs::read_dir(&source_cache)? {
            let entry = entry?;
            let expected = crate::files::read_regular(&entry.path(), 16 * 1024 * 1024, false)?;
            if crate::files::read_regular(&cache.join(entry.file_name()), 16 * 1024 * 1024, false)?
                != expected
            {
                bail!("existing shader cache is damaged; no compiler exchange published");
            }
        }
    } else {
        let publication = tempfile::tempdir_in(&shader_root)?;
        let staged_cache = publication.path().join("cache");
        let mut copied = 0;
        copy_outputs(&source_cache, &staged_cache, &mut copied)?;
        fs::rename(staged_cache, &cache)?;
    }
    publish_file(&destination.join(receipt_name), &shader_receipt)?;
    publish_file(&destination.join("game.wasm"), &bytes)?;
    crate::integrity::record_build(&destination.join("game.wasm"), &bytes)?;
    Ok(())
}
fn plain_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!("compiler exchange folders must be real directories");
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            bail!("compiler exchange folders must not be Windows reparse points");
        }
    }
    Ok(())
}
fn prepare_directory(path: &Path) -> Result<()> {
    if !path.try_exists()? {
        fs::create_dir(path)?;
    }
    plain_directory(path)
}
fn publish_file(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut output =
        tempfile::NamedTempFile::new_in(path.parent().context("invalid compiler cache path")?)?;
    output.write_all(bytes)?;
    output.as_file().sync_all()?;
    output.persist(path)?;
    Ok(())
}

fn require_entries(path: &Path, expected: &[String]) -> Result<()> {
    let expected: std::collections::BTreeSet<std::ffi::OsString> =
        expected.iter().map(std::ffi::OsString::from).collect();
    let mut actual = std::collections::BTreeSet::new();
    for entry in fs::read_dir(path)? {
        actual.insert(entry?.file_name());
        if actual.len() > expected.len() {
            bail!("unexpected compiler output files");
        }
    }
    if actual != expected {
        bail!("unexpected or missing compiler output files");
    }
    Ok(())
}
