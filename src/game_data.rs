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
        manifest.game_data.as_ref().is_some_and(|data| {
            manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE)
                || !data.layouts.is_empty()
        }),
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
pub fn available_for(manifest: &Manifest, path: &Path) -> bool {
    if let Some(data) = &manifest.game_data {
        if !data.layouts.is_empty() {
            return matches(path, &data.layouts).is_ok();
        }
    }
    available(path)
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

/// A player folder that satisfied one reviewed layout. The first match wins.
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
    let path = std::fs::canonicalize(path).context("choose the game-data folder")?;
    ensure!(path.is_dir(), "game data must be a folder");
    let mut notes = Vec::new();
    for layout in layouts {
        match try_layout(&path, layout) {
            Ok(found) => return Ok(found),
            Err(error) => notes.push(error.to_string()),
        }
    }
    bail!(
        "the selected folder does not match this game's data layout{}",
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
    path.is_dir()
        && ["data", "packages"].iter().all(|name| {
            path.join(name)
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        })
}
pub fn remember(root: &Path, manifest: &Manifest, path: &Path) -> Result<()> {
    let key = identity(manifest)?;
    let path = std::fs::canonicalize(path).context("choose the original game-data folder")?;
    ensure!(path.is_dir(), "game data must be a folder");
    let text = path
        .to_str()
        .context("folder location must be representable as text")?;
    ensure!(
        text.len() <= 4096 && !text.chars().any(char::is_control),
        "invalid game-data location"
    );
    if let Some(data) = &manifest.game_data {
        if !data.layouts.is_empty() {
            matches(&path, &data.layouts)?;
        } else {
            for name in ["data", "packages"] {
                let metadata = path.join(name).symlink_metadata()?;
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "choose the original installation containing real data and packages folders"
                );
            }
        }
    }
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
