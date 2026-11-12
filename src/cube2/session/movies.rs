//! Original movie recording must coexist with host-delivered game audio.
use super::*;

fn assert_movie_timing(movie: &[u8]) {
    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }
    fn chunks<'a>(bytes: &'a [u8], out: &mut Vec<(&'a [u8], &'a [u8])>) {
        let mut offset = 0;
        while offset < bytes.len() {
            let size = word(bytes, offset + 4) as usize;
            let id = &bytes[offset..offset + 4];
            let data = &bytes[offset + 8..offset + 8 + size];
            out.push((id, data));
            if id == b"LIST" {
                chunks(&data[4..], out);
            }
            offset += 8 + size + (size & 1);
        }
        assert_eq!(offset, bytes.len());
    }
    assert_eq!(word(movie, 4) as usize + 8, movie.len());
    let mut all = Vec::new();
    chunks(&movie[12..], &mut all);
    let find = |id: &[u8]| all.iter().find(|(kind, _)| *kind == id).unwrap().1;
    let index = find(b"idx1");
    let frames = index
        .as_chunks::<16>()
        .0
        .iter()
        .filter(|entry| &entry[..4] == b"00dc")
        .count() as u32;
    assert!(frames > 0);
    assert_eq!(word(find(b"avih"), 16), frames, "AVI global frame count");
    assert_eq!(word(find(b"dmlh"), 0), frames, "OpenDML frame count");
    assert_eq!(word(find(b"ix00"), 4), frames, "video index entries");
    let mut samples = 0;
    for (_, header) in all.iter().filter(|(id, _)| *id == b"strh") {
        if &header[..4] == b"vids" {
            assert_eq!(word(header, 32), frames, "video stream duration");
        } else if &header[..4] == b"auds" {
            let alignment = word(header, 44);
            assert!(alignment > 0);
            let bytes: u32 = index
                .as_chunks::<16>()
                .0
                .iter()
                .filter(|entry| &entry[..4] == b"01wb")
                .map(|entry| word(entry, 12))
                .sum();
            assert_eq!(bytes % alignment, 0);
            samples = bytes / alignment;
            assert_eq!(
                word(header, 32),
                bytes / alignment,
                "audio stream sample count"
            );
        }
    }
    assert!(samples > 0);
    for (_, header) in all.iter().filter(|(id, _)| *id == b"indx") {
        assert_eq!(word(header, 4), 1, "one bounded RIFF segment");
        let duration = if &header[8..12] == b"00dc" {
            frames
        } else {
            samples
        };
        assert_eq!(word(header, 36), duration, "superindex stream duration");
    }
}
#[test]
#[ignore = "requires original Cube 2 data, compiler/runtime namespaces and FFmpeg tools"]
fn original_movie_recording_keeps_game_audio_during_and_after_capture() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
    let binary = super::super::build::compile().unwrap();
    let config = b"exec data/defaults.cfg\nbind F9 [movie PortMovie]\nbind F10 [movie]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
        || Ok(binary.clone()),
        None,
        "; mode 0; map complex".into(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    let mut baseline = 0;
    for _ in 0..40 {
        baseline += session.tick(0, 0, 0).unwrap().audio.len();
    }
    assert!(
        baseline > 0,
        "Original mixer did not reach host before movie capture"
    );
    session.key(66, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(66, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    for _ in 0..5 {
        session.tick(0, 0, 0).unwrap();
    }
    let mut during = 0;
    for _ in 0..30 {
        during += session.tick(1, 2, 0).unwrap().audio.len();
    }
    assert!(
        during > 0,
        "Original movie capture stole the host audio callback"
    );
    session.key(67, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(67, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    let mut after = 0;
    for _ in 0..30 {
        after += session.tick(0, 0, 0).unwrap().audio.len();
    }
    assert!(
        after > 0,
        "Stopping original movie capture removed host audio delivery"
    );
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let movie = files
        .iter()
        .find(|file| file.path == "PortMovie.avi")
        .expect("Original movie capture silently disappeared from the scoped save")
        .bytes
        .to_vec();
    assert_eq!(&movie[..4], b"RIFF");
    assert_eq!(&movie[8..12], b"AVI ");
    assert_movie_timing(&movie);
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("recording.avi");
    fs::write(&path, &movie).unwrap();
    let probe = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let metadata: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    let streams = metadata["streams"].as_array().unwrap();
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .unwrap();
    assert_eq!(video["width"], 320);
    assert_eq!(video["height"], 240);
    assert!(video["nb_frames"].as_str().unwrap().parse::<u32>().unwrap() > 0);
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio")
        .unwrap();
    assert_eq!(audio["channels"], 2);
    assert_eq!(audio["sample_rate"], "48000");
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    let saved = crate::cube2::recordings::test_import_roundtrip(&saved, "PortMovie.avi", &movie);
    drop(session);
    let mut restored = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &saved,
        || Ok(binary),
        None,
        "; mode 0; map complex".into(),
    )
    .unwrap();
    restored.tick(0, 0, 0).unwrap();
    restored.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..10 {
        restored.tick(0, 0, 0).unwrap();
    }
    let saved = restored.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == "PortMovie.avi")
            .unwrap()
            .bytes,
        movie
    );
}

#[test]
#[ignore = "requires original Cube 2 data, compiler/runtime namespaces and FFmpeg tools"]
fn original_movie_storage_limit_keeps_game_running_and_finishes_valid_avi() {
    let data = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original game data");
    let binary = super::super::build::compile().unwrap();
    let config = b"exec data/defaults.cfg\nbind F9 [moviefps 120; movie PortLimit]\n";
    let mut seed = b"GPS1".to_vec();
    seed.extend((config.len() as u32).to_le_bytes());
    seed.extend([0; 8]);
    seed.extend(config);
    let mut session = Session::start(
        Path::new(&data),
        2 * 1024 * 1048576,
        Duration::from_secs(60),
        &seed,
        || Ok(binary),
        None,
        "; mode 0; map complex".into(),
    )
    .unwrap();
    session.tick(0, 0, 0).unwrap();
    session.set_deadline(Duration::from_millis(500)).unwrap();
    for _ in 0..40 {
        session.tick(0, 0, 0).unwrap();
    }
    session.key(66, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(66, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    let mut audio = 0usize;
    for _ in 0..600 {
        audio += session
            .tick(1, 2, 0)
            .expect("Recording quota terminated the game")
            .audio
            .len();
    }
    assert!(audio > 0);
    assert!(
        String::from_utf8_lossy(&session.diagnostics.lock().unwrap()).contains("storage limit"),
        "Movie did not report its storage limit"
    );
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let movie = files
        .iter()
        .find(|file| file.path == "PortLimit.avi")
        .unwrap()
        .bytes;
    assert!(movie.len() <= protocol::MAX_SAVE_FILE && movie.len() > 15 * 1048576);
    assert_movie_timing(movie);
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("limit.avi");
    fs::write(&path, movie).unwrap();
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&path)
        .args(["-map", "0:v:0", "-map", "0:a:0", "-f", "null", "-"])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
}
