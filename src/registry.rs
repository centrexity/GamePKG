use crate::{archive::Package, manifest::Manifest, verify::verify_embedded};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub fn default_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("GAMEPKG_HOME") {
        return Ok(PathBuf::from(path));
    }
    let settings = crate::settings::load()?;
    if !settings.library_directory.is_empty() {
        return Ok(PathBuf::from(settings.library_directory));
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|p| PathBuf::from(p).join(".local/share"))
        })
        .context("set GAMEPKG_HOME to an installation directory")?;
    Ok(base.join("gamepkg"))
}
fn parsed_url(value: &str) -> Result<url::Url> {
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        bail!("invalid download URL");
    }
    let mut url = url::Url::parse(value).context("invalid download URL")?;
    if !url.username().is_empty() || url.password().is_some() {
        bail!("URL credentials are not allowed");
    }
    let normalized_host = url
        .host_str()
        .context("download URL requires a host")?
        .trim_end_matches('.')
        .to_owned();
    url.set_host(Some(&normalized_host))?;
    let host = url.host_str().context("download URL requires a host")?;
    let onion = host.ends_with(".onion");
    if onion {
        let service = host
            .trim_end_matches(".onion")
            .rsplit('.')
            .next()
            .unwrap_or("");
        if service.len() != 56
            || !service
                .bytes()
                .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
        {
            bail!("a v3 onion address is required");
        }
        if !matches!(url.scheme(), "http" | "https") {
            bail!("onion downloads require HTTP or HTTPS");
        }
    } else if url.scheme() != "https" {
        bail!("ordinary download URLs must use HTTPS");
    }
    Ok(url)
}
pub fn validate_url(url: &str) -> Result<()> {
    parsed_url(url).map(|_| ())
}
fn tor_proxy() -> Result<String> {
    let value = match std::env::var("GAMEPKG_TOR_PROXY") {
        Ok(value) => value,
        Err(_) => crate::settings::load()?.tor_proxy,
    };
    let address = crate::settings::validate_proxy(&value)?;
    Ok(format!("socks5h://{address}"))
}
pub fn download(value: &str) -> Result<tempfile::NamedTempFile> {
    download_kind(value, ".gamepkg", crate::archive::MAX_TOTAL)
}
pub fn download_catalog(value: &str) -> Result<tempfile::NamedTempFile> {
    download_kind(value, ".gamepkglist", 4 * 1024 * 1024)
}
pub fn download_store(value: &str) -> Result<tempfile::NamedTempFile> {
    download_kind(value, ".gamepkgstore", crate::archive::MAX_TOTAL)
}
fn download_kind(value: &str, extension: &str, limit: u64) -> Result<tempfile::NamedTempFile> {
    let url = parsed_url(value)?;
    if let Some(api) = github_release_api(&url)? {
        let metadata = download_direct(api.as_str(), 4 * 1024 * 1024)?;
        let release: serde_json::Value = serde_json::from_reader(metadata.as_file())
            .context("invalid GitHub release metadata")?;
        let asset = github_release_asset(&release, extension, limit)?;
        return download_direct(&asset, limit);
    }
    download_direct(value, limit)
}

// Repository links select the latest published release; explicit tags stay pinned.
fn github_release_api(url: &url::Url) -> Result<Option<url::Url>> {
    if url.host_str() != Some("github.com") || url.port().is_some() {
        return Ok(None);
    }
    let parts: Vec<_> = url
        .path()
        .trim_end_matches('/')
        .trim_start_matches('/')
        .split('/')
        .collect();
    let repository = matches!(parts.len(), 2)
        || (parts.len() == 4 && parts[2] == "releases" && parts[3] == "latest")
        || (parts.len() == 5 && parts[2] == "releases" && parts[3] == "tag");
    if !repository {
        return Ok(None);
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("GitHub project URLs cannot contain a query or fragment");
    }
    for part in &parts[..2] {
        if part.is_empty()
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            || matches!(*part, "." | "..")
        {
            bail!("invalid GitHub owner or repository");
        }
    }
    let endpoint = if parts.len() == 5 {
        format!("tags/{}", parts[4])
    } else {
        "latest".to_owned()
    };
    Ok(Some(url::Url::parse(&format!(
        "https://api.github.com/repos/{}/{}/releases/{endpoint}",
        parts[0], parts[1]
    ))?))
}

