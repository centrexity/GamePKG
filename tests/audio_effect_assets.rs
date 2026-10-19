#[path = "common/music_fixture.rs"]
mod music_fixture;
use gamepkg::{
    audio_voices::{Clip, Mixer},
    memory_budget::Budget,
};
use std::io::{Read, Seek, SeekFrom, Write};

fn fixture() -> (tempfile::TempDir, Vec<i16>, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let samples = (0..4098).map(|i| (i * 7919) as i16).collect::<Vec<_>>();
    let size = samples.len() as u32 * 2;
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\0\0\0\0".to_vec();
    bytes[4..8].copy_from_slice(&(size + 36).to_le_bytes());
    bytes[40..44].copy_from_slice(&size.to_le_bytes());
    bytes.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
    let path = temporary.path().join("effect.wav");
    std::fs::write(&path, bytes).unwrap();
    (temporary, samples, path)
}
#[test]
fn signed_effect_requires_permission_verification_and_scoped_asset_then_plays_exact_stereo() {
    let (_input, samples, path) = fixture();
    let (package_root, mut package) = music_fixture::signed_music_package(&[path]);
    let budget = Budget::new(32768);
    let archive = package_root.path().join("private-music-fixture.gamepkg");
    let mut unverified = gamepkg::archive::Package::open(&archive).unwrap();
    assert!(Clip::from_verified_asset(&mut unverified, "music/effect.wav", true, &budget).is_err());
    assert!(Clip::from_verified_asset(&mut package, "music/effect.wav", false, &budget).is_err());
    for path in [
        "../music/effect.wav",
        "/music/effect.wav",
        "music\\effect.wav",
        "music/missing.wav",
    ] {
        assert!(Clip::from_verified_asset(&mut package, path, true, &budget).is_err());
    }
    assert_eq!(budget.used(), 0);
    let clip = Clip::from_verified_asset(&mut package, "music/effect.wav", true, &budget).unwrap();
    let retained = budget.used();
    assert!(retained >= (samples.len() * 2) as u64);
    assert!(retained < (samples.len() * 2 + 256) as u64);
    let mut mixer = Mixer::default();
    mixer.play(clip.clone());
    drop(clip);
    assert_eq!(budget.used(), retained);
    for frame in samples.as_chunks::<2>().0 {
        assert_eq!(mixer.frame([0; 2]), *frame);
    }
    assert_eq!(mixer.active(), 0);
    assert_eq!(budget.used(), 0);
}
#[test]
fn post_verification_corruption_cannot_publish_a_clip_or_leak_its_memory() {
    let (_input, _samples, path) = fixture();
    let (package_root, mut package) = music_fixture::signed_music_package(&[path]);
    let archive = package_root.path().join("private-music-fixture.gamepkg");
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
    let entry = zip.by_name("assets/music/effect.wav").unwrap();
    let offset = entry.data_start() + 44;
    drop(entry);
    drop(zip);
    let mut changed = std::fs::OpenOptions::new()
        .write(true)
        .open(&archive)
        .unwrap();
    changed.seek(SeekFrom::Start(offset)).unwrap();
    changed.write_all(&[1]).unwrap();
    changed.flush().unwrap();
    let budget = Budget::new(32768);
    let error = Clip::from_verified_asset(&mut package, "music/effect.wav", true, &budget)
        .err()
        .unwrap();
    assert!(
        format!("{error:#}").to_lowercase().contains("checksum"),
        "{error:#}"
    );
    assert_eq!(budget.used(), 0);
    changed.seek(SeekFrom::Start(offset)).unwrap();
    changed.write_all(&[0]).unwrap();
    changed.flush().unwrap();
    let clip = Clip::from_verified_asset(&mut package, "music/effect.wav", true, &budget).unwrap();
    drop(clip);
    assert_eq!(budget.used(), 0);
}
#[test]
fn verified_compressed_effect_fails_before_allocating_unaccounted_workspace() {
    let (_input, _samples, path) = fixture();
    let (package_root, _package) = music_fixture::signed_music_package(&[path]);
    let original = package_root.path().join("private-music-fixture.gamepkg");
    let compressed = package_root.path().join("compressed.gamepkg");
    let mut input = zip::ZipArchive::new(std::fs::File::open(original).unwrap()).unwrap();
    let mut output = zip::ZipWriter::new(std::fs::File::create(&compressed).unwrap());
    for index in 0..input.len() {
        let mut entry = input.by_index(index).unwrap();
        let method = if entry.name() == "assets/music/effect.wav" {
            zip::CompressionMethod::Zstd
        } else {
            zip::CompressionMethod::Stored
        };
        output
            .start_file(
                entry.name(),
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        output.write_all(&bytes).unwrap();
    }
    output.finish().unwrap();
    let mut package = gamepkg::archive::Package::open(&compressed).unwrap();
    gamepkg::verify::verify_embedded(&mut package).unwrap();
    let budget = Budget::new(32768);
    let error = Clip::from_verified_asset(&mut package, "music/effect.wav", true, &budget)
        .err()
        .unwrap();
    assert!(error.to_string().contains("decompressor memory accounting"));
    assert_eq!(budget.used(), 0);
}

#[test]
#[ignore = "requires privately generated original-rate SDL effect WAVs"]
fn all_original_recordings_load_from_signed_assets_inside_one_effect_pool() {
    use sha2::{Digest, Sha256};
    let root = std::path::PathBuf::from(std::env::var_os("GAMEPKG_EFFECT_WAV_DIRECTORY").unwrap());
    let names = [
        "bounce.wav",
        "glass.wav",
        "launch.wav",
        "explosion.wav",
        "life-loss.wav",
    ];
    let paths = names.map(|name| root.join(name));
    let (_package_root, mut package) = music_fixture::signed_music_package(&paths);
    let parent = Budget::new(4 * 1048576);
    let pool = parent.partition(4 * 1048576).unwrap();
    let clips = names.map(|name| {
        Clip::from_verified_asset(&mut package, &format!("music/{name}"), true, &pool).unwrap()
    });
    let retained = pool.used();
    assert!(retained < 4 * 1048576);
    let mut mixer = Mixer::default();
    for (name, clip) in names.iter().zip(&clips) {
        assert_eq!(mixer.play(clip.clone()), Some(0));
        let bytes = std::fs::read(root.join(name)).unwrap();
        let mut hash = Sha256::new();
        let frames = (bytes.len() - 44) / 4;
        for _ in 0..frames {
            for sample in mixer.frame([0; 2]) {
                hash.update(sample.to_le_bytes());
            }
        }
        assert_eq!(
            hex::encode(hash.finalize()),
            hex::encode(Sha256::digest(&bytes[44..])),
            "{name}"
        );
        assert_eq!(mixer.active(), 0);
        assert_eq!(pool.used(), retained);
        println!("Original-rate effect {name}: {frames} exact stereo frames");
    }
    // Active voices keep their pool's parent charge after cache shutdown.
    for clip in &clips[..4] {
        assert!(mixer.play(clip.clone()).is_some());
    }
    assert!(mixer.play(clips[4].clone()).is_none());
    drop(clips);
    drop(pool);
    assert_eq!(parent.used(), 4 * 1048576);
    mixer.stop();
    assert_eq!(parent.used(), 0);
}
