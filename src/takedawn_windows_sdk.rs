//! Managed, checksum-pinned native Windows build tools. Extraction never runs
//! SDK installers or downloaded game code. The same installer is fixture-tested
//! on Linux; the installed executables are Windows programs.
#![cfg_attr(test, allow(dead_code))]
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek},
    path::{Path, PathBuf},
};
const PINS: &str = include_str!("../engines/takedawn/windows-dependencies.json");
const MAX_ARCHIVE: u64 = 512 * 1048576;
const MAX_FILE: u64 = 512 * 1048576;
const MAX_TOTAL: u64 = 4 * 1024 * 1048576;
const MAX_FILES: usize = 100_000;
const INVENTORY: &str = "gamepkg-sdk.json";

/// Rust filesystem canonicalization uses verbatim Windows paths. The bundled
/// GNU linker does not understand that prefix in output/search arguments.
/// Preserve drive/UNC identity and spaces while using its ordinary path syntax.
/// Executable paths must retain their verbatim form to launch beyond MAX_PATH.
pub(crate) fn compiler_path(path: &Path) -> Result<PathBuf> {
    let text = path
        .to_str()
        .context("native Windows build tools require Unicode paths")?;
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        return Ok(PathBuf::from(format!(r"\\{unc}")));
    }
    if let Some(drive) = text.strip_prefix(r"\\?\") {
        ensure!(
            drive.as_bytes().get(1) == Some(&b':')
                && drive
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphabetic),
            "native Windows build tools require a drive or UNC path"
        );
        return Ok(PathBuf::from(drive));
    }
    Ok(path.to_path_buf())
}
#[derive(Clone, Deserialize)]
struct Download {
    url: String,
    sha256: String,
}
#[derive(Clone)]
enum Layout {
    Zip(usize),
    Tar(usize),
    Rust(String),
}
#[derive(Clone)]
struct Part {
    download: Download,
    destination: String,
    layout: Layout,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct Entry {
    bytes: u64,
    sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    format: String,
    recipe: String,
    files: BTreeMap<String, Entry>,
}
fn plan() -> Result<Vec<Part>> {
    let pins: serde_json::Value = serde_json::from_str(PINS)?;
    let rust = &pins["rust"];
    let mut out = Vec::new();
    for (name, record) in rust["components"]
        .as_object()
        .context("missing Rust components")?
    {
        out.push(Part {
            download: serde_json::from_value(record["gzip"].clone())?,
            destination: format!(
                "rust-{}-windows-gnu",
                rust["version"].as_str().context("missing Rust version")?
            ),
            layout: Layout::Rust(if name == "rust-std" {
                format!(
                    "rust-std-{}",
                    rust["target"].as_str().context("missing Rust target")?
                )
            } else {
                name.clone()
            }),
        });
    }
    for (key, variant, destination, layout) in [
        (
            "llvm_mingw",
            Some("windows_x86_64"),
            "llvm-mingw-windows",
            Layout::Zip(1),
        ),
        (
            "cmake",
            Some("windows_x86_64"),
            "cmake-windows",
            Layout::Zip(1),
        ),
        (
            "ninja",
            Some("windows_x86_64"),
            "ninja-windows",
            Layout::Zip(0),
        ),
        ("sdl2", Some("mingw"), "SDL2", Layout::Tar(1)),
        ("vulkan_headers", None, "vulkan-src", Layout::Tar(1)),
        ("glslang", None, "glslang-src", Layout::Tar(1)),
    ] {
        let record = if let Some(variant) = variant {
            &pins[key][variant]
        } else {
            &pins[key]
        };
        out.push(Part {
            download: serde_json::from_value(record.clone())?,
            destination: destination.into(),
            layout,
        });
    }
    for part in &out {
        ensure!(
            part.download.url.starts_with("https://")
                && part.download.sha256.len() == 64
                && part
                    .download
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "invalid reviewed SDK pin"
        );
    }
    Ok(out)
}
fn recipe() -> String {
    crate::verify::digest(format!("GamePKG native Windows SDK extraction v1\0{PINS}").as_bytes())
}
fn real_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "SDK path must be a real directory"
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "SDK directory must not be a reparse point"
        );
    }
    Ok(())
}
fn make_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path)?;
    }
    real_directory(path)
}
fn digest_file(path: &Path) -> Result<Entry> {
    let mut file = crate::files::open_regular(path, MAX_FILE, false)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        ensure!(bytes <= MAX_FILE, "SDK file grew while verifying");
        hash.update(&buffer[..n]);
    }
    Ok(Entry {
        bytes,
        sha256: hex::encode(hash.finalize()),
    })
}
fn relative(name: &str, strip: usize) -> Result<Option<String>> {
    let name = name.strip_suffix('/').unwrap_or(name);
    ensure!(
        crate::archive::valid_path(name),
        "unsafe SDK archive path: {name}"
    );
    let parts: Vec<_> = name.split('/').collect();
    if parts.len() <= strip {
        return Ok(None);
    }
    Ok(Some(parts[strip..].join("/")))
}
fn publish_file(
    root: &Path,
    name: &str,
    size: u64,
    reader: &mut impl Read,
    inventory: &mut BTreeMap<String, Entry>,
    total: &mut u64,
    names: &mut BTreeMap<String, String>,
) -> Result<()> {
    ensure!(
        crate::archive::valid_path(name) && name != INVENTORY && size <= MAX_FILE,
        "invalid SDK payload"
    );
    ensure!(
        inventory.len() < MAX_FILES,
        "SDK file inventory exceeds bound"
    );
    *total = total
        .checked_add(size)
        .context("SDK payload size overflow")?;
    ensure!(*total <= MAX_TOTAL, "SDK payload exceeds bound");
    let target = root.join(name);
    fs::create_dir_all(target.parent().context("invalid SDK payload path")?)?;
    let mut file = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
    ensure!(
        std::io::copy(&mut reader.take(size + 1), file.as_file_mut())? == size,
        "SDK member length differs"
    );
    let entry = digest_file(file.path())?;
    if let Some(prior) = inventory.get(name) {
        ensure!(prior == &entry, "conflicting SDK component file: {name}");
        return Ok(());
    }
    let mut prefix = String::new();
    for component in name.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        let folded = prefix.to_ascii_lowercase();
        if let Some(prior) = names.get(&folded) {
            ensure!(prior == &prefix, "SDK payload has a case alias");
        }
        names.insert(folded, prefix.clone());
    }
    file.persist(&target).map_err(|e| e.error)?;
    inventory.insert(name.into(), entry);
    Ok(())
}
fn unpack_zip(
    reader: impl Read + Seek,
    part: &Part,
    root: &Path,
    inventory: &mut BTreeMap<String, Entry>,
    total: &mut u64,
    strip: usize,
    names: &mut BTreeMap<String, String>,
) -> Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;
    ensure!(zip.len() <= MAX_FILES, "SDK archive has too many members");
    for index in 0..zip.len() {
        let mut item = zip.by_index(index)?;
        let Some(name) = relative(item.name(), strip)? else {
            continue;
        };
        let kind = item.unix_mode().unwrap_or(0) & 0o170000;
        ensure!(
            kind == 0 || kind == 0o100000 || (kind == 0o040000 && item.is_dir()),
            "SDK archive contains a link or special file"
        );
        if item.is_dir() {
            continue;
        }
        let size = item.size();
        publish_file(
            root,
            &format!("{}/{name}", part.destination),
            size,
            &mut item,
            inventory,
            total,
            names,
        )?;
    }
    Ok(())
}
fn unpack_tar(
    reader: impl Read,
    part: &Part,
    root: &Path,
    inventory: &mut BTreeMap<String, Entry>,
    total: &mut u64,
    names: &mut BTreeMap<String, String>,
) -> Result<()> {
    let decoder = flate2::read::MultiGzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder.take(MAX_TOTAL + 32 * 1048576));
    archive.set_ignore_zeros(true);
    let mut count = 0;
    for item in archive.entries()? {
        let mut item = item?;
        count += 1;
        ensure!(count <= MAX_FILES, "SDK archive has too many members");
        let raw = std::str::from_utf8(item.path_bytes().as_ref())?.to_owned();
        // Validate before stripping any vendor prefix, including skipped metadata.
        relative(&raw, 0)?;
        let kind = item.header().entry_type();
        // GitHub source archives carry their commit as a PAX global comment.
        // It is metadata, never an extracted file or a path/link override.
        if kind.is_pax_global_extensions() {
            ensure!(item.size() <= 65536, "SDK archive metadata is too large");
            if let Some(records) = item.pax_extensions()? {
                for record in records {
                    ensure!(
                        record?.key_bytes() == b"comment",
                        "unsupported SDK global archive metadata"
                    );
                }
            }
            continue;
        }
        ensure!(
            kind.is_file() || kind.is_dir(),
            "SDK archive contains a link or special file: {raw}"
        );
        if kind.is_dir() {
            continue;
        }
        let name = match &part.layout {
            Layout::Tar(strip) => relative(&raw, *strip)?,
            Layout::Rust(payload) => {
                let parts: Vec<_> = raw.split('/').collect();
                if parts.len() >= 3 && parts[1] == payload {
                    if parts.last() == Some(&"manifest.in") {
                        continue;
                    }
                    Some(parts[2..].join("/"))
                } else if parts.len() == 2 {
                    Some(format!("gamepkg-notices/{payload}/{}", parts[1]))
                } else {
                    None
                }
            }
            _ => unreachable!(),
        };
        if let Some(name) = name {
            let size = item.size();
            publish_file(
                root,
                &format!("{}/{name}", part.destination),
                size,
                &mut item,
                inventory,
                total,
                names,
            )?;
        }
    }
    Ok(())
}
fn unpack(
    path: &Path,
    part: &Part,
    root: &Path,
    inventory: &mut BTreeMap<String, Entry>,
    total: &mut u64,
    names: &mut BTreeMap<String, String>,
) -> Result<()> {
    // Keep the verified handle, rewind it, and extract those same bytes.
    let mut file = crate::files::open_regular(path, MAX_ARCHIVE, false)?;
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash)?;
    ensure!(
        hex::encode(hash.finalize()) == part.download.sha256,
        "SDK download integrity failed"
    );
    file.rewind()?;
    match part.layout {
        Layout::Zip(strip) => unpack_zip(file, part, root, inventory, total, strip, names),
        _ => unpack_tar(file, part, root, inventory, total, names),
    }
}
fn scan(root: &Path, dir: &Path, out: &mut BTreeMap<String, Entry>, total: &mut u64) -> Result<()> {
    real_directory(dir)?;
    for item in fs::read_dir(dir)? {
        let path = item?.path();
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "SDK cache contains a link"
        );
        if metadata.is_dir() {
            scan(root, &path, out, total)?;
        } else {
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("invalid SDK cache filename")?
                .replace('\\', "/");
            if name == INVENTORY {
                continue;
            }
            ensure!(
                crate::archive::valid_path(&name) && out.len() < MAX_FILES,
                "invalid SDK cache inventory"
            );
            let entry = digest_file(&path)?;
            *total = total
                .checked_add(entry.bytes)
                .context("SDK inventory size overflow")?;
            ensure!(*total <= MAX_TOTAL, "SDK cache exceeds bound");
            out.insert(name, entry);
        }
    }
    Ok(())
}
fn payload_digest(files: &BTreeMap<String, Entry>) -> Result<String> {
    Ok(crate::verify::digest(&serde_json::to_vec(files)?))
}
fn verify(root: &Path, expected_recipe: &str, expected_payload: &str) -> Result<()> {
    let saved: Inventory = serde_json::from_slice(&crate::files::read_regular(
        &root.join(INVENTORY),
        32 * 1048576,
        false,
    )?)?;
    ensure!(
        saved.format == "gamepkg-native-windows-sdk-v1"
            && saved.recipe == expected_recipe
            && !saved.files.is_empty(),
        "SDK cache recipe differs"
    );
    ensure!(
        payload_digest(&saved.files)? == expected_payload,
        "SDK inventory is not the reviewed official payload"
    );
    let mut actual = BTreeMap::new();
    scan(root, root, &mut actual, &mut 0)?;
    ensure!(actual == saved.files, "SDK cache integrity failed");
    Ok(())
}
fn install(
    root: &Path,
    parts: &[Part],
    identity: &str,
    expected_payload: &str,
    mut fetch: impl FnMut(&Download) -> Result<PathBuf>,
) -> Result<PathBuf> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let cache = root.join("native-sdk");
    make_directory(&cache)?;
    let _lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache.join(format!("{identity}.lock")))?;
    _lock.lock()?;
    let target = cache.join(identity);
    if target.exists() && verify(&target, identity, expected_payload).is_ok() {
        return Ok(target);
    }
    let staging = tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(&cache)?;
    let mut inventory = BTreeMap::new();
    let mut total = 0;
    let mut names = BTreeMap::new();
    for (index, part) in parts.iter().enumerate() {
        println!(
            "GamePKG: preparing native Windows build tools ({}/{})",
            index + 1,
            parts.len()
        );
        let archive = fetch(&part.download)?;
        unpack(
            &archive,
            part,
            staging.path(),
            &mut inventory,
            &mut total,
            &mut names,
        )
        .with_context(|| format!("unable to prepare {}", part.destination))?;
    }
    ensure!(!inventory.is_empty(), "empty SDK installation");
    let record = Inventory {
        format: "gamepkg-native-windows-sdk-v1".into(),
        recipe: identity.into(),
        files: inventory,
    };
    fs::write(staging.path().join(INVENTORY), serde_json::to_vec(&record)?)?;
    verify(staging.path(), identity, expected_payload)?;
    // Preserve an invalid prior cache for inspection; never recursively delete
    // unexpected files or follow a pre-existing directory link.
    if fs::symlink_metadata(&target).is_ok() {
        let quarantine = tempfile::Builder::new()
            .prefix(".replaced-")
            .tempdir_in(&cache)?;
        let path = quarantine.keep();
        fs::remove_dir(&path)?;
        fs::rename(&target, &path)?;
    }
    fs::rename(staging.path(), &target)?;
    Ok(target)
}
/// Installs official Windows executables without running any vendor installer.
/// The SDK is verified in full on every reuse. No ISO or game package is read.
pub(crate) fn prepare(root: &Path) -> Result<PathBuf> {
    let parts = plan()?;
    let identity = recipe();
    let pins: serde_json::Value = serde_json::from_str(PINS)?;
    let expected = pins["payload_inventory_sha256"]
        .as_str()
        .context("missing reviewed SDK inventory digest")?;
    let archives = root.join("native-sdk-archives");
    fs::create_dir_all(&archives)?;
    real_directory(&archives)?;
    install(root, &parts, &identity, expected, |record| {
        let path = archives.join(&record.sha256);
        if digest_file(&path).is_ok_and(|entry| entry.sha256 == record.sha256) {
            return Ok(path);
        }
        println!("GamePKG: downloading native Windows build tools");
        let downloaded = crate::registry::download_direct(&record.url, MAX_ARCHIVE)?;
        ensure!(
            digest_file(downloaded.path())?.sha256 == record.sha256,
            "SDK download integrity failed"
        );
        let mut file = tempfile::NamedTempFile::new_in(&archives)?;
        std::io::copy(&mut downloaded.reopen()?, file.as_file_mut())?;
        file.as_file().sync_all()?;
        file.persist(&path).map_err(|e| e.error)?;
        Ok(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compiler_arguments_preserve_drive_unc_unicode_and_spaces() {
        for (input, expected) in [
            (
                r"\\?\C:\Native library with spaces\工具\.tmp.exe",
                r"C:\Native library with spaces\工具\.tmp.exe",
            ),
            (
                r"\\?\UNC\server\share\Native library\.tmp.exe",
                r"\\server\share\Native library\.tmp.exe",
            ),
            (r"D:\Native library\game.exe", r"D:\Native library\game.exe"),
            (r"\\server\share\game.exe", r"\\server\share\game.exe"),
        ] {
            assert_eq!(
                compiler_path(Path::new(input)).unwrap(),
                PathBuf::from(expected)
            );
        }
        assert!(
            compiler_path(Path::new(r"\\?\GLOBALROOT\Device\HarddiskVolume1\game.exe")).is_err()
        );
    }
    fn archive(path: &Path, files: &[(&str, &[u8])]) {
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            fs::File::create(path).unwrap(),
            flate2::Compression::fast(),
        ));
        for (name, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            tar.append_data(&mut h, *name, *data).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }
    fn fixture(root: &Path) -> (PathBuf, Part) {
        let path = root.join("component.tar.gz");
        archive(
            &path,
            &[
                ("component/LICENSE-MIT", b"notice"),
                ("component/rustc/bin/rustc.exe", b"compiler"),
                ("component/rustc/manifest.in", b"metadata"),
            ],
        );
        let part = Part {
            download: Download {
                url: "https://example.invalid/component".into(),
                sha256: digest_file(&path).unwrap().sha256,
            },
            destination: "rust".into(),
            layout: Layout::Rust("rustc".into()),
        };
        (path, part)
    }
    fn fixture_digest() -> String {
        let files = [
            ("rust/bin/rustc.exe", b"compiler".as_slice()),
            (
                "rust/gamepkg-notices/rustc/LICENSE-MIT",
                b"notice".as_slice(),
            ),
        ]
        .into_iter()
        .map(|(name, data)| {
            (
                name.to_owned(),
                Entry {
                    bytes: data.len() as u64,
                    sha256: crate::verify::digest(data),
                },
            )
        })
        .collect();
        payload_digest(&files).unwrap()
    }
    #[test]
    fn fixed_plan_has_only_pinned_standalone_archives() {
        let parts = plan().unwrap();
        assert_eq!(parts.len(), 10);
        assert!(parts
            .iter()
            .filter(|p| matches!(p.layout, Layout::Rust(_)))
            .all(|p| p.download.url.ends_with(".tar.gz")));
    }
    #[test]
    fn installer_reuses_verified_cache_and_repairs_corruption_without_deleting_prior_data() {
        let temp = tempfile::tempdir().unwrap();
        let (path, part) = fixture(temp.path());
        let root = temp.path().join("library");
        let installed = install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| Ok(path.clone()),
        )
        .unwrap();
        assert_eq!(
            fs::read(installed.join("rust/bin/rustc.exe")).unwrap(),
            b"compiler"
        );
        assert_eq!(
            fs::read(installed.join("rust/gamepkg-notices/rustc/LICENSE-MIT")).unwrap(),
            b"notice"
        );
        assert!(!installed.join("rust/manifest.in").exists());
        install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| panic!("verified cache should not download"),
        )
        .unwrap();
        fs::write(installed.join("rust/bin/rustc.exe"), b"changed").unwrap();
        fs::write(installed.join("unexpected.txt"), b"preserve").unwrap();
        let mut saved: Inventory =
            serde_json::from_slice(&fs::read(installed.join(INVENTORY)).unwrap()).unwrap();
        saved.files.insert(
            "rust/bin/rustc.exe".into(),
            digest_file(&installed.join("rust/bin/rustc.exe")).unwrap(),
        );
        fs::write(
            installed.join(INVENTORY),
            serde_json::to_vec(&saved).unwrap(),
        )
        .unwrap();
        assert!(verify(&installed, "fixture", &fixture_digest()).is_err());
        install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| Ok(path.clone()),
        )
        .unwrap();
        assert_eq!(
            fs::read(installed.join("rust/bin/rustc.exe")).unwrap(),
            b"compiler"
        );
        let quarantine = fs::read_dir(root.join("native-sdk"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".replaced-")
            })
            .unwrap();
        assert_eq!(
            fs::read(quarantine.join("unexpected.txt")).unwrap(),
            b"preserve"
        );
    }
    #[test]
    fn wrong_archive_and_conflicting_component_cannot_publish() {
        let temp = tempfile::tempdir().unwrap();
        let (path, mut part) = fixture(temp.path());
        let root = temp.path().join("library");
        let digest = part.download.sha256.clone();
        part.download.sha256 = "0".repeat(64);
        assert!(install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| Ok(path.clone())
        )
        .is_err());
        assert!(!root.join("native-sdk/fixture").exists());
        part.download.sha256 = digest;
        let second = temp.path().join("second.tar.gz");
        archive(
            &second,
            &[("component/rustc/bin/rustc.exe", b"replacement")],
        );
        let mut other = part.clone();
        other.download.sha256 = digest_file(&second).unwrap().sha256;
        assert!(install(
            &root,
            &[part.clone(), other],
            "fixture",
            &fixture_digest(),
            |d| Ok(if d.sha256 == part.download.sha256 {
                path.clone()
            } else {
                second.clone()
            })
        )
        .is_err());
        assert!(!root.join("native-sdk/fixture").exists());
    }
    #[test]
    fn portable_paths_reject_traversal_devices_and_prefix_aliases() {
        for path in [
            "../escape",
            "root/../../escape",
            "/absolute",
            "C:/drive",
            "root\\name",
            "root/CON.txt",
            "root/name.",
            "root/name ",
            "root/a:b",
        ] {
            assert!(relative(path, 1).is_err(), "{path}");
        }
    }
    #[test]
    fn zip_links_and_case_aliases_cannot_publish() {
        use std::io::Write;
        for names in [
            vec!["outer/../outside"],
            vec!["outer/CON.txt"],
            vec!["outer/A.txt", "outer/a.txt"],
            vec!["outer/Include/a.h", "outer/include/b.h"],
        ] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("sdk.zip");
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            for name in names {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(b"payload").unwrap();
            }
            zip.finish().unwrap();
            let part = Part {
                download: Download {
                    url: "https://example.invalid/sdk.zip".into(),
                    sha256: digest_file(&path).unwrap().sha256,
                },
                destination: "tools".into(),
                layout: Layout::Zip(1),
            };
            assert!(
                install(temp.path(), &[part], "fixture", &fixture_digest(), |_| Ok(
                    path.clone()
                ))
                .is_err()
            );
            assert!(!temp.path().join("native-sdk/fixture").exists());
            assert!(!temp.path().join("outside").exists());
        }
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("link.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        zip.add_symlink(
            "outer/link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.finish().unwrap();
        let part = Part {
            download: Download {
                url: "https://example.invalid/sdk.zip".into(),
                sha256: digest_file(&path).unwrap().sha256,
            },
            destination: "tools".into(),
            layout: Layout::Zip(1),
        };
        assert!(
            install(temp.path(), &[part], "fixture", &fixture_digest(), |_| Ok(
                path.clone()
            ))
            .is_err()
        );
    }
    #[cfg(unix)]
    #[test]
    fn cached_links_do_not_change_files_outside_the_sdk() {
        let temp = tempfile::tempdir().unwrap();
        let (path, part) = fixture(temp.path());
        let root = temp.path().join("library");
        let installed = install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| Ok(path.clone()),
        )
        .unwrap();
        let outside = temp.path().join("outside");
        fs::write(&outside, b"compiler").unwrap();
        fs::remove_file(installed.join("rust/bin/rustc.exe")).unwrap();
        std::os::unix::fs::symlink(&outside, installed.join("rust/bin/rustc.exe")).unwrap();
        assert!(verify(&installed, "fixture", &fixture_digest()).is_err());
        install(
            &root,
            std::slice::from_ref(&part),
            "fixture",
            &fixture_digest(),
            |_| Ok(path.clone()),
        )
        .unwrap();
        assert_eq!(fs::read(&outside).unwrap(), b"compiler");
        assert!(!fs::symlink_metadata(installed.join("rust/bin/rustc.exe"))
            .unwrap()
            .file_type()
            .is_symlink());
    }
    #[test]
    #[ignore = "official SDK extraction gate; set GAMEPKG_WINDOWS_SDK_ARCHIVES to an archive cache"]
    fn official_sdk_installation() {
        let archives = PathBuf::from(
            std::env::var_os("GAMEPKG_WINDOWS_SDK_ARCHIVES").expect("archive directory"),
        );
        let root = tempfile::Builder::new()
            .prefix("gamepkg-windows-sdk-install.")
            .tempdir()
            .unwrap();
        let installed = install(
            root.path(),
            &plan().unwrap(),
            &recipe(),
            serde_json::from_str::<serde_json::Value>(PINS).unwrap()["payload_inventory_sha256"]
                .as_str()
                .unwrap(),
            |pin| {
                let named = archives.join(pin.url.rsplit('/').next().unwrap());
                let hashed = archives.join(&pin.sha256);
                let p = if named.is_file() { named } else { hashed };
                ensure!(p.is_file(), "missing pinned SDK archive: {}", pin.url);
                Ok(p)
            },
        )
        .unwrap();
        for name in [
            "rust-1.89.0-windows-gnu/bin/rustc.exe",
            "rust-1.89.0-windows-gnu/bin/cargo.exe",
            "llvm-mingw-windows/bin/x86_64-w64-mingw32-clang.exe",
            "cmake-windows/bin/cmake.exe",
            "ninja-windows/ninja.exe",
            "SDL2/x86_64-w64-mingw32/bin/SDL2.dll",
            "vulkan-src/include/vulkan/vulkan_core.h",
            "glslang-src/CMakeLists.txt",
        ] {
            assert!(installed.join(name).is_file(), "{name}");
        }
        verify(
            &installed,
            &recipe(),
            serde_json::from_str::<serde_json::Value>(PINS).unwrap()["payload_inventory_sha256"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        println!("Verified official Windows SDK: {}", installed.display());
        // Preserve only this explicitly requested real-SDK evidence tree.
        let path = root.keep();
        println!("Retained SDK evidence: {}", path.display());
    }
}
