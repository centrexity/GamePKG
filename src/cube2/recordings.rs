//! Explicit player export of validated, creator-scoped original recordings.
use anyhow::Result;
use std::path::Path;
#[derive(Debug, Clone, serde::Serialize)]
pub struct Recording {
    pub name: String,
    pub bytes: usize,
    pub kind: &'static str,
}
pub fn list(root: &Path, id: &str) -> Result<Vec<Recording>> {
    #[cfg(target_os = "linux")]
    return local::list(root, id);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, id);
        anyhow::bail!("Recording access is currently supported on Linux");
    }
}
pub fn export(root: &Path, id: &str, name: &str, output: &Path) -> Result<usize> {
    #[cfg(target_os = "linux")]
    return local::export(root, id, name, output);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, id, name, output);
        anyhow::bail!("Recording export is currently supported on Linux");
    }
}
pub fn import(root: &Path, id: &str, source: &Path, name: Option<&str>) -> Result<Recording> {
    #[cfg(target_os = "linux")]
    return local::import(root, id, source, name, None);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, id, source, name);
        anyhow::bail!("Recording import is currently supported on Linux");
    }
}
#[cfg(feature = "gui")]
pub(crate) fn import_selected(
    root: &Path,
    manifest: &crate::manifest::Manifest,
    source: &Path,
) -> Result<Recording> {
    #[cfg(target_os = "linux")]
    return local::import(
        root,
        &manifest.id,
        source,
        None,
        Some((&manifest.creator_key, manifest.save_schema)),
    );
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, manifest, source);
        anyhow::bail!("Recording import is currently supported on Linux");
    }
}
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn test_import_roundtrip(seed: &[u8], name: &str, bytes: &[u8]) -> Vec<u8> {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("library");
    let package = temporary.path().join("test.gamepkg");
    crate::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/cube2"),
        &package,
        &[0x69; 32],
    )
    .unwrap();
    let manifest = crate::registry::install(&root, &package, true).unwrap();
    let entries = crate::cube2::protocol::save_files(seed).unwrap();
    let kept = entries
        .iter()
        .filter(|file| file.path != name)
        .map(|file| (file.path, file.bytes))
        .collect::<Vec<_>>();
    crate::cube2::save::Store::open(
        &root,
        &manifest.id,
        &manifest.creator_key,
        manifest.save_schema,
    )
    .unwrap()
    .commit(&crate::cube2::protocol::test_tree(&kept))
    .unwrap();
    let source = temporary.path().join(Path::new(name).file_name().unwrap());
    std::fs::write(&source, bytes).unwrap();
    import(&root, &manifest.id, &source, Some(name)).unwrap();
    let restored = crate::cube2::save::Store::read_snapshot(
        &root,
        &manifest.id,
        &manifest.creator_key,
        manifest.save_schema,
    )
    .unwrap();
    assert_eq!(
        restored, seed,
        "Import changed existing files or recording bytes"
    );
    restored
}
#[cfg(target_os = "linux")]
mod local {
    use super::*;
    use anyhow::{ensure, Context};
    use std::{
        fs,
        io::{Read, Write},
        os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
        path::PathBuf,
    };
    fn read(root: &Path, id: &str) -> Result<Vec<u8>> {
        let manifest = crate::registry::installed_manifest(root, id)?;
        ensure!(
            manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE),
            "This game has no original-engine recordings"
        );
        crate::cube2::save::Store::read_snapshot(
            root,
            &manifest.id,
            &manifest.creator_key,
            manifest.save_schema,
        )
    }
    fn files(seed: &[u8]) -> Result<Vec<crate::cube2::protocol::SaveFile<'_>>> {
        if seed.is_empty() || seed.starts_with(b"GPS1") {
            return Ok(Vec::new());
        }
        crate::cube2::protocol::save_files(seed)
    }
    fn kind(name: &str) -> Option<&'static str> {
        let extension = name.get(name.len().checked_sub(4)?..)?;
        if extension.eq_ignore_ascii_case(".dmo") {
            Some("demo")
        } else if extension.eq_ignore_ascii_case(".avi") {
            Some("movie")
        } else {
            None
        }
    }
    pub(super) fn list(root: &Path, id: &str) -> Result<Vec<Recording>> {
        let seed = read(root, id)?;
        Ok(files(&seed)?
            .into_iter()
            .filter_map(|file| {
                kind(file.path).map(|kind| Recording {
                    name: file.path.to_owned(),
                    bytes: file.bytes.len(),
                    kind,
                })
            })
            .collect())
    }
    pub(super) fn export(root: &Path, id: &str, name: &str, output: &Path) -> Result<usize> {
        ensure!(kind(name).is_some(), "Select a saved demo or AVI movie");
        let seed = read(root, id)?;
        let files = files(&seed)?;
        let file = files
            .iter()
            .find(|file| file.path == name)
            .context("Recording is not present in this game's save")?;
        write_new(output, file.bytes)?;
        Ok(file.bytes.len())
    }
    pub(super) fn import(
        root: &Path,
        id: &str,
        source: &Path,
        name: Option<&str>,
        expected: Option<(&str, u32)>,
    ) -> Result<Recording> {
        let _change = crate::registry::LibraryChange::acquire(root)?;
        let manifest = crate::registry::installed_manifest(root, id)?;
        ensure!(
            manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE),
            "This game has no original-engine recordings"
        );
        if let Some((creator, schema)) = expected {
            ensure!(
                manifest.creator_key == creator && manifest.save_schema == schema,
                "The game changed. Refresh the library before importing recordings"
            );
        }
        let mut store = crate::cube2::save::Store::open(
            root,
            &manifest.id,
            &manifest.creator_key,
            manifest.save_schema,
        )?;
        let bytes =
            crate::files::read_regular(source, crate::cube2::protocol::MAX_SAVE_FILE as u64, true)?;
        let basename = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("Select a recording filename")?;
        let default = match kind(basename) {
            Some("demo") => format!("demo/{basename}"),
            Some("movie") => format!("movies/{basename}"),
            _ => anyhow::bail!("Select a .dmo demo or .avi movie"),
        };
        let name = name.unwrap_or(&default);
        ensure!(
            crate::cube2::protocol::save_path(name),
            "Recording name must be a safe relative path"
        );
        let kind = kind(name).context("Recording name must end in .dmo or .avi")?;
        validate(&bytes, kind)?;
        let merged = merge(&store.seed, name, &bytes)?;
        store.commit(&merged)?;
        Ok(Recording {
            name: name.to_owned(),
            bytes: bytes.len(),
            kind,
        })
    }
    fn validate(bytes: &[u8], kind: &str) -> Result<()> {
        ensure!(
            bytes.len() <= crate::cube2::protocol::MAX_SAVE_FILE,
            "Recording exceeds the per-file save quota"
        );
        if kind == "movie" {
            ensure!(
                bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"AVI ",
                "Select an AVI movie"
            );
            ensure!(
                u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize == bytes.len() - 8,
                "AVI size does not match its header"
            );
            let mut found = [false; 3];
            avi_chunks(&bytes[12..], 0, &mut found)?;
            ensure!(
                found.iter().all(|found| *found),
                "AVI needs a video header, stream and frame data"
            );
            return Ok(());
        }
        let mut decoder = flate2::bufread::GzDecoder::new(bytes);
        let mut header = [0u8; 24];
        decoder.read_exact(&mut header)?;
        ensure!(
            &header[..16] == b"SAUERBRATEN_DEMO"
                && u32::from_le_bytes(header[16..20].try_into().unwrap()) == 1
                && u32::from_le_bytes(header[20..24].try_into().unwrap()) == 260,
            "Demo version does not match this engine"
        );
        let mut total = 24usize;
        let mut packets = 0usize;
        let mut previous = 0i32;
        loop {
            let mut packet = [0u8; 12];
            if decoder.read(&mut packet[..1])? == 0 {
                break;
            }
            decoder.read_exact(&mut packet[1..])?;
            let stamp = i32::from_le_bytes(packet[..4].try_into().unwrap());
            let channel = i32::from_le_bytes(packet[4..8].try_into().unwrap());
            let length = i32::from_le_bytes(packet[8..12].try_into().unwrap());
            ensure!(
                stamp >= previous && (0..=2).contains(&channel) && length >= 0,
                "Invalid demo packet header"
            );
            previous = stamp;
            total = total
                .checked_add(12)
                .and_then(|size| size.checked_add(length as usize))
                .context("Demo size overflow")?;
            ensure!(
                total <= crate::cube2::protocol::MAX_SAVE_TREE,
                "Decoded demo exceeds the save-tree budget"
            );
            let copied = std::io::copy(
                &mut (&mut decoder).take(length as u64),
                &mut std::io::sink(),
            )?;
            ensure!(copied == length as u64, "Truncated demo packet");
            packets += 1;
        }
        ensure!(
            packets > 0 && decoder.into_inner().is_empty(),
            "Demo is empty or contains trailing compressed data"
        );
        Ok(())
    }
    fn avi_chunks(mut bytes: &[u8], depth: usize, found: &mut [bool; 3]) -> Result<()> {
        ensure!(depth <= 16, "AVI chunk nesting exceeds its parser budget");
        while !bytes.is_empty() {
            ensure!(bytes.len() >= 8, "Truncated AVI chunk header");
            let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
            let end = 8usize
                .checked_add(size)
                .context("AVI chunk size overflow")?;
            let next = end.checked_add(size % 2).context("AVI padding overflow")?;
            ensure!(next <= bytes.len(), "Truncated AVI chunk");
            let payload = &bytes[8..end];
            match &bytes[..4] {
                b"LIST" => {
                    ensure!(payload.len() >= 4, "Truncated AVI list");
                    avi_chunks(&payload[4..], depth + 1, found)?;
                }
                b"avih" => {
                    ensure!(payload.len() >= 56, "Truncated AVI video header");
                    let width = u32::from_le_bytes(payload[32..36].try_into().unwrap());
                    let height = u32::from_le_bytes(payload[36..40].try_into().unwrap());
                    ensure!(width > 0 && height > 0, "AVI has no video dimensions");
                    found[0] = true;
                }
                b"strh" => {
                    ensure!(payload.len() >= 56, "Truncated AVI stream header");
                    if &payload[..4] == b"vids" {
                        ensure!(
                            u32::from_le_bytes(payload[32..36].try_into().unwrap()) > 0,
                            "AVI has no recorded video frames"
                        );
                        found[1] = true;
                    }
                }
                _ => {
                    if bytes[0].is_ascii_digit()
                        && bytes[1].is_ascii_digit()
                        && matches!(&bytes[2..4], b"db" | b"dc")
                        && size > 0
                    {
                        found[2] = true;
                    }
                }
            }
            bytes = &bytes[next..];
        }
        Ok(())
    }
    fn merge(seed: &[u8], name: &str, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut entries = std::collections::BTreeMap::new();
        if seed.starts_with(b"GPS2") {
            for file in crate::cube2::protocol::save_files(seed)? {
                entries.insert(file.path, file.bytes);
            }
        } else if !seed.is_empty() {
            crate::cube2::protocol::save_seed(seed)?;
            let mut offset = 16usize;
            for (index, path) in ["config.cfg", "init.cfg", "servers.cfg"].iter().enumerate() {
                let at = 4 + index * 4;
                let size = u32::from_le_bytes(seed[at..at + 4].try_into().unwrap()) as usize;
                if size > 0 {
                    entries.insert(*path, &seed[offset..offset + size]);
                }
                offset += size;
            }
        }
        ensure!(
            !entries.contains_key(name),
            "A recording with this name already exists; choose another name"
        );
        entries.insert(name, bytes);
        ensure!(
            entries.len() <= crate::cube2::protocol::MAX_SAVE_FILES,
            "Save-file count quota exceeded"
        );
        let size = entries
            .iter()
            .try_fold(16usize, |total, (name, data)| {
                total
                    .checked_add(16 + name.len())
                    .and_then(|n| n.checked_add(data.len()))
            })
            .context("Save size overflow")?;
        ensure!(
            size <= crate::cube2::protocol::MAX_SAVE_TREE,
            "Save-tree quota exceeded"
        );
        let mut output = Vec::with_capacity(size);
        output.extend(b"GPS2");
        output.extend((entries.len() as u32).to_le_bytes());
        output.extend(((size - 16) as u32).to_le_bytes());
        output.extend([0; 4]);
        for (name, data) in entries {
            output.extend((name.len() as u32).to_le_bytes());
            output.extend((data.len() as u32).to_le_bytes());
            output.extend([0; 8]);
            output.extend(name.as_bytes());
            output.extend(data);
        }
        crate::cube2::protocol::save_files(&output)?;
        Ok(output)
    }
    fn write_new(output: &Path, bytes: &[u8]) -> Result<()> {
        let name = output.file_name().context("Choose an output filename")?;
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent)?;
        let directory = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent)?;
        let pinned = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
        let mut temporary = tempfile::NamedTempFile::new_in(&pinned)?;
        temporary.write_all(bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(pinned.join(name))
            .map_err(|error| error.error)
            .context("Export requires a new filename; existing files are never replaced")?;
        directory.sync_all()?;
        Ok(())
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn demo() -> Vec<u8> {
            let mut raw = b"SAUERBRATEN_DEMO".to_vec();
            raw.extend(1u32.to_le_bytes());
            raw.extend(260u32.to_le_bytes());
            for value in [0u32, 1, 2] {
                raw.extend(value.to_le_bytes());
            }
            raw.extend([0, 1]);
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&raw).unwrap();
            encoder.finish().unwrap()
        }
        #[test]
        fn imported_demo_checks_crc_version_packets_truncations_and_trailing_data() {
            let valid = demo();
            validate(&valid, "demo").unwrap();
            let mut raw = Vec::new();
            flate2::read::GzDecoder::new(valid.as_slice())
                .read_to_end(&mut raw)
                .unwrap();
            for (offset, value) in [
                (16, 2u32),
                (20, 259),
                (24, u32::MAX),
                (28, 3),
                (32, u32::MAX),
                (32, crate::cube2::protocol::MAX_SAVE_TREE as u32),
            ] {
                let mut changed = raw.clone();
                changed[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(&changed).unwrap();
                assert!(
                    validate(&encoder.finish().unwrap(), "demo").is_err(),
                    "invalid field at {offset}"
                );
            }

            for length in 0..valid.len() {
                assert!(validate(&valid[..length], "demo").is_err());
            }
            let mut bad = valid.clone();
            let last = bad.len() - 1;
            bad[last] ^= 1;
            assert!(validate(&bad, "demo").is_err());
            let mut bad = valid.clone();
            bad.extend(b"trailing");
            assert!(validate(&bad, "demo").is_err());
            let mut bad = valid.clone();
            bad.extend(&valid);
            assert!(validate(&bad, "demo").is_err());
            assert!(validate(b"not an AVI", "movie").is_err());
            assert!(validate(b"RIFF\x04\0\0\0AVI ", "movie").is_err());
        }
        #[test]
        fn original_avi_zero_global_count_uses_stream_count_and_real_frame_chunk() {
            fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
                let mut out = tag.to_vec();
                out.extend((data.len() as u32).to_le_bytes());
                out.extend(data);
                if data.len() % 2 != 0 {
                    out.push(0);
                }
                out
            }
            let mut main = [0u8; 56];
            main[32..36].copy_from_slice(&320u32.to_le_bytes());
            main[36..40].copy_from_slice(&240u32.to_le_bytes());
            let mut stream = [0u8; 56];
            stream[..4].copy_from_slice(b"vids");
            stream[32..36].copy_from_slice(&1u32.to_le_bytes());
            let mut body = b"AVI ".to_vec();
            body.extend(chunk(b"avih", &main));
            body.extend(chunk(b"strh", &stream));
            body.extend(chunk(b"00dc", &[1, 2, 3]));
            let valid = chunk(b"RIFF", &body);
            validate(&valid, "movie").unwrap();
            for length in 0..valid.len() {
                assert!(validate(&valid[..length], "movie").is_err());
            }
            stream[32..36].copy_from_slice(&0u32.to_le_bytes());
            let mut body = b"AVI ".to_vec();
            body.extend(chunk(b"avih", &main));
            body.extend(chunk(b"strh", &stream));
            body.extend(chunk(b"00dc", &[1, 2, 3]));
            assert!(validate(&chunk(b"RIFF", &body), "movie").is_err());
        }
        #[test]
        fn import_merge_preserves_all_files_and_legacy_preferences_without_replacement() {
            let bytes = demo();
            let original = crate::cube2::protocol::test_tree(&[
                ("autoexec.cfg", b"private startup commands"),
                ("config.cfg", b"preferences"),
                ("demo/old.dmo", b"original"),
            ]);
            let merged = merge(&original, "demo/new.dmo", &bytes).unwrap();
            let entries = crate::cube2::protocol::save_files(&merged).unwrap();
            assert_eq!(entries.len(), 4);
            assert_eq!(entries[0].path, "autoexec.cfg");
            assert_eq!(entries[0].bytes, b"private startup commands");
            assert_eq!(entries[1].bytes, b"preferences");
            assert_eq!(entries[2].bytes, bytes);
            assert_eq!(entries[3].bytes, b"original");
            assert!(merge(&original, "demo/old.dmo", &bytes).is_err());
            assert!(merge(&original, "../bad.dmo", &bytes).is_err());
            let mut legacy = b"GPS1".to_vec();
            for size in [3u32, 3, 3] {
                legacy.extend(size.to_le_bytes());
            }
            legacy.extend(b"foobarbaz");
            let migrated = merge(&legacy, "demo/new.dmo", &bytes).unwrap();
            let entries = crate::cube2::protocol::save_files(&migrated).unwrap();
            assert_eq!(
                entries
                    .iter()
                    .find(|f| f.path == "config.cfg")
                    .unwrap()
                    .bytes,
                b"foo"
            );
            assert_eq!(
                entries.iter().find(|f| f.path == "init.cfg").unwrap().bytes,
                b"bar"
            );
            assert_eq!(
                entries
                    .iter()
                    .find(|f| f.path == "servers.cfg")
                    .unwrap()
                    .bytes,
                b"baz"
            );
        }
        #[test]
        fn import_merge_keeps_existing_file_count_and_tree_quotas() {
            let names = (0..crate::cube2::protocol::MAX_SAVE_FILES)
                .map(|index| format!("demo/{index:03}.dmo"))
                .collect::<Vec<_>>();
            let records = names
                .iter()
                .map(|name| (name.as_str(), b"kept".as_slice()))
                .collect::<Vec<_>>();
            let saturated = crate::cube2::protocol::test_tree(&records);
            assert!(merge(&saturated, "demo/extra.dmo", &demo()).is_err());
            let large = vec![0; crate::cube2::protocol::MAX_SAVE_FILE];
            let seed = crate::cube2::protocol::test_tree(&[("movies/first.avi", &large)]);
            assert!(merge(&seed, "movies/second.avi", &large).is_err());
            assert_eq!(
                crate::cube2::protocol::save_files(&seed).unwrap()[0].bytes,
                large
            );
        }
        #[test]
        fn export_is_exact_and_never_replaces_files_or_links() {
            let root = tempfile::tempdir().unwrap();
            let output = root.path().join("movie.avi");
            write_new(&output, b"exact recording").unwrap();
            assert_eq!(fs::read(&output).unwrap(), b"exact recording");
            assert!(write_new(&output, b"overwrite").is_err());
            let link = root.path().join("linked.avi");
            std::os::unix::fs::symlink(&output, &link).unwrap();
            assert!(write_new(&link, b"overwrite").is_err());
            assert_eq!(fs::read(&output).unwrap(), b"exact recording");
            assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
        }
        #[test]
        fn only_recording_extensions_are_listed_after_complete_validation() {
            let seed = crate::cube2::protocol::test_tree(&[
                ("Movie.AVI", b"movie"),
                ("autoexec.cfg", b"private startup commands"),
                ("config.cfg", b"private settings"),
                ("recordings/one.dMo", b"demo"),
            ]);
            let names = files(&seed)
                .unwrap()
                .into_iter()
                .filter_map(|file| kind(file.path).map(|_| file.path))
                .collect::<Vec<_>>();
            assert_eq!(names, ["Movie.AVI", "recordings/one.dMo"]);
            assert!(files(&crate::cube2::protocol::test_tree(&[(
                "../movie.avi",
                b"escape"
            )]))
            .is_err());
            assert!(files(b"corrupt save").is_err());
        }
    }
}
