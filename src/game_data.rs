//! Host-only remembered data folders, scoped to signed creator and declaration.
use crate::manifest::Manifest;
use anyhow::{bail, ensure, Context, Result};
use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
    path::{Component, Path, PathBuf},
};
pub(crate) fn identity(manifest: &Manifest) -> Result<String> {
    ensure!(
        manifest
            .game_data
            .as_ref()
            .is_some_and(|data| !data.layouts.is_empty()),
        "this game does not request a supported data folder"
    );
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        &manifest.game_data,
        &manifest.builtin_engine,
    ))?))
}
fn read(root: &Path) -> Result<BTreeMap<String, String>> {
    let path = root.join("game-data-selections.json");
    if !path.try_exists()? {
        return Ok(BTreeMap::new());
    }
    let entries: BTreeMap<String, String> =
        serde_json::from_slice(&crate::files::read_regular(&path, 1048576, false)?)?;
    ensure!(
        entries.len() <= 1024
            && entries.iter().all(|(k, v)| k.len() == 64
                && k.bytes().all(|b| b.is_ascii_hexdigit())
                && v.len() <= 4096
                && !v.chars().any(char::is_control)),
        "invalid remembered game-data selections"
    );
    Ok(entries)
}
pub fn load(root: &Path, manifest: &Manifest) -> Result<Option<PathBuf>> {
    if manifest.game_data.is_none() {
        return Ok(None);
    }
    Ok(read(root)?.remove(&identity(manifest)?).map(PathBuf::from))
}
/// Basic startup readiness, not a complete asset/version compatibility audit.
pub fn validate(path: &Path) -> Result<()> {
    ensure!(path.is_dir(), "selected game-data folder is unavailable");
    for name in ["data", "packages"] {
        let metadata = path
            .join(name)
            .symlink_metadata()
            .with_context(|| format!("game data is missing {name}"))?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "game data requires a real {name} folder"
        );
    }
    validate_parts(&path.join("data"), &path.join("packages"))
}
pub(crate) fn validate_parts(data: &Path, packages: &Path) -> Result<()> {
    let metadata = packages
        .join("textures")
        .symlink_metadata()
        .context("game data is missing packages/textures")?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "game data requires a real packages/textures folder"
    );
    for (path, name) in [
        (data.join("stdlib.cfg"), "data/stdlib.cfg"),
        (
            packages.join("textures/notexture.png"),
            "packages/textures/notexture.png",
        ),
    ] {
        let metadata = path
            .symlink_metadata()
            .with_context(|| format!("game data is missing {name}"))?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0,
            "game data requires a nonempty regular {name} file"
        );
    }
    Ok(())
}
pub fn available_for(manifest: &Manifest, path: &Path) -> bool {
    validate_for(manifest, path).is_ok()
}

pub fn validate_for(manifest: &Manifest, path: &Path) -> Result<()> {
    if let Some(data) = &manifest.game_data {
        if manifest.uses_service_runtime() {
            #[cfg(target_os = "linux")]
            return open_native_directories(data, path).map(|_| ());
            #[cfg(not(target_os = "linux"))]
            bail!("native game-data folders currently require Linux");
        }
        if !data.layouts.is_empty() {
            return matches(path, &data.layouts).map(|_| ());
        }
    }
    validate(path)
}

