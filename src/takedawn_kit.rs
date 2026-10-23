//! Host source-kit verification. The expected archive digest must come from
//! reviewed host metadata, never an installed game's manifest or URL response.
//! This does not register a runtime profile or grant permission to execute code.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};
const MAX_ARCHIVE: u64 = 128 * 1048576;
const MAX_UNPACKED: u64 = 1024 * 1048576;
const MAX_MANIFEST: u64 = 2 * 1048576;
const MAX_FILES: usize = 8192;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    sha256: String,
    bytes: u64,
    mode: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    game_commit: String,
    toolkit_commit: String,
    generated_game_functions: u32,
    native_exports: u32,
    files: BTreeMap<String, File>,
    omitted_maintainer_fixtures: Vec<String>,
}
fn hash_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn source_path(name: &str) -> bool {
    crate::archive::valid_path(name)
        && name != "kit.json"
        && !name.to_ascii_lowercase().ends_with(".iso")
        && !name.to_ascii_lowercase().ends_with(".xbe")
        && !(name.starts_with("game/src/game/recomp/gen/") && name.ends_with(".c"))
}
/// A newly extracted private directory, retained only for the build's lifetime.
/// Files have been matched against both the trusted archive digest and inventory.
pub struct SourceKit {
    directory: tempfile::TempDir,
    count: usize,
}
impl SourceKit {
    pub fn directory(&self) -> &Path {
        self.directory.path()
    }
    pub fn file_count(&self) -> usize {
        self.count
    }
    pub fn open(archive: &Path, trusted_sha256: &str) -> Result<Self> {
        ensure!(
            hash_valid(trusted_sha256),
            "invalid trusted source-kit digest"
        );
        // An owned bounded snapshot avoids checking one file and unpacking a
        // replacement pathname. No unverified archive is ever extracted.
        let bytes = crate::files::read_regular(archive, MAX_ARCHIVE, false)?;
        ensure!(
            crate::verify::digest(&bytes) == trusted_sha256,
            "source-kit archive integrity failed"
        );
        let decoder = flate2::read::MultiGzDecoder::new(bytes.as_slice());
        let mut tar = tar::Archive::new(decoder.take(MAX_UNPACKED + 16 * 1048576));
        // Read past tar zero blocks as well, so appended members cannot hide
        // extra files outside the signed inventory.
        tar.set_ignore_zeros(true);
        let mut entries = tar.entries()?;
        let mut first = entries.next().context("source-kit manifest is missing")??;
        ensure!(
            first.path_bytes().as_ref() == b"kit.json" && first.header().entry_type().is_file(),
            "source-kit manifest must be the first regular file"
        );
        ensure!(
            first.size() <= MAX_MANIFEST,
            "source-kit manifest exceeds 2 MiB"
        );
        let mut metadata = Vec::new();
        first.read_to_end(&mut metadata)?;
        let manifest: Manifest = serde_json::from_slice(&metadata)?;
        drop(first);
        let lock: serde_json::Value =
            serde_json::from_str(include_str!("../engines/takedawn/upstream-lock.json"))?;
        ensure!(
            manifest.format == "gamepkg-takedawn-source-kit-v1"
                && manifest.game_commit == lock["game"]["commit"].as_str().unwrap_or_default()
                && manifest.toolkit_commit
                    == lock["toolkit"]["commit"].as_str().unwrap_or_default()
                && manifest.generated_game_functions == 24001
                && manifest.native_exports == 24030,
            "unsupported source-kit revision"
        );
        ensure!(
            manifest.omitted_maintainer_fixtures == ["toolkit/tools/conformance/test.xbe"],
            "unreviewed source-kit omissions"
        );
        ensure!(
            !manifest.files.is_empty() && manifest.files.len() <= MAX_FILES,
            "invalid source-kit inventory size"
        );
        let mut total = 0u64;
        for (name, file) in &manifest.files {
            ensure!(
                source_path(name)
                    && hash_valid(&file.sha256)
                    && file.bytes <= crate::archive::MAX_FILE
                    && matches!(file.mode, 0o644 | 0o755),
                "invalid source-kit file record: {name}"
            );
            total = total
                .checked_add(file.bytes)
                .context("source-kit size overflow")?;
            ensure!(total <= MAX_UNPACKED, "source kit exceeds 1 GiB");
        }
        let directory = tempfile::tempdir()?;
        let count = manifest.files.len();
        let mut remaining = manifest.files;
        for entry in entries {
            let mut entry = entry?;
            ensure!(
                entry.header().entry_type().is_file(),
                "source-kit links and special files are forbidden"
            );
            let name = std::str::from_utf8(entry.path_bytes().as_ref())?.to_owned();
            let file = remaining
                .remove(&name)
                .context("unexpected or duplicate source-kit file")?;
            ensure!(
                entry.size() == file.bytes && entry.header().mode()? == file.mode,
                "source-kit file metadata differs: {name}"
            );
            let path = directory.path().join(&name);
            fs::create_dir_all(path.parent().context("invalid source-kit parent")?)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let n = entry.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                use std::io::Write;
                output.write_all(&buffer[..n])?;
                hash.update(&buffer[..n]);
            }
            ensure!(
                hex::encode(hash.finalize()) == file.sha256,
                "source-kit file integrity failed: {name}"
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                output.set_permissions(fs::Permissions::from_mode(file.mode))?;
            }
        }
        ensure!(remaining.is_empty(), "source-kit inventory is incomplete");
        ensure!(
            tar.into_inner().limit() > 0,
            "source-kit decompression limit exceeded"
        );
        Ok(Self { directory, count })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name: &str, payload: &[u8], extra: bool, link: bool) -> Vec<u8> {
        let lock: serde_json::Value =
            serde_json::from_str(include_str!("../engines/takedawn/upstream-lock.json")).unwrap();
        let manifest = serde_json::json!({"format":"gamepkg-takedawn-source-kit-v1", "game_commit":lock["game"]["commit"],"toolkit_commit":lock["toolkit"]["commit"],"generated_game_functions":24001,"native_exports":24030,"omitted_maintainer_fixtures":["toolkit/tools/conformance/test.xbe"],"files":{name:{"sha256":crate::verify::digest(payload),"bytes":payload.len(),"mode":420}}});
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        let mut append = |name: &str, data: &[u8], symlink: bool| {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_size(if symlink { 0 } else { data.len() as u64 });
            if symlink {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("/tmp/outside").unwrap();
            }
            header.set_cksum();
            tar.append_data(&mut header, name, if symlink { &[][..] } else { data })
                .unwrap();
        };
        append("kit.json", &serde_json::to_vec(&manifest).unwrap(), false);
        // Bad manifest paths are sufficient to test rejection before extraction.
        append(if source_path(name) { name } else { "safe" }, payload, link);
        if extra {
            append("extra", b"unexpected", false);
        }
        tar.into_inner().unwrap().finish().unwrap()
    }
    fn open(bytes: &[u8]) -> Result<SourceKit> {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(bytes)?;
        SourceKit::open(file.path(), &crate::verify::digest(bytes))
    }
    #[test]
    fn valid_source_snapshot_and_archive_tampering() {
        use std::io::Write;
        let bytes = fixture("prepared/src/one.rs", b"native source", false, false);
        let kit = open(&bytes).unwrap();
        assert_eq!(kit.file_count(), 1);
        assert_eq!(
            fs::read(kit.directory().join("prepared/src/one.rs")).unwrap(),
            b"native source"
        );
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        assert!(SourceKit::open(file.path(), &"0".repeat(64)).is_err());
    }
    #[test]
    fn unsafe_inventory_paths_and_generated_c_are_rejected() {
        for name in [
            "../outside",
            "/absolute",
            "a\\b",
            "C:/outside",
            "asset.xbe",
            "disc.iso",
            "game/src/game/recomp/gen/recomp_0000.c",
        ] {
            assert!(
                open(&fixture(name, b"data", false, false)).is_err(),
                "{name}"
            );
        }
    }
    #[test]
    fn extra_members_and_links_are_rejected() {
        assert!(open(&fixture("file", b"native", true, false)).is_err());
        assert!(open(&fixture("file", b"native", false, true)).is_err());
        assert!(open(&fixture("extra", b"native", true, false)).is_err());
    }
    #[test]
    fn changed_inner_payload_and_truncated_gzip_are_rejected() {
        use std::io::Write;
        let original = fixture("file", b"native source", false, false);
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(original.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        let offset = raw
            .windows(13)
            .position(|part| part == b"native source")
            .unwrap();
        raw[offset..offset + 13].copy_from_slice(b"edited source");
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gz.write_all(&raw).unwrap();
        // The new outer digest matches, but the signed inner file hash does not.
        assert!(open(&gz.finish().unwrap()).is_err());
        assert!(open(&original[..original.len() / 2]).is_err());
    }
    #[test]
    #[ignore = "private prepared SDK archive gate; set GAMEPKG_TAKEDAWN_KIT and GAMEPKG_TAKEDAWN_KIT_SHA256"]
    fn prepared_sdk_archive() {
        let path = std::env::var_os("GAMEPKG_TAKEDAWN_KIT").unwrap();
        let digest = std::env::var("GAMEPKG_TAKEDAWN_KIT_SHA256").unwrap();
        let kit = SourceKit::open(Path::new(&path), &digest).unwrap();
        assert!(kit.file_count() > 4000);
        assert_eq!(
            fs::read_dir(kit.directory().join("prepared/src"))
                .unwrap()
                .count(),
            485
        );
        assert!(kit
            .directory()
            .join("prepared/src/recomp_dispatch.rs")
            .is_file());
    }
}