fn github_release_asset(
    release: &serde_json::Value,
    extension: &str,
    limit: u64,
) -> Result<String> {
    if release.get("draft").and_then(|v| v.as_bool()) != Some(false) {
        bail!("GitHub release must be published");
    }
    let assets = release
        .get("assets")
        .and_then(|v| v.as_array())
        .context("GitHub release has no assets")?;
    let packages: Vec<_> = assets
        .iter()
        .filter(|a| {
            a.get("name")
                .and_then(|v| v.as_str())
                .is_some_and(|n| n.ends_with(extension))
        })
        .collect();
    if packages.len() != 1 {
        bail!("GitHub release must contain exactly one {extension} asset; use a direct asset URL when there are multiple matching assets");
    }
    let asset = packages[0];
    let size = asset
        .get("size")
        .and_then(|v| v.as_u64())
        .context("GitHub asset has no size")?;
    if size == 0 || size > limit {
        bail!("GitHub release asset exceeds size limit or is empty");
    }
    let value = asset
        .get("browser_download_url")
        .and_then(|v| v.as_str())
        .context("GitHub asset has no download URL")?;
    let url = parsed_url(value)?;
    if url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.path().contains("/releases/download/")
    {
        bail!("invalid GitHub release asset URL");
    }
    Ok(url.to_string())
}