pub(crate) fn validate_native_layout(data: &crate::manifest::GameDataRequirement) -> Result<()> {
    let [layout] = data.layouts.as_slice() else {
        bail!("native game data requires exactly one folder layout");
    };
    validate_layouts(&data.layouts)?;
    ensure!(
        !data.title.trim().is_empty()
            && data.title.len() <= 256
            && !data.title.chars().any(char::is_control)
            && !data.user_selected_servers
            && layout.placement == "link"
            && layout.extensions.is_empty()
            && (1..=8).contains(&layout.directories.len()),
        "native game data requires one to eight read-only folders in selection order"
    );
    let maximum = layout.max_bytes.unwrap_or(8 * 1024 * 1024 * 1024);
    ensure!(
        layout.min_bytes.unwrap_or(0) <= maximum && maximum <= 8 * 1024 * 1024 * 1024,
        "native game-data file size bounds are invalid"
    );
    let mut names = HashSet::new();
    for directory in &layout.directories {
        ensure!(
            !directory.contains('/')
                && crate::archive::valid_path(directory)
                && names.insert(directory.to_lowercase()),
            "native game-data directory names must be distinct top-level paths"
        );
    }
    for required in &layout.paths {
        ensure!(
            crate::archive::valid_path(required)
                && required.split_once('/').is_some_and(|(top, _)| {
                    layout.directories.iter().any(|directory| directory == top)
                }),
            "required game-data files must be inside a declared directory"
        );
    }
    Ok(())
}

// Open the validated roots once. The launcher mounts the returned descriptors
// directly so a later filesystem rename cannot substitute a different root.
#[cfg(target_os = "linux")]
pub(crate) fn open_native_directories(
    data: &crate::manifest::GameDataRequirement,
    root: &Path,
) -> Result<Vec<cap_std::fs::Dir>> {
    use cap_fs_ext::{DirExt, MetadataExt};
    use cap_std::fs::Dir;

    validate_native_layout(data)?;
    ensure!(
        root.is_absolute(),
        "game-data selection needs an absolute path"
    );
    let mut selected = Dir::open_ambient_dir("/", cap_std::ambient_authority())?;
    for part in root.components().skip(1) {
        let Component::Normal(part) = part else {
            bail!("invalid game-data selection path");
        };
        selected = selected
            .open_dir_nofollow(part)
            .context("open selected game-data directory")?;
    }
    let layout = &data.layouts[0];
    let min = layout.min_bytes.unwrap_or(0);
    let max = layout.max_bytes.unwrap_or(8 * 1024 * 1024 * 1024);
    layout
        .directories
        .iter()
        .map(|name| {
            let directory = selected
                .open_dir_nofollow(name)
                .with_context(|| format!("the folder needs a real {name} directory"))?;
            for required in &layout.paths {
                let Some((top, relative)) = required.split_once('/') else {
                    continue;
                };
                if top != name {
                    continue;
                }
                let mut parent = directory.try_clone()?;
                let (parents, leaf) = relative.rsplit_once('/').unwrap_or(("", relative));
                for part in parents.split('/').filter(|part| !part.is_empty()) {
                    parent = parent
                        .open_dir_nofollow(part)
                        .with_context(|| format!("open required game-data path {required}"))?;
                }
                let metadata = parent
                    .symlink_metadata(leaf)
                    .with_context(|| format!("the folder needs {required}"))?;
                ensure!(
                    metadata.is_file()
                        && MetadataExt::nlink(&metadata) == 1
                        && (min..=max).contains(&metadata.len()),
                    "{required} must be an unaliased regular file within the declared size range"
                );
            }
            Ok(directory)
        })
        .collect()
}
pub fn validate_layouts(layouts: &[crate::manifest::DataLayout]) -> Result<()> {
    ensure!(
        !layouts.is_empty() && layouts.len() <= 8,
        "an external engine needs one to eight data layouts"
    );
    for layout in layouts {
        ensure!(
            matches!(layout.placement.as_str(), "link" | "beside"),
            "data layout placement must be link or beside"
        );
        ensure!(
            layout.directories.len() <= 16
                && layout.paths.len() <= 32
                && layout.extensions.len() <= 16,
            "data layout is too large"
        );
        for value in layout.directories.iter().chain(&layout.paths) {
            let path = Path::new(value);
            ensure!(
                !value.is_empty()
                    && value.len() <= 256
                    && !path.is_absolute()
                    && !value.contains('\\')
                    && !value
                        .split('/')
                        .any(|part| part.is_empty() || part == "." || part == ".."),
                "data layout paths must be relative and cannot contain .."
            );
        }
        for extension in &layout.extensions {
            ensure!(
                !extension.is_empty()
                    && extension.len() <= 8
                    && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()),
                "data layout extension must be short and alphanumeric"
            );
        }
        if let (Some(min), Some(max)) = (layout.min_bytes, layout.max_bytes) {
            ensure!(
                min <= max && max <= 8 * 1024 * 1024 * 1024,
                "data layout size bounds are invalid"
            );
        }
        match layout.placement.as_str() {
            "link" => ensure!(
                !layout.directories.is_empty() || !layout.paths.is_empty(),
                "a link layout needs directories or paths"
            ),
            "beside" => ensure!(
                !layout.extensions.is_empty() && layout.directories.is_empty(),
                "a beside layout names file extensions only"
            ),
            _ => unreachable!(),
        }
    }
    Ok(())
}

