use gamepkg::{memory_budget::Budget, xbox_disc::VerifiedDisc};
use sha2::{Digest, Sha256};
use std::{io::Read, path::PathBuf, time::Instant};

#[test]
#[ignore = "requires the player's supported ISO through GAMEPKG_TAKEDAWN_DISC; reads it in place"]
fn every_verified_original_disc_file_streams_with_its_exact_manifest_hash() {
    let path =
        PathBuf::from(std::env::var_os("GAMEPKG_TAKEDAWN_DISC").expect("missing private ISO path"));
    let budget = Budget::new(64 * 1024 * 1024);
    let started = Instant::now();
    let media = VerifiedDisc::open(&path, &budget).unwrap();
    let verification = started.elapsed();
    assert_eq!(media.report().verified_files, 716);
    assert_eq!(media.report().verified_bytes, 2476107072);
    assert_eq!(media.report().extra_files, 0);
    let mut directories = vec![String::new()];
    let mut directory_count = 0;
    let mut listed_files = std::collections::HashSet::new();
    while let Some(path) = directories.pop() {
        directory_count += 1;
        for entry in media.directory(&path).unwrap() {
            let full = if path.is_empty() {
                entry.name.clone()
            } else {
                format!("{path}/{}", entry.name)
            };
            if entry.is_directory {
                directories.push(full);
            } else {
                assert!(listed_files.insert(full.to_ascii_lowercase()));
                assert_eq!(media.open_stream(&full).unwrap().len(), entry.size);
            }
        }
    }
    // Independently enumerated by the pinned original Xiso reader:56
    // directories including root, no empty directories, and716 original files.
    assert_eq!(directory_count, 56);
    assert_eq!(listed_files.len(), 716);
    let started = Instant::now();
    let mut files = 0;
    let mut bytes = 0;
    let mut block = [0; 65536];
    for line in include_str!("../engines/takedawn/ntsc-u-retail.manifest")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields = line.splitn(3, ' ').collect::<Vec<_>>();
        assert!(listed_files.contains(&fields[2].to_ascii_lowercase()));
        let mut stream = media.open_stream(fields[2]).unwrap();
        let mut hash = Sha256::new();
        let mut read = 0;
        loop {
            let count = stream.read(&mut block).unwrap();
            if count == 0 {
                break;
            }
            hash.update(&block[..count]);
            read += count as u64;
        }
        assert_eq!(read, fields[1].parse::<u64>().unwrap());
        assert_eq!(read, stream.len());
        assert_eq!(hex::encode(hash.finalize()), fields[0], "{}", fields[2]);
        files += 1;
        bytes += read;
    }
    assert_eq!((files, bytes), (716, 2476107072));
    println!("Verified {files} complete original files and {bytes} bytes through checked streams; admission {verification:?}, streamed hash comparison {:?}, retained budget {} bytes", started.elapsed(), budget.used());
    drop(media);
    assert_eq!(budget.used(), 0);
}
