//! Fetch a pinned external engine and cache one of its reviewed targets.
use super::{cache_ready, cached_binary, select, validate_image, Engine, Target, BINARY_LIMIT};
use crate::game_data::DataMatch;
use anyhow::{bail, ensure, Context, Result};
use std::{
    ffi::OsStr,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

pub fn compile_package(root: &Path, id: &str, requested: Option<&str>) -> Result<()> {
    let _guard = crate::build::BuildChange::acquire(root, id)?;
    let package_path = root.join(format!("{id}.gamepkg"));
    let original = crate::files::read_regular(&package_path, crate::archive::MAX_TOTAL, false)?;
    let mut snapshot = tempfile::NamedTempFile::new()?;
    snapshot.write_all(&original)?;
    let mut package = crate::archive::Package::open(snapshot.path())?;
    let manifest = crate::verify::verify_embedded(&mut package)?;
    let engine = manifest
        .builtin_engine
        .as_deref()
        .and_then(super::get)
        .context("not an external engine package")?;
    super::validate_recipe(engine, &mut package)?;
    let target = select(engine, requested)?;
    let source = ensure_source(root, engine)?;
    build_upstream(&source, target)?;
    let built = source.join(target.binary);
    let binary = crate::files::read_regular(&built, BINARY_LIMIT, false)?;
    validate_image(&target.image, &binary)?;
    let mut companions = Vec::new();
    for companion in target.companions {
        companions.push((
            companion,
            crate::files::read_regular(&source.join(companion.relative), companion.limit, false)?,
        ));
    }
    ensure!(
        crate::files::read_regular(&package_path, crate::archive::MAX_TOTAL, false)? == original,
        "The installed package changed during the build"
    );
    let digest = crate::verify::digest(&original);
    let base = root.join("builds").join(&digest).join(engine.id);
    let directory = base.join(target.id);
    fs::create_dir_all(&directory)?;
    let filename = Path::new(target.binary)
        .file_name()
        .context("engine target has no executable name")?;
    let output = directory.join(filename);
    write_file(&output, &binary)?;
    let receipt = output.with_extension("sha256");
    if receipt.symlink_metadata().is_ok() {
        fs::remove_file(&receipt)?;
    }
    crate::integrity::record_build(&output, &binary)?;
    for (companion, bytes) in companions {
        let name = Path::new(companion.relative)
            .file_name()
            .context("engine companion has no file name")?;
        let path = directory.join(name);
        write_file(&path, &bytes)?;
        write_companion_receipt(&path, &bytes)?;
    }
    write_current(&base, target.id)?;
    ensure!(
        cache_ready(&output, engine)?,
        "engine cache failed its own receipt check"
    );
    Ok(())
}

pub fn launch(root: &Path, id: &str) -> Result<()> {
    let manifest = crate::registry::installed_manifest(root, id)?;
    let engine = manifest
        .builtin_engine
        .as_deref()
        .and_then(super::get)
        .context("not an external engine package")?;
    let digest = {
        let bytes = crate::files::read_regular(
            &root.join(format!("{id}.gamepkg")),
            crate::archive::MAX_TOTAL,
            false,
        )?;
        crate::verify::digest(&bytes)
    };
    let binary = cached_binary(root, &digest, engine);
    ensure!(
        cache_ready(&binary, engine)?,
        "Build this game before playing. The cached engine is missing or failed its integrity check."
    );
    let target_id = binary
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let target = engine
        .targets
        .iter()
        .find(|target| target.id == target_id)
        .context("cached engine target is not in the catalog")?;
    ensure!(
        target.os == std::env::consts::OS,
        "the cached engine target {} is built for {}, and this host is {}",
        target.id,
        target.os,
        std::env::consts::OS
    );
    let selected = crate::game_data::load(root, &manifest)?
        .context("Choose the folder that matches this game's data layout")?;
    let data = crate::game_data::matches(
        &selected,
        &manifest
            .game_data
            .as_ref()
            .context("this engine requires game data")?
            .layouts,
    )?;
    let runtime = root.join("runtime").join(format!(
        "{}-{}",
        engine.id,
        &crate::verify::digest(format!("{}\0{}", manifest.id, manifest.creator_key).as_bytes())
            [..16]
    ));
    if runtime.exists() {
        let meta = fs::symlink_metadata(&runtime)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "engine runtime directory must be a real directory"
        );
    }
    fs::create_dir_all(runtime.join("home"))?;
    fs::create_dir_all(runtime.join("saves"))?;
    install_runtime_file(
        &binary,
        &runtime.join(binary.file_name().unwrap()),
        BINARY_LIMIT,
    )?;
    for companion in target.companions {
        let name = Path::new(companion.relative).file_name().unwrap();
        install_runtime_file(
            &binary.with_file_name(name),
            &runtime.join(name),
            companion.limit,
        )?;
    }
    let runtime = fs::canonicalize(&runtime)?;
    let executable = runtime.join(binary.file_name().unwrap());
    let data_root = arrange_data(&runtime, engine, &data, executable.file_name())?;
    let log_path = runtime.join("engine.log");
    let log = fs::File::create(&log_path)?;
    let mut child = command(&executable, &runtime, !manifest.permissions.network);
    if target.os == "windows" {
        child.env("USERPROFILE", runtime.join("home"));
    } else {
        child
            .env("HOME", runtime.join("home"))
            .env("XDG_DATA_HOME", runtime.join("home"))
            .env("XDG_CONFIG_HOME", runtime.join("home"));
    }
    for (key, value) in engine.launch.fixed {
        child.env(key, value);
    }
    if let (Some(key), Some(root)) = (engine.launch.save_root, Some(runtime.join("saves"))) {
        child.env(key, root);
    }
    if let (Some(key), Some(root)) = (engine.launch.data_root, data_root) {
        child.env(key, root);
    }
    if let Some(audio) = &target.audio {
        if std::env::var_os(audio.variable).is_none()
            && !audio.libraries.iter().any(|path| Path::new(path).exists())
        {
            eprintln!("{}", audio.warning);
            child.env(audio.variable, audio.value);
        }
    }
    for key in engine.launch.passthrough {
        if let Some(value) = std::env::var_os(key) {
            child.env(key, value);
        }
    }
    child
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    let mut progress = crate::progress::Session::open(root, &manifest.id, &manifest.creator_key)?;
    progress.start();
    let started = Instant::now();
    let waited = child
        .spawn()
        .context("start the external engine")
        .and_then(|mut child| child.wait().context("wait for the external engine"));
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
            "the engine exited with {status} after {} ms. {}",
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

fn ensure_source(root: &Path, engine: &Engine) -> Result<PathBuf> {
    let source = std::env::var_os("GAMEPKG_ENGINE_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("engine-sources").join(engine.id));
    if source.join(".git").exists() {
        let head = git(&source, &["rev-parse", "HEAD"])?;
        ensure!(
            head.trim() == engine.commit,
            "engine checkout {} is {}, not pinned {}",
            source.display(),
            head.trim(),
            engine.commit
        );
        verify_tree(&source, engine)?;
        return Ok(source);
    }
    if source.exists() {
        bail!(
            "remove the incomplete engine directory {} or set GAMEPKG_ENGINE_SOURCE to a pinned checkout",
            source.display()
        );
    }
    if let Some(parent) = source.parent() {
        fs::create_dir_all(parent)?;
    }
    let clone = Command::new("git")
        .args(["clone", "--no-tags", engine.clone_url])
        .arg(&source)
        .status()
        .context("git clone external engine")?;
    ensure!(clone.success(), "git clone of {} failed", engine.id);
    let checkout = Command::new("git")
        .arg("-C")
        .arg(&source)
        .args(["checkout", "--detach", engine.commit])
        .status()
        .context("git checkout pinned engine")?;
    ensure!(
        checkout.success(),
        "could not check out the pinned engine commit"
    );
    verify_tree(&source, engine)?;
    Ok(source)
}

fn verify_tree(source: &Path, engine: &Engine) -> Result<()> {
    let license = fs::read(source.join("LICENSE")).context("engine LICENSE missing")?;
    ensure!(
        crate::verify::digest(&license) == engine.license_sha256,
        "engine LICENSE does not match the pinned digest"
    );
    let marker = engine.license_marker.as_bytes();
    ensure!(
        license.windows(marker.len()).any(|window| window == marker),
        "engine LICENSE is missing its required license marker"
    );
    for relative in engine.required_paths {
        ensure!(
            source.join(relative).exists(),
            "engine checkout is missing {relative}"
        );
    }
    let mut forbidden = Vec::new();
    visit(source, source, engine, &mut forbidden)?;
    ensure!(
        forbidden.is_empty(),
        "engine checkout contains game data that must stay out of the build tree: {}",
        forbidden.into_iter().take(8).collect::<Vec<_>>().join(", ")
    );
    Ok(())
}

fn visit(root: &Path, dir: &Path, engine: &Engine, forbidden: &mut Vec<String>) -> Result<()> {
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
        if engine
            .forbidden_directories
            .iter()
            .any(|dir| lowered == *dir)
        {
            forbidden.push(format!("{relative}/"));
            continue;
        }
        if path.is_dir() {
            visit(root, &path, engine, forbidden)?;
            continue;
        }
        if engine
            .forbidden_suffixes
            .iter()
            .any(|suffix| lowered.ends_with(suffix))
        {
            forbidden.push(relative);
        }
    }
    Ok(())
}

fn build_upstream(source: &Path, target: &Target) -> Result<()> {
    let binary = source.join(target.binary);
    if binary.is_file()
        && target
            .companions
            .iter()
            .all(|companion| source.join(companion.relative).is_file())
    {
        if let Ok(bytes) = crate::files::read_regular(&binary, BINARY_LIMIT, false) {
            if validate_image(&target.image, &bytes).is_ok() {
                return Ok(());
            }
        }
    }
    for tool in target.tools {
        ensure!(
            Command::new(tool).arg("--version").output().is_ok(),
            "this engine target needs {tool} on PATH"
        );
    }
    let configure = expand(target.configure)?;
    ensure!(
        !configure.is_empty(),
        "this engine target has no configure command"
    );
    let mut command = Command::new(&configure[0]);
    command.args(&configure[1..]).current_dir(source);
    let status = command
        .status()
        .with_context(|| format!("configure {} with {}", target.id, configure[0]))?;
    ensure!(status.success(), "configure for {} failed", target.id);
    let build = Command::new(target.build_program)
        .args(target.build_args)
        .current_dir(source)
        .status()
        .with_context(|| format!("build {}", target.id))?;
    ensure!(build.success(), "build for {} failed", target.id);
    ensure!(
        binary.is_file(),
        "the build did not produce {}",
        target.binary
    );
    Ok(())
}

fn expand(args: &[&str]) -> Result<Vec<String>> {
    args.iter()
        .map(|arg| {
            let mut value = (*arg).to_owned();
            for (token, env) in [
                ("{vctools}", "GAMEPKG_MSVC_TOOLS"),
                ("{winsdk}", "GAMEPKG_WINDOWS_SDK"),
            ] {
                if !value.contains(token) {
                    continue;
                }
                let replacement = std::env::var(env).with_context(|| {
                    format!("{env} must point at the toolchain directory this engine target compiles against")
                })?;
                ensure!(
                    !replacement.is_empty() && !replacement.chars().any(char::is_control),
                    "{env} is empty or invalid"
                );
                value = value.replace(token, &replacement);
            }
            Ok(value)
        })
        .collect()
}

fn git(source: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(args)
        .output()
        .context("git")?;
    ensure!(output.status.success(), "git {} failed", args.join(" "));
    String::from_utf8(output.stdout).context("git output was not UTF-8")
}

fn command(binary: &Path, runtime: &Path, isolate_network: bool) -> Command {
    if isolate_network
        && cfg!(target_os = "linux")
        && Command::new("bwrap").arg("--version").output().is_ok()
    {
        let mut command = Command::new("bwrap");
        command
            .arg("--unshare-net")
            .arg("--die-with-parent")
            .arg("--ro-bind")
            .arg("/")
            .arg("/")
            .arg("--dev-bind")
            .arg("/dev")
            .arg("/dev")
            .arg("--proc")
            .arg("/proc")
            .arg("--bind")
            .arg(runtime)
            .arg(runtime);
        if Path::new("/dev/shm").is_dir() {
            command.arg("--bind").arg("/dev/shm").arg("/dev/shm");
        }
        if Path::new("/tmp/.X11-unix").is_dir() {
            command
                .arg("--bind")
                .arg("/tmp/.X11-unix")
                .arg("/tmp/.X11-unix");
        }
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            if Path::new(&dir).is_dir() {
                command.arg("--bind").arg(&dir).arg(&dir);
            }
        }
        command.arg("--chdir").arg(runtime).arg("--").arg(binary);
        return command;
    }
    if isolate_network && cfg!(target_os = "linux") {
        eprintln!(
            "Network isolation needs bubblewrap; the engine process keeps this user's network."
        );
    }
    let mut command = Command::new(binary);
    command.current_dir(runtime);
    command
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
    set_executable(path)?;
    Ok(())
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn write_companion_receipt(path: &Path, bytes: &[u8]) -> Result<()> {
    let name = path.file_name().context("companion has no file name")?;
    let receipt = path.with_file_name(format!("{}.sha256", name.to_string_lossy()));
    if receipt.symlink_metadata().is_ok() {
        fs::remove_file(&receipt)?;
    }
    fs::write(receipt, crate::verify::digest(bytes))?;
    Ok(())
}

fn write_current(directory: &Path, id: &str) -> Result<()> {
    fs::create_dir_all(directory)?;
    let path = directory.join("current");
    if path.symlink_metadata().is_ok() {
        fs::remove_file(&path)?;
    }
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(id.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn arrange_data(
    runtime: &Path,
    engine: &Engine,
    data: &DataMatch,
    executable: Option<&OsStr>,
) -> Result<Option<PathBuf>> {
    let label = match data {
        DataMatch::Link { root, .. } => format!("link:{}", root.display()),
        DataMatch::Beside { file } => format!("beside:{}", file.display()),
    };
    let marker = runtime.join("selected");
    if fs::read_to_string(&marker).ok().as_deref() != Some(label.as_str()) {
        for name in engine.cleanup {
            remove_path(&runtime.join(name))?;
        }
        remove_archive_links(runtime, engine)?;
        fs::write(&marker, &label)?;
    }
    match data {
        DataMatch::Link { root, directories } => {
            let content = runtime.join("content");
            fs::create_dir_all(&content)?;
            for directory in directories {
                replace_link(&content.join(directory), &root.join(directory))?;
            }
            Ok(Some(content))
        }
        DataMatch::Beside { file } => {
            let name = file.file_name().context("game data file has no name")?;
            ensure!(
                Some(name) != executable,
                "game data file name collides with the engine executable"
            );
            replace_link(&runtime.join(name), file)?;
            Ok(None)
        }
    }
}

fn replace_link(link: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_path(link)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
            .with_context(|| format!("link {} to {}", link.display(), target.display()))?;
    }
    #[cfg(windows)]
    {
        let metadata =
            fs::metadata(target).with_context(|| format!("stat {}", target.display()))?;
        let result = if metadata.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        result.with_context(|| format!("link {} to {}", link.display(), target.display()))?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (link, target);
        bail!("this operating system cannot expose game data without copying it");
    }
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

fn remove_archive_links(runtime: &Path, engine: &Engine) -> Result<()> {
    let extensions = engine
        .layout_specs
        .iter()
        .filter(|layout| layout.placement == "beside")
        .flat_map(|layout| layout.extensions.iter().copied())
        .collect::<Vec<_>>();
    for entry in fs::read_dir(runtime)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !extensions
            .iter()
            .any(|extension| name.ends_with(&format!(".{extension}")))
        {
            continue;
        }
        if entry.file_type()?.is_symlink() {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn install_runtime_file(from: &Path, to: &Path, limit: u64) -> Result<()> {
    let bytes = crate::files::read_regular(from, limit, false)?;
    if to.is_file()
        && crate::files::read_regular(to, limit, false).ok().as_deref() == Some(bytes.as_slice())
    {
        set_executable(to)?;
        return Ok(());
    }
    if to.symlink_metadata().is_ok() {
        fs::remove_file(to)?;
    }
    write_file(to, &bytes)
}