/// Player-selected data satisfying one reviewed layout. The first match wins.
#[derive(Debug)]
pub enum DataMatch {
    Link {
        root: PathBuf,
        directories: Vec<String>,
    },
    Beside {
        file: PathBuf,
    },
}

pub fn matches(path: &Path, layouts: &[crate::manifest::DataLayout]) -> Result<DataMatch> {
    let path = std::fs::canonicalize(path).context("choose the game-data file or folder")?;
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.is_dir() || metadata.is_file(),
        "game data must be a regular file or folder"
    );
    let mut notes = Vec::new();
    for layout in layouts {
        match try_layout(&path, layout) {
            Ok(found) => return Ok(found),
            Err(error) => notes.push(error.to_string()),
        }
    }
    bail!(
        "the selected file or folder does not match this game's data layout{}",
        if notes.is_empty() {
            String::new()
        } else {
            format!(": {}", notes.join("; "))
        }
    )
}

fn try_layout(root: &Path, layout: &crate::manifest::DataLayout) -> Result<DataMatch> {
    let min = layout.min_bytes.unwrap_or(0);
    let max = layout.max_bytes.unwrap_or(8 * 1024 * 1024 * 1024);
    match layout.placement.as_str() {
        "link" => {
            ensure!(root.is_dir(), "this layout needs a game-data folder");
            for directory in &layout.directories {
                let metadata = root
                    .join(directory)
                    .symlink_metadata()
                    .with_context(|| format!("the folder needs a real {directory} directory"))?;
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "{directory} must be a real directory, not a link"
                );
            }
            for relative in &layout.paths {
                let metadata = root
                    .join(relative)
                    .symlink_metadata()
                    .with_context(|| format!("the folder needs {relative}"))?;
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "{relative} must be a regular file"
                );
                ensure!(
                    (min..=max).contains(&metadata.len()),
                    "{relative} is outside the reviewed size range"
                );
            }
            for directory in &layout.directories {
                reject_escaping_links(&root.join(directory))?;
            }
            Ok(DataMatch::Link {
                root: root.to_path_buf(),
                directories: layout.directories.clone(),
            })
        }
        "beside" => {
            if root.is_file() {
                let extension = root
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                ensure!(
                    layout
                        .extensions
                        .iter()
                        .any(|expected| expected.eq_ignore_ascii_case(extension)),
                    "the selected game-data file has an unsupported extension"
                );
                let metadata = std::fs::metadata(root)?;
                ensure!(
                    (min..=max).contains(&metadata.len()),
                    "game data archive is outside the reviewed size range"
                );
                return Ok(DataMatch::Beside {
                    file: root.to_path_buf(),
                });
            }
            let mut files = Vec::new();
            for entry in std::fs::read_dir(root).context("read the game-data folder")? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                let extension = name.rsplit_once('.').map(|(_, extension)| extension);
                if !extension.is_some_and(|extension| {
                    layout
                        .extensions
                        .iter()
                        .any(|expected| expected == extension)
                }) {
                    continue;
                }
                let metadata = std::fs::symlink_metadata(entry.path())?;
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "game data archive must be a regular file, not a link"
                );
                ensure!(
                    (min..=max).contains(&metadata.len()),
                    "game data archive is outside the reviewed size range"
                );
                files.push(entry.path());
            }
            ensure!(
                files.len() == 1,
                "choose a folder with exactly one file matching this layout"
            );
            Ok(DataMatch::Beside {
                file: files.remove(0),
            })
        }
        other => bail!("unsupported data layout {other}"),
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A link whose target leaves the reviewed directory would let the engine read
/// or write something the player did not select. Links that stay inside are kept.
fn reject_escaping_links(directory: &Path) -> Result<()> {
    let base = normalize(directory);
    let mut pending = vec![(directory.to_path_buf(), 0u32)];
    let mut queued = HashSet::new();
    queued.insert(base.clone());
    let mut seen = 0u32;
    while let Some((dir, depth)) = pending.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", dir.display()));
            }
        };
        for entry in entries {
            let entry = entry?;
            seen += 1;
            ensure!(seen <= 100_000, "game data folder is too large to check");
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if !metadata.file_type().is_symlink() {
                if metadata.is_dir() {
                    let normalized = normalize(&path);
                    if queued.insert(normalized) {
                        pending.push((path, depth));
                    }
                }
                continue;
            }
            let target = std::fs::read_link(&path)?;
            let joined = if target.is_absolute() {
                target
            } else {
                path.parent().unwrap_or(Path::new("/")).join(target)
            };
            let normalized = normalize(&joined);
            ensure!(
                normalized.starts_with(&base),
                "{} is a link that leaves the game-data directory",
                path.strip_prefix(directory).unwrap_or(&path).display()
            );
            if std::fs::metadata(&path).is_ok_and(|meta| meta.is_dir()) && queued.insert(normalized)
            {
                ensure!(depth < 32, "game data links are nested too deeply");
                pending.push((path, depth + 1));
            }
        }
    }
    Ok(())
}

