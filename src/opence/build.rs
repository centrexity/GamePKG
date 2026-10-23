//! Fetch the pinned OpenCE commit and cache its Linux binary.
use super::{
    cache_ready, classify, command, validate_i686, GameData, BINARY_LIMIT, CLONE_URL, COMMIT,
    LICENSE_SHA256, PROFILE, SDL_LIMIT,
};
use anyhow::{bail, ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

pub fn compile_package(root: &Path, id: &str) -> Result<()> {
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
        manifest.builtin_engine.as_deref() == Some(PROFILE),
        "not an OpenCE package"
    );
    super::validate_recipe(&mut package)?;
    let source = ensure_source(root)?;
    build_upstream(&source)?;
    let built = source.join("build/linux/halo");
    let sdl = source.join("build/linux/libSDL3.so.0");
    let binary = crate::files::read_regular(&built, BINARY_LIMIT, false)?;
    validate_i686(&binary)?;
    let sdl_bytes = crate::files::read_regular(&sdl, SDL_LIMIT, false)?;
    ensure!(
        crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false
        )? == original,
        "The installed package changed during the build"
    );
    let target = super::binary_path(root, &crate::verify::digest(&original));
    let directory = target.parent().context("invalid OpenCE build path")?;
    fs::create_dir_all(directory)?;
    write_file(&target, &binary)?;
    let receipt = target.with_extension("sha256");
    if receipt.symlink_metadata().is_ok() {
        fs::remove_file(&receipt)?;
    }
    crate::integrity::record_build(&target, &binary)?;
    let sdl_target = target.with_file_name("libSDL3.so.0");
    write_file(&sdl_target, &sdl_bytes)?;
    fs::write(
        target.with_file_name("libSDL3.so.0.sha256"),
        crate::verify::digest(&sdl_bytes),
    )?;
    ensure!(
        cache_ready(&target)?,
        "OpenCE cache failed its own receipt check"
    );
    Ok(())
}

pub fn launch(root: &Path, id: &str) -> Result<()> {
    let manifest = crate::registry::installed_manifest(root, id)?;
    ensure!(
        manifest.builtin_engine.as_deref() == Some(PROFILE),
        "not an OpenCE package"
    );
    let digest = {
        let bytes = crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false,
        )?;
        crate::verify::digest(&bytes)
    };
    let binary = super::binary_path(root, &digest);
    ensure!(
        cache_ready(&binary)?,
        "Build OpenCE before playing. The cached 32-bit engine is missing or failed its integrity check."
    );
    let selected = crate::game_data::load(root, &manifest)?
        .context("Choose the folder that contains your Halo maps or Xbox disc image")?;
    let data = classify(&selected)?;
    let runtime = root.join("runtime").join(format!(
        "opence-{}",
        &crate::verify::digest(format!("{}\0{}", manifest.id, manifest.creator_key).as_bytes())
            [..16]
    ));
    if runtime.exists() {
        let meta = fs::symlink_metadata(&runtime)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "OpenCE runtime directory must be a real directory"
        );
    }
    fs::create_dir_all(runtime.join("home"))?;
    fs::create_dir_all(runtime.join("saves"))?;
    install_runtime_file(&binary, &runtime.join("halo"))?;
    install_runtime_file(
        &binary.with_file_name("libSDL3.so.0"),
        &runtime.join("libSDL3.so.0"),
    )?;
    let runtime = fs::canonicalize(&runtime)?;
    let data_root = arrange_data(&runtime, &data)?;
    let log_path = runtime.join("halo.log");
    let log = fs::File::create(&log_path)?;
    let mut child = command(
        &runtime.join("halo"),
        &runtime,
        !manifest.permissions.network,
    );
    child
        .env("HOME", runtime.join("home"))
        .env("XDG_DATA_HOME", runtime.join("home"))
        .env("XDG_CONFIG_HOME", runtime.join("home"))
        .env("HALO_UPDATE_AUTO", "false")
        .env("HALO_SAVE_ROOT", runtime.join("saves"));
    if let Some(root) = &data_root {
        child.env("HALO_DATA_ROOT", root);
    }
    child
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    if let Some(value) = std::env::var_os("SDL_AUDIODRIVER") {
        child.env("SDL_AUDIODRIVER", value);
    } else if !super::alsa_available() {
        eprintln!("32-bit ALSA was not found. OpenCE will start without sound (SDL dummy audio).");
        child.env("SDL_AUDIODRIVER", "dummy");
    }
    for key in [
        "SDL_VIDEODRIVER",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "LANG",
        "LC_ALL",
        "HALO_HIDDEN_WINDOW",
        "HALO_NULL_RENDERER",
        "HALO_EXIT_AFTER",
    ] {
        if let Some(value) = std::env::var_os(key) {
            child.env(key, value);
        }
    }
    let mut progress = crate::progress::Session::open(root, &manifest.id, &manifest.creator_key)?;
    progress.start();
    let started = Instant::now();
    let waited = child
        .spawn()
        .context("start the OpenCE engine")
        .and_then(|mut child| child.wait().context("wait for the OpenCE engine"));
    progress.finish()?;
    let status = waited?;
    if !status.success() {
        let mut note = String::new();
        if let Ok(file) = fs::File::open(&log_path) {
            let mut bytes = Vec::new();
            file.take(4000).read_to_end(&mut bytes)?;
            note = String::from_utf8_lossy(&bytes).trim().to_owned();
        }
        bail!(
            "OpenCE exited with {status} after {} ms. {}",
            started.elapsed().as_millis(),
            if note.is_empty() {
                format!("See {}", log_path.display())
            } else {
                note
            }
        );
    }
    Ok(())
}