pub(crate) fn download_direct(value: &str, limit: u64) -> Result<tempfile::NamedTempFile> {
    let mut current = parsed_url(value)?;
    let mut tor_only = current.host_str().is_some_and(|h| h.ends_with(".onion"));
    let file = tempfile::NamedTempFile::new()?;
    for _ in 0..=5 {
        let onion = current.host_str().is_some_and(|h| h.ends_with(".onion"));
        if tor_only && !onion {
            bail!("onion downloads cannot redirect to an ordinary host");
        }
        tor_only |= onion;
        let headers = tempfile::NamedTempFile::new()?;
        // Ignore curlrc, never auto-follow redirects, and override NO_PROXY for Tor.
        let mut command = Command::new("curl");
        command
            .args([
                "--disable",
                "--fail",
                "--silent",
                "--show-error",
                "--proto",
                if onion { "=http,https" } else { "=https" },
                "--connect-timeout",
                "15",
                "--max-time",
                "120",
                "--max-filesize",
                &limit.to_string(),
                "--user-agent",
                "GamePKG/0.1",
                "--write-out",
                "%{http_code}",
                "--dump-header",
            ])
            .arg(headers.path())
            .arg("--output")
            .arg(file.path());
        if onion {
            command.args(["--proxy", &tor_proxy()?, "--noproxy", ""]);
        }
        let output = command
            .arg("--url")
            .arg(current.as_str())
            .output()
            .context("URL downloads require curl installed")?;
        if !output.status.success() {
            bail!(
                "download failed{}: {}",
                if onion {
                    " through Tor (start your local Tor SOCKS proxy)"
                } else {
                    ""
                },
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if file.as_file().metadata()?.len() > limit {
            bail!("download exceeds size limit");
        }
        let status: u16 = std::str::from_utf8(&output.stdout)?
            .trim()
            .parse()
            .context("invalid HTTP status")?;
        if (200..300).contains(&status) {
            return Ok(file);
        }
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            bail!("unexpected HTTP status {status}");
        }
        let mut data = String::new();
        fs::File::open(headers.path())?
            .take(65537)
            .read_to_string(&mut data)?;
        if data.len() > 65536 {
            bail!("redirect headers too large");
        }
        let location = data
            .lines()
            .filter_map(|l| l.split_once(':'))
            .filter(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.trim())
            .next_back()
            .context("redirect has no Location")?;
        let next = current.join(location)?;
        current = parsed_url(next.as_str())?;
    }
    bail!("too many download redirects")
}
pub struct LibraryInspection {
    pub games: Vec<Manifest>,
    pub rejected: Vec<(String, String)>,
}
/// Inspect each file independently for display; rejected files never supply game metadata.
pub fn inspect_library(root: &Path) -> Result<LibraryInspection> {
    let mut report = LibraryInspection {
        games: Vec::new(),
        rejected: Vec::new(),
    };
    if !root.exists() {
        return Ok(report);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.path().extension().is_none_or(|x| x != "gamepkg") {
            continue;
        }
        let result = (|| -> Result<Manifest> {
            if entry.file_type()?.is_symlink() {
                bail!("installed package is a symlink");
            }
            let m = verify_embedded(&mut Package::open(&entry.path())?)?;
            if entry.file_name().to_str() != Some(&format!("{}.gamepkg", m.id)) {
                bail!("installed package ID mismatch");
            }
            Ok(m)
        })();
        match result {
            Ok(m) => report.games.push(m),
            Err(error) => report.rejected.push((
                entry.file_name().to_string_lossy().into_owned(),
                format!("{error:#}"),
            )),
        }
    }
    report.games.sort_by(|a, b| a.id.cmp(&b.id));
    report.rejected.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(report)
}
/// Verify one installed game without letting unrelated damaged files block it.
pub fn installed_manifest(root: &Path, id: &str) -> Result<Manifest> {
    installed_manifest_with_policy(root, id, true)
}

fn installed_manifest_with_policy(root: &Path, id: &str, scan_source: bool) -> Result<Manifest> {
    if !crate::archive::valid_path(id)
        || id.len() > 200
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        bail!("invalid package ID");
    }
    let path = root.join(format!("{id}.gamepkg"));
    let metadata = fs::symlink_metadata(&path).context("package is not installed")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("installed package must be a regular file");
    }
    let mut package = Package::open(&path)?;
    let manifest = if scan_source {
        verify_embedded(&mut package)?
    } else {
        crate::verify::verify_signed_identity(&mut package)?
    };
    if manifest.id != id {
        bail!("installed package ID mismatch");
    }
    Ok(manifest)
}
pub fn list(root: &Path) -> Result<Vec<Manifest>> {
    let report = inspect_library(root)?;
    if let Some((name, error)) = report.rejected.first() {
        bail!("installed package {name} rejected: {error}");
    }
    Ok(report.games)
}
pub(crate) struct LibraryChange {
    path: PathBuf,
    handle: Option<fs::File>,
}
impl LibraryChange {
    pub(crate) fn acquire(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let path = root.join(".install-lock");
        let handle = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .context("another library change is active; if interrupted, remove .install-lock")?;
        Ok(Self {
            path,
            handle: Some(handle),
        })
    }
}
impl Drop for LibraryChange {
    fn drop(&mut self) {
        self.handle.take();
        let _ = fs::remove_file(&self.path);
    }
}
pub fn install(root: &Path, path: &Path, approve_creator: bool) -> Result<Manifest> {
    let _lock = LibraryChange::acquire(root)?;
    // Snapshot bytes so verification and publication operate on the same file.
    let mut snapshot = tempfile::NamedTempFile::new_in(root)?;
    let copied = std::io::copy(
        &mut fs::File::open(path)?.take(crate::archive::MAX_TOTAL + 1),
        snapshot.as_file_mut(),
    )?;
    if copied > crate::archive::MAX_TOTAL {
        bail!("package exceeds installation size limit");
    }
    snapshot.flush()?;
    let mut package = Package::open(snapshot.path())?;
    let m = verify_embedded(&mut package)?;
    package.resolve_shared_assets(root)?;
    let target = root.join(format!("{}.gamepkg", m.id));
    if target.exists() {
        if fs::symlink_metadata(&target)?.file_type().is_symlink() {
            bail!("installed package is a symlink");
        }
        let old = crate::verify::verify_signed_identity(&mut Package::open(&target)?)?;
        if old.creator_key != m.creator_key {
            bail!("update creator key differs from installed creator");
        }
        if old.id != m.id
            || semver::Version::parse(&m.version)? <= semver::Version::parse(&old.version)?
        {
            bail!("update must have same ID and a higher version");
        }
    } else if !approve_creator {
        bail!(
            "first install requires creator approval: {} ({})",
            m.id,
            crate::verify::digest(&hex::decode(&m.creator_key)?)
        );
    }
    snapshot.as_file().sync_all()?;
    crate::integrity::retain_package(root, &m, snapshot.path())?;
    snapshot.persist(&target).map_err(|e| e.error)?;
    Ok(m)
}
pub fn update(root: &Path, id: &str) -> Result<Manifest> {
    let old = installed_manifest_with_policy(root, id, false)?;
    let url = old.update_url.context("package has no update URL")?;
    let downloaded = download(&url)?;
    let candidate = verify_embedded(&mut Package::open(downloaded.path())?)?;
    if candidate.id != id {
        bail!("update URL returned a different package ID");
    }
    install(root, downloaded.path(), false)
}