pub fn available(path: &Path) -> bool {
    validate(path).is_ok()
}
pub fn remember(root: &Path, manifest: &Manifest, path: &Path) -> Result<()> {
    let key = identity(manifest)?;
    let path =
        std::fs::canonicalize(path).context("choose the original game-data file or folder")?;
    let text = path
        .to_str()
        .context("game-data location must be representable as text")?;
    ensure!(
        text.len() <= 4096 && !text.chars().any(char::is_control),
        "invalid game-data location"
    );
    validate_for(manifest, &path)?;
    let mut entries = read(root)?;
    entries.insert(key, text.to_owned());
    ensure!(entries.len() <= 1024, "too many remembered games");
    let bytes = serde_json::to_vec_pretty(&entries)?;
    ensure!(
        bytes.len() <= 1048576,
        "remembered game-data locations exceed their quota"
    );
    if let Ok(metadata) = root.symlink_metadata() {
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "library must be a real directory"
        );
    }
    std::fs::create_dir_all(root)?;
    let mut staged = tempfile::NamedTempFile::new_in(root)?;
    staged.write_all(&bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist(root.join("game-data-selections.json"))
        .map_err(|e| e.error)?;
    Ok(())
}
pub fn remember_for_game(root: &Path, id: &str, path: &Path) -> Result<()> {
    let _change = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    remember(root, &manifest, path)
}

#[cfg(test)]
mod tests {
    const NATIVE_LAYOUT: &str = r#"
[game_data]
title = "Original installation"
[[game_data.layouts]]
directories = ["packages", "data"]
paths = ["packages/textures/default.png", "data/start.cfg"]
min_bytes = 1
max_bytes = 1024
"#;

    fn parse_native_layout(layout: &str) -> anyhow::Result<super::Manifest> {
        let base = include_str!("../examples/minimal/manifest.toml");
        super::Manifest::parse(format!("native_services = true\n{base}\n{layout}").as_bytes())
    }