fn ensure_source(root: &Path) -> Result<PathBuf> {
    let source = std::env::var_os("GAMEPKG_OPENCE_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("engine-sources").join(format!("opence-{COMMIT}")));
    if source.join(".git").exists() {
        let head = git(&source, &["rev-parse", "HEAD"])?;
        ensure!(
            head.trim() == COMMIT,
            "OpenCE checkout {} is {}, not pinned {COMMIT}",
            source.display(),
            head.trim()
        );
        verify_tree(&source)?;
        return Ok(source);
    }
    if source.exists() {
        bail!(
            "remove the incomplete OpenCE directory {} or set GAMEPKG_OPENCE_SOURCE to a pinned checkout",
            source.display()
        );
    }
    if let Some(parent) = source.parent() {
        fs::create_dir_all(parent)?;
    }
    let clone = Command::new("git")
        .args(["clone", "--no-tags", CLONE_URL])
        .arg(&source)
        .status()
        .context("git clone OpenCE")?;
    ensure!(clone.success(), "git clone of OpenCE failed");
    let checkout = Command::new("git")
        .arg("-C")
        .arg(&source)
        .args(["checkout", "--detach", COMMIT])
        .status()
        .context("git checkout pinned OpenCE")?;
    ensure!(
        checkout.success(),
        "could not check out the pinned OpenCE commit"
    );
    verify_tree(&source)?;
    Ok(source)
}

fn verify_tree(source: &Path) -> Result<()> {
    let license = fs::read(source.join("LICENSE")).context("OpenCE LICENSE missing")?;
    ensure!(
        crate::verify::digest(&license) == LICENSE_SHA256,
        "OpenCE LICENSE does not match the pinned digest"
    );
    ensure!(
        license
            .windows(b"GNU AFFERO GENERAL PUBLIC LICENSE".len())
            .any(|window| window == b"GNU AFFERO GENERAL PUBLIC LICENSE"),
        "OpenCE LICENSE is not the Affero GPL"
    );
    for relative in ["configure.py", "source", "port/linux"] {
        ensure!(
            source.join(relative).exists(),
            "OpenCE checkout is missing {relative}"
        );
    }
    let mut forbidden = Vec::new();
    visit(source, source, &mut forbidden)?;
    ensure!(
        forbidden.is_empty(),
        "OpenCE checkout contains game data that must stay out of the build tree: {}",
        forbidden.into_iter().take(8).collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

fn visit(root: &Path, dir: &Path, forbidden: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let lowered = name.to_string_lossy().to_ascii_lowercase();
        if lowered == "maps" || lowered == "custom_maps" {
            forbidden.push(format!("{relative}/"));
            continue;
        }
        if path.is_dir() {
            visit(root, &path, forbidden)?;
            continue;
        }
        if [".iso", ".xiso", ".map", ".wad", ".sfc", ".smc", ".rom"]
            .iter()
            .any(|suffix| lowered.ends_with(suffix))
        {
            forbidden.push(relative);
        }
    }
    Ok(())
}

fn build_upstream(source: &Path) -> Result<()> {
    let binary = source.join("build/linux/halo");
    let sdl = source.join("build/linux/libSDL3.so.0");
    if binary.is_file() && sdl.is_file() {
        let bytes = crate::files::read_regular(&binary, BINARY_LIMIT, false)?;
        if validate_i686(&bytes).is_ok() {
            return Ok(());
        }
    }
    for tool in ["python3", "ninja", "clang", "cmake", "pkg-config"] {
        ensure!(
            Command::new(tool).arg("--version").output().is_ok(),
            "OpenCE's Linux build needs {tool} on PATH"
        );
    }
    let configure = Command::new("python3")
        .args([
            "configure.py",
            "--release",
            "--portable",
            "--pgo=off",
            "--lto=off",
        ])
        .current_dir(source)
        .status()
        .context("configure OpenCE")?;
    ensure!(configure.success(), "OpenCE configure.py failed");
    let ninja = Command::new("ninja")
        .arg("linux")
        .current_dir(source)
        .status()
        .context("build OpenCE")?;
    ensure!(ninja.success(), "ninja linux failed");
    ensure!(binary.is_file(), "OpenCE did not produce build/linux/halo");
    Ok(())
}

fn git(source: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(args)
        .output()
        .context("git")?;
    ensure!(output.status.success(), "git {} failed", args.join(" "));
    Ok(String::from_utf8(output.stdout).context("git output was not UTF-8")?)
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.symlink_metadata().is_ok() {
        fs::remove_file(path)?;
    }
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// Point the engine at the player's files without copying them into git.
///
/// Maps stay on the player's disk. A symlink under the library runtime makes
/// OpenCE's log and `custom_maps` land beside that link. A disc image is
/// linked next to the executable with `paths.data` unset, so OpenCE extracts
/// `maps/` into this runtime directory instead of the folder that holds the disc.
fn arrange_data(runtime: &Path, data: &GameData) -> Result<Option<PathBuf>> {
    let label = match data {
        GameData::Maps(path) => format!("maps:{}", path.display()),
        GameData::Disc(_, disc) => format!("disc:{}", disc.display()),
    };
    let marker = runtime.join("selected");
    if fs::read_to_string(&marker).ok().as_deref() != Some(label.as_str()) {
        remove_path(&runtime.join("maps"))?;
        remove_path(&runtime.join("custom_maps"))?;
        remove_path(&runtime.join("content"))?;
        remove_disc_links(runtime)?;
        fs::write(&marker, &label)?;
    }
    match data {
        GameData::Maps(path) => {
            let content = runtime.join("content");
            fs::create_dir_all(&content)?;
            replace_symlink(&content.join("maps"), &path.join("maps"))?;
            Ok(Some(content))
        }
        GameData::Disc(_, disc) => {
            let name = disc.file_name().context("disc image has no name")?;
            replace_symlink(&runtime.join(name), disc)?;
            Ok(None)
        }
    }
}

fn replace_symlink(link: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_path(link)?;
    std::os::unix::fs::symlink(target, link)
        .with_context(|| format!("link {} to {}", link.display(), target.display()))?;
    Ok(())
}

fn remove_path(path: &Path) -> Result<()> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if meta.file_type().is_symlink() || meta.is_file() {
        fs::remove_file(path)?;
    } else if meta.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn remove_disc_links(runtime: &Path) -> Result<()> {
    for entry in fs::read_dir(runtime)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !(name.ends_with(".iso") || name.ends_with(".xiso")) {
            continue;
        }
        if entry.file_type()?.is_symlink() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn install_runtime_file(from: &Path, to: &Path) -> Result<()> {
    let bytes = crate::files::read_regular(from, BINARY_LIMIT, false)?;
    if to.is_file()
        && crate::files::read_regular(to, BINARY_LIMIT, false)
            .ok()
            .as_deref()
            == Some(bytes.as_slice())
    {
        fs::set_permissions(to, fs::Permissions::from_mode(0o755))?;
        return Ok(());
    }
    if to.symlink_metadata().is_ok() {
        fs::remove_file(to)?;
    }
    write_file(to, &bytes)
}