/// Stage an available update without installing or modifying the current game.
pub fn check_update(root: &Path, id: &str) -> Result<Option<(Manifest, tempfile::NamedTempFile)>> {
    let old = installed_manifest_with_policy(root, id, false)?;
    let url = old.update_url.context("package has no update URL")?;
    let file = download(&url)?;
    let candidate = verify_embedded(&mut Package::open(file.path())?)?;
    if candidate.id != old.id {
        bail!("update URL returned a different package ID");
    }
    if candidate.creator_key != old.creator_key {
        bail!("update creator key differs from installed creator");
    }
    if semver::Version::parse(&candidate.version)? < semver::Version::parse(&old.version)? {
        bail!("update URL returned an older version");
    }
    if candidate.version == old.version {
        return Ok(None);
    }
    Ok(Some((candidate, file)))
}

#[cfg(test)]
mod github_tests {
    use super::*;
    #[test]
    fn repository_latest_tag_and_direct_asset_urls() {
        for (source, endpoint) in [
            ("https://github.com/creator/game/", "latest"),
            ("https://github.com/creator/game/releases/latest", "latest"),
            (
                "https://github.com/creator/game/releases/tag/v1.2.3",
                "tags/v1.2.3",
            ),
        ] {
            assert_eq!(
                github_release_api(&parsed_url(source).unwrap())
                    .unwrap()
                    .unwrap()
                    .as_str(),
                format!("https://api.github.com/repos/creator/game/releases/{endpoint}")
            );
        }
        assert!(github_release_api(
            &parsed_url("https://github.com/creator/game/releases/download/v1/game.gamepkg")
                .unwrap()
        )
        .unwrap()
        .is_none());
        assert!(github_release_api(
            &parsed_url("https://github.com/creator/game?token=secret").unwrap()
        )
        .is_err());
        assert!(github_release_api(
            &parsed_url("https://github.com.evil.test/creator/game").unwrap()
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn published_single_bounded_package_required() {
        let mut release = serde_json::json!({"draft": false, "assets": [{"name": "game.gamepkg", "size": 123, "browser_download_url": "https://github.com/creator/game/releases/download/v1/game.gamepkg"}, {"name": "README.txt"}]});
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_ok());
        release["assets"][0]["size"] = serde_json::json!(crate::archive::MAX_TOTAL + 1);
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_err());
        release["assets"][0]["size"] = serde_json::json!(123);
        release["assets"][0]["browser_download_url"] =
            serde_json::json!("https://evil.test/game.gamepkg");
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_err());
        release["assets"] = serde_json::json!([]);
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_err());
        release["assets"] = serde_json::json!([{"name":"a.gamepkg"}, {"name":"b.gamepkg"}]);
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_err());
        release["draft"] = serde_json::json!(true);
        assert!(github_release_asset(&release, ".gamepkg", crate::archive::MAX_TOTAL).is_err());
    }
}