    #[test]
    fn signed_native_layout_has_ordered_distinct_top_level_directories() {
        let manifest = parse_native_layout(NATIVE_LAYOUT).unwrap();
        assert_eq!(manifest.required_game_data_directory_count(), 2);
        for layout in [
            NATIVE_LAYOUT.replace(r#""packages", "data""#, r#""packages", "packages""#),
            NATIVE_LAYOUT.replace("packages/textures/default.png", "private/password.txt"),
            NATIVE_LAYOUT.replace(r#""packages", "data""#, r#""../packages", "data""#),
            NATIVE_LAYOUT.replace("max_bytes = 1024", "max_bytes = 8589934593"),
            NATIVE_LAYOUT.replace("min_bytes = 1", "min_bytes = 2048"),
        ] {
            assert!(parse_native_layout(&layout).is_err(), "{layout}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_selection_pins_only_the_declared_directories() {
        use std::{fs, io::Read, os::unix::fs::symlink};

        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        fs::create_dir(root.path().join("data")).unwrap();
        fs::write(
            root.path().join("packages/textures/default.png"),
            b"original",
        )
        .unwrap();
        fs::write(root.path().join("data/start.cfg"), b"config").unwrap();
        fs::write(root.path().join("private"), b"excluded").unwrap();
        let manifest = parse_native_layout(NATIVE_LAYOUT).unwrap();
        let data = manifest.game_data.as_ref().unwrap();
        let mounts = super::open_native_directories(data, root.path()).unwrap();
        for (directory, name, expected) in [
            (&mounts[0], "textures/default.png", b"original".as_slice()),
            (&mounts[1], "start.cfg", b"config".as_slice()),
        ] {
            let mut contents = Vec::new();
            directory
                .open(name)
                .unwrap()
                .read_to_end(&mut contents)
                .unwrap();
            assert_eq!(contents, expected);
            assert!(directory.open("private").is_err());
        }
        fs::rename(
            root.path().join("packages/textures"),
            root.path().join("moved"),
        )
        .unwrap();
        symlink("../moved", root.path().join("packages/textures")).unwrap();
        assert!(super::open_native_directories(data, root.path()).is_err());
        fs::rename(root.path().join("data"), root.path().join("moved-data")).unwrap();
        fs::create_dir(root.path().join("data")).unwrap();
        fs::write(root.path().join("data/start.cfg"), b"replacement").unwrap();
        let mut contents = String::new();
        mounts[1]
            .open("start.cfg")
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "config");
    }

    #[cfg(unix)]
    #[test]
    fn linked_core_files_and_texture_directory_are_not_ready() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        let target = root.path().join("original");
        std::fs::write(&target, b"original").unwrap();
        let names = ["data/stdlib.cfg", "packages/textures/notexture.png"];
        for name in names {
            std::fs::write(root.path().join(name), b"original").unwrap();
        }
        for name in names {
            let path = root.path().join(name);
            std::fs::remove_file(&path).unwrap();
            symlink(&target, &path).unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_file(&path).unwrap();
            std::fs::write(&path, b"original").unwrap();
        }
        let textures = root.path().join("packages/textures");
        let moved = root.path().join("original-textures");
        std::fs::rename(&textures, &moved).unwrap();
        symlink(&moved, &textures).unwrap();
        assert!(!super::available(root.path()));
        assert_eq!(std::fs::read(target).unwrap(), b"original");
    }
    #[test]
    fn incomplete_data_is_not_ready() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        std::fs::create_dir(root.path().join("packages")).unwrap();
        assert!(
            !super::available(root.path()),
            "Empty folders were marked ready to play"
        );
    }
    #[test]
    fn core_readiness_reports_each_missing_empty_or_nonregular_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        let names = ["data/stdlib.cfg", "packages/textures/notexture.png"];
        for name in names {
            std::fs::write(root.path().join(name), b"original").unwrap();
        }
        assert!(super::available(root.path()));
        for name in names {
            let path = root.path().join(name);
            std::fs::remove_file(&path).unwrap();
            assert!(super::validate(root.path())
                .unwrap_err()
                .to_string()
                .contains(name));
            std::fs::write(&path, b"").unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_dir(&path).unwrap();
            std::fs::write(&path, b"original").unwrap();
            assert!(super::available(root.path()));
        }
    }
}
