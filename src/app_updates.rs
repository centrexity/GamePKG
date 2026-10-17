//! Updates of the portable host application, separate from game package updates.
use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

const API: &str = "https://api.github.com/repos/centrexity/GamePKG/releases/latest";
const MAX_DOWNLOAD: u64 = 512 * 1024 * 1024;
const MAX_EXPANDED: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Release {
    pub version: semver::Version,
    pub name: String,
    url: String,
    checksum_url: String,
    size: u64,
}
#[derive(Deserialize)]
struct Metadata {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn platform() -> Result<String> {
    ensure!(
        matches!(std::env::consts::OS, "linux" | "windows"),
        "App updates are available for Linux and Windows portable releases"
    );
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "x86" => "i686",
        _ => bail!("No portable release for this CPU"),
    };
    Ok(format!("{}-{arch}", std::env::consts::OS))
}
fn asset_url(value: &str, tag: &str, name: &str) -> Result<()> {
    let expected = format!("https://github.com/centrexity/GamePKG/releases/download/{tag}/{name}");
    ensure!(
        value == expected,
        "Release asset is not from the official GamePKG repository"
    );
    Ok(())
}
fn parse_release(bytes: &[u8], current: &str, platform: &str) -> Result<Option<Release>> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    ensure!(
        !metadata.draft && !metadata.prerelease,
        "Expected a stable public release"
    );
    let version = semver::Version::parse(
        metadata
            .tag_name
            .strip_prefix('v')
            .context("Invalid release tag")?,
    )?;
    ensure!(
        version.pre.is_empty() && version.build.is_empty(),
        "Invalid stable version"
    );
    if version <= semver::Version::parse(current)? {
        return Ok(None);
    }
    let extension = if platform.starts_with("windows-") {
        "zip"
    } else {
        "tar.gz"
    };
    let name = format!("gamepkg-{version}-{platform}.{extension}");
    let find = |name: &str| -> Result<&Asset> {
        let matches: Vec<_> = metadata.assets.iter().filter(|a| a.name == name).collect();
        ensure!(
            matches.len() == 1,
            "Release is missing a unique {name} asset"
        );
        let asset = matches[0];
        asset_url(&asset.browser_download_url, &metadata.tag_name, name)?;
        Ok(asset)
    };
    let archive = find(&name)?;
    ensure!(
        archive.size > 0 && archive.size <= MAX_DOWNLOAD,
        "Release download is too large or empty"
    );
    let checksum = find(&format!("{name}.sha256"))?;
    ensure!(
        checksum.size > 0 && checksum.size <= 1024,
        "Invalid release checksum size"
    );
    Ok(Some(Release {
        version,
        name,
        url: archive.browser_download_url.clone(),
        checksum_url: checksum.browser_download_url.clone(),
        size: archive.size,
    }))
}
pub fn check() -> Result<Option<Release>> {
    let data = crate::registry::download_direct(API, 4 * 1024 * 1024)?;
    parse_release(
        &fs::read(data.path())?,
        env!("CARGO_PKG_VERSION"),
        &platform()?,
    )
}
fn checksum(bytes: &[u8], name: &str) -> Result<String> {
    let value = std::str::from_utf8(bytes)?;
    let fields: Vec<_> = value.split_whitespace().collect();
    ensure!(
        fields.len() == 2
            && fields[1] == name
            && fields[0].len() == 64
            && fields[0].bytes().all(|c| c.is_ascii_hexdigit()),
        "Invalid release checksum"
    );
    Ok(fields[0].to_ascii_lowercase())
}
fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
// Reject links, absolute paths, duplicate entries, Windows drive/ADS names and
// path separators with different interpretations on the two supported systems.
fn relative_path(value: &Path, root: &str) -> Result<PathBuf> {
    let text = value.to_str().context("Non-UTF-8 archive path")?;
    ensure!(
        !text.contains(['\\', ':']) && !text.chars().any(char::is_control),
        "Unsafe archive path"
    );
    let mut components = value.components();
    ensure!(
        components.next() == Some(Component::Normal(root.as_ref())),
        "Wrong release archive root"
    );
    let mut relative = PathBuf::new();
    for component in components {
        match component {
            Component::Normal(p) => {
                let s = p.to_str().context("Invalid path")?;
                ensure!(!s.ends_with(['.', ' ']), "Ambiguous archive path");
                let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
                ensure!(
                    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        && !(stem.len() == 4
                            && (stem.starts_with("COM") || stem.starts_with("LPT"))
                            && matches!(stem.as_bytes()[3], b'1'..=b'9')),
                    "Reserved archive path"
                );
                relative.push(p);
            }
            _ => bail!("Unsafe archive path"),
        }
    }
    Ok(relative)
}
fn extract(file: &Path, destination: &Path, root: &str, zip: bool) -> Result<()> {
    let mut total = 0u64;
    let mut seen = HashSet::new();
    let mut copy = |path: &Path, size: u64, directory: bool, input: &mut dyn Read| -> Result<()> {
        let relative = relative_path(path, root)?;
        ensure!(
            seen.len() < 100000 && seen.insert(relative.to_string_lossy().to_lowercase()),
            "Duplicate archive path or too many entries"
        );
        total = total.checked_add(size).context("Archive size overflow")?;
        ensure!(total <= MAX_EXPANDED, "Expanded release is too large");
        let output = destination.join(relative);
        if directory {
            fs::create_dir_all(output)?;
        } else {
            fs::create_dir_all(output.parent().context("Missing parent")?)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            let copied = std::io::copy(&mut input.take(size + 1), &mut output)?;
            ensure!(copied == size, "Incorrect archive entry size");
            output.flush()?;
        }
        Ok(())
    };
    if zip {
        let mut archive = zip::ZipArchive::new(fs::File::open(file)?)?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let kind = entry.unix_mode().unwrap_or(0) & 0o170000;
            ensure!(
                matches!(kind, 0 | 0o100000 | 0o040000),
                "Release archive contains a link or special file"
            );
            let path = PathBuf::from(entry.name());
            copy(&path, entry.size(), entry.is_dir(), &mut entry)?;
        }
    } else {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(file)?));
        for entry in archive.entries()? {
            let mut entry = entry?;
            let kind = entry.header().entry_type();
            ensure!(
                kind.is_file() || kind.is_dir(),
                "Release archive contains a link or special file"
            );
            let path = entry.path()?.into_owned();
            copy(&path, entry.size(), kind.is_dir(), &mut entry)?;
        }
    }
    Ok(())
}
#[derive(Deserialize)]
struct Build {
    version: String,
    platform: String,
    executables_sha256: std::collections::BTreeMap<String, String>,
}
fn validate_install(root: &Path, version: &str, platform: &str) -> Result<PathBuf> {
    ensure!(
        fs::metadata(root.join("BUILD.json"))?.len() <= 65536,
        "Invalid build metadata"
    );
    let data = fs::read(root.join("BUILD.json"))?;
    ensure!(data.len() <= 65536, "Invalid build metadata");
    let build: Build = serde_json::from_slice(&data)?;
    ensure!(
        build.version == version && build.platform == platform,
        "Release build identity mismatch"
    );
    let suffix = if platform.starts_with("windows-") {
        ".exe"
    } else {
        ""
    };
    for binary in ["gamepkg", "gamepkg-gui"] {
        let name = format!("{binary}{suffix}");
        let path = root.join("bin").join(&name);
        ensure!(
            build.executables_sha256.get(&name) == Some(&hash(&path)?),
            "Release executable checksum mismatch: {name}"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(root.join("bin").join(format!("gamepkg-gui{suffix}")))
}
/// Installs into a new portable folder. The existing application and library are
/// never modified, including if downloading, verification or extraction fails.
pub fn install(release: &Release, parent: &Path) -> Result<PathBuf> {
    ensure!(
        parent.is_absolute() && parent.is_dir(),
        "Choose an existing absolute installation folder"
    );
    let platform = platform()?;
    let root = format!("gamepkg-{}-{platform}", release.version);
    let target = parent.join(&root);
    ensure!(
        !target.exists(),
        "This version is already present at {}. Choose another folder or launch the existing copy.",
        target.display()
    );
    let checksum_file = crate::registry::download_direct(&release.checksum_url, 1024)?;
    let expected = checksum(&fs::read(checksum_file.path())?, &release.name)?;
    let archive = crate::registry::download_direct(&release.url, MAX_DOWNLOAD)?;
    ensure!(
        archive.as_file().metadata()?.len() == release.size && hash(archive.path())? == expected,
        "Release download failed its integrity check"
    );
    let staging = tempfile::Builder::new()
        .prefix(".gamepkg-update-")
        .tempdir_in(parent)?;
    extract(
        archive.path(),
        staging.path(),
        &root,
        platform.starts_with("windows-"),
    )?;
    let relative_executable =
        validate_install(staging.path(), &release.version.to_string(), &platform)?
            .strip_prefix(staging.path())?
            .to_path_buf();
    // Never merge with another install or remove any user files.
    ensure!(
        !target.exists(),
        "Installation destination appeared while downloading"
    );
    publish(staging.path(), &target).context("Unable to finish installing the update")?;
    Ok(target.join(relative_executable))
}
fn publish(staging: &Path, target: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let source = std::ffi::CString::new(staging.as_os_str().as_bytes())?;
        let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
        // Atomic publication must not replace a directory created concurrently.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                target.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(not(target_os = "linux"))]
    fs::rename(staging, target)?;
    Ok(())
}
pub fn suggested_parent() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let root = exe.parent()?.parent()?;
    root.join("BUILD.json")
        .is_file()
        .then(|| root.parent().map(Path::to_path_buf))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> serde_json::Value {
        serde_json::json!({"tag_name":"v0.1.17","draft":false,"prerelease":false,"assets":[
            {"name":"gamepkg-0.1.17-linux-x86_64.tar.gz","size":100,"browser_download_url":"https://github.com/centrexity/GamePKG/releases/download/v0.1.17/gamepkg-0.1.17-linux-x86_64.tar.gz"},
            {"name":"gamepkg-0.1.17-linux-x86_64.tar.gz.sha256","size":100,"browser_download_url":"https://github.com/centrexity/GamePKG/releases/download/v0.1.17/gamepkg-0.1.17-linux-x86_64.tar.gz.sha256"}
        ]})
    }
    fn parse(value: &serde_json::Value, version: &str) -> Result<Option<Release>> {
        parse_release(&serde_json::to_vec(value)?, version, "linux-x86_64")
    }
    #[test]
    fn stable_versions_and_no_downgrades() {
        assert_eq!(
            parse(&metadata(), "0.1.16")
                .unwrap()
                .unwrap()
                .version
                .to_string(),
            "0.1.17"
        );
        assert!(parse(&metadata(), "0.1.17").unwrap().is_none());
        assert!(parse(&metadata(), "0.2.0").unwrap().is_none());
        assert!(parse(&metadata(), "0.1.9").unwrap().is_some());
    }
    #[test]
    fn rejects_untrusted_or_incomplete_releases() {
        for key in ["draft", "prerelease"] {
            let mut v = metadata();
            v[key] = true.into();
            assert!(parse(&v, "0.1.16").is_err());
        }
        for tag in ["v0.1.17-beta.1", "v0.1.17+foo", "../../evil"] {
            let mut v = metadata();
            v["tag_name"] = tag.into();
            assert!(parse(&v, "0.1.16").is_err());
        }
        for url in ["http://github.com/centrexity/GamePKG/x", "https://evil.test/a", "https://github.com/other/GamePKG/releases/download/v0.1.17/gamepkg-0.1.17-linux-x86_64.tar.gz"] {
            let mut v = metadata(); v["assets"][0]["browser_download_url"] = url.into(); assert!(parse(&v, "0.1.16").is_err());
        }
        let mut v = metadata();
        v["assets"].as_array_mut().unwrap().pop();
        assert!(parse(&v, "0.1.16").is_err());
        let mut v = metadata();
        let a = v["assets"][0].clone();
        v["assets"].as_array_mut().unwrap().push(a);
        assert!(parse(&v, "0.1.16").is_err());
        let mut v = metadata();
        v["assets"][0]["size"] = (MAX_DOWNLOAD + 1).into();
        assert!(parse(&v, "0.1.16").is_err());
        assert!(parse_release(
            &serde_json::to_vec(&metadata()).unwrap(),
            "0.1.16",
            "windows-i686"
        )
        .is_err());
    }
    #[test]
    fn strict_checksums() {
        let digest = "a".repeat(64);
        assert_eq!(
            checksum(format!("{digest}  release.zip\n").as_bytes(), "release.zip").unwrap(),
            digest
        );
        for text in [
            format!("{digest} other.zip"),
            format!("{digest} release.zip extra"),
            "bad release.zip".into(),
            format!("{} release.zip", "z".repeat(64)),
        ] {
            assert!(checksum(text.as_bytes(), "release.zip").is_err());
        }
    }
    #[test]
    fn rejects_path_escapes_and_windows_aliases() {
        for path in [
            "/release/a",
            "release/../a",
            "other/a",
            "release/a\\b",
            "release/C:foo",
            "release/CON.txt",
            "release/nul",
            "release/COM1.txt",
            "release/a.",
            "release/a ",
        ] {
            assert!(relative_path(Path::new(path), "release").is_err(), "{path}");
        }
        assert_eq!(
            relative_path(Path::new("release/bin/gamepkg"), "release").unwrap(),
            PathBuf::from("bin/gamepkg")
        );
    }
    fn zip_entries(entries: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        {
            let mut zip = zip::ZipWriter::new(&mut file);
            for (path, data) in entries {
                zip.start_file(
                    *path,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
                zip.write_all(data).unwrap();
            }
            zip.finish().unwrap();
        }
        file
    }
    #[test]
    fn deflate_zip_extracts_and_rejects_case_collisions_and_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let zip = zip_entries(&[("release/bin/app.exe", b"hello")]);
        extract(zip.path(), dir.path(), "release", true).unwrap();
        assert_eq!(fs::read(dir.path().join("bin/app.exe")).unwrap(), b"hello");
        for entries in [
            vec![("release/A", &b"one"[..]), ("release/a", &b"two"[..])],
            vec![("release/../escape", &b"bad"[..])],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let zip = zip_entries(&entries);
            assert!(extract(zip.path(), dir.path(), "release", true).is_err());
            assert!(!dir.path().parent().unwrap().join("escape").exists());
        }
    }
    #[test]
    fn rejects_tar_links_and_oversized_files() {
        for link in [true, false] {
            let file = tempfile::NamedTempFile::new().unwrap();
            let mut gzip =
                flate2::write::GzEncoder::new(file.reopen().unwrap(), flate2::Compression::fast());
            if link {
                let mut archive = tar::Builder::new(&mut gzip);
                let mut header = tar::Header::new_gnu();
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                header.set_mode(0o777);
                header.set_link_name("/tmp").unwrap();
                header.set_cksum();
                archive
                    .append_data(&mut header, "release/link", std::io::empty())
                    .unwrap();
                archive.finish().unwrap();
            } else {
                let mut header = tar::Header::new_gnu();
                header.set_path("release/huge").unwrap();
                header.set_size(MAX_EXPANDED + 1);
                header.set_mode(0o644);
                header.set_cksum();
                gzip.write_all(header.as_bytes()).unwrap();
            }
            gzip.finish().unwrap();
            let dir = tempfile::tempdir().unwrap();
            assert!(extract(file.path(), dir.path(), "release", false).is_err());
        }
    }
    #[test]
    fn build_identity_and_binary_hashes_required() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("bin")).unwrap();
        for name in ["gamepkg", "gamepkg-gui"] {
            fs::write(dir.path().join("bin").join(name), name).unwrap();
        }
        let build = serde_json::json!({"version":"0.1.17","platform":"linux-x86_64","executables_sha256":{
            "gamepkg":hash(&dir.path().join("bin/gamepkg")).unwrap(), "gamepkg-gui":hash(&dir.path().join("bin/gamepkg-gui")).unwrap()
        }});
        fs::write(dir.path().join("BUILD.json"), build.to_string()).unwrap();
        assert!(validate_install(dir.path(), "0.1.17", "linux-x86_64").is_ok());
        assert!(validate_install(dir.path(), "0.1.18", "linux-x86_64").is_err());
        assert!(validate_install(dir.path(), "0.1.17", "windows-x86_64").is_err());
        fs::write(dir.path().join("bin/gamepkg"), b"tampered").unwrap();
        assert!(validate_install(dir.path(), "0.1.17", "linux-x86_64").is_err());
    }
    #[test]
    fn publication_never_replaces_an_existing_install() {
        let parent = tempfile::tempdir().unwrap();
        let staging = parent.path().join("staging");
        let target = parent.path().join("existing");
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(staging.join("new"), "new").unwrap();
        fs::write(target.join("keep"), "old").unwrap();
        assert!(publish(&staging, &target).is_err());
        assert_eq!(fs::read_to_string(target.join("keep")).unwrap(), "old");
        assert!(staging.join("new").is_file());
        fs::remove_file(target.join("keep")).unwrap();
        assert!(
            publish(&staging, &target).is_err(),
            "Even an empty existing directory must be preserved"
        );
        fs::remove_dir(&target).unwrap();
        publish(&staging, &target).unwrap();
        assert!(target.join("new").is_file());
    }
    #[test]
    #[ignore = "Validates both real release archive formats; set GAMEPKG_UPDATE_TEST_ARCHIVES"]
    fn packaged_linux_and_windows_archives() -> Result<()> {
        let parent = PathBuf::from(
            std::env::var_os("GAMEPKG_UPDATE_TEST_ARCHIVES")
                .context("Set GAMEPKG_UPDATE_TEST_ARCHIVES")?,
        );
        for (platform, extension) in [("linux-x86_64", "tar.gz"), ("windows-x86_64", "zip")] {
            let root = format!("gamepkg-0.1.16-{platform}");
            let dir = tempfile::tempdir()?;
            extract(
                &parent.join(format!("{root}.{extension}")),
                dir.path(),
                &root,
                extension == "zip",
            )?;
            ensure!(
                validate_install(dir.path(), "0.1.16", platform)?.is_file(),
                "Missing GUI"
            );
        }
        Ok(())
    }
    #[test]
    #[ignore = "Downloads and installs the real latest official release into a temporary folder"]
    fn live_release_install() -> Result<()> {
        let data = crate::registry::download_direct(API, 4 * 1024 * 1024)?;
        let release =
            parse_release(&fs::read(data.path())?, "0.0.0", &platform()?)?.context("No release")?;
        let parent = tempfile::tempdir()?;
        let old = parent.path().join("old-version");
        fs::create_dir(&old)?;
        fs::write(old.join("keep"), "unchanged")?;
        let executable = install(&release, parent.path())?;
        ensure!(executable.is_file(), "Missing updated executable");
        let output =
            std::process::Command::new(executable.parent().unwrap().join(if cfg!(windows) {
                "gamepkg.exe"
            } else {
                "gamepkg"
            }))
            .arg("--version")
            .output()?;
        ensure!(
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&release.version.to_string()),
            "Installed CLI failed"
        );
        ensure!(
            install(&release, parent.path()).is_err(),
            "Existing version was overwritten"
        );
        ensure!(
            fs::read_to_string(old.join("keep"))? == "unchanged",
            "Old installation modified"
        );
        Ok(())
    }
}
