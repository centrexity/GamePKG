//! Host-side port controller fixture; game code still has no dependencies.
use std::cell::RefCell;
use std::collections::VecDeque;
#[derive(Default)]
struct Trace {
    calls: Vec<String>,
    draws: VecDeque<u64>,
    finished: bool,
    failed: bool,
    accepts: bool,
    playlist: Option<Vec<u8>>,
    fail_offset: Option<u32>,
}
thread_local! {static TRACE: RefCell<Trace> = RefCell::new(Trace { accepts: true, ..Trace::default() });}
mod gamepkg {
    pub fn asset_read(path: &str, offset: u32, output: &mut [u8]) -> Option<u32> {
        if path != "audio/music-list.txt" {
            return None;
        }
        super::TRACE.with(|state| {
            let state = state.borrow();
            if state.fail_offset.is_some_and(|limit| offset >= limit) {
                return None;
            }
            let bytes = state.playlist.as_ref()?;
            let offset = offset as usize;
            if offset > bytes.len() {
                return None;
            }
            let count = output.len().min(bytes.len() - offset);
            output[..count].copy_from_slice(&bytes[offset..offset + count]);
            Some(count as u32)
        })
    }
    pub fn music_volume(value: u32) -> bool {
        super::TRACE.with(|s| s.borrow_mut().calls.push(format!("volume:{value}")));
        true
    }
    pub fn music_play(path: &str) -> bool {
        super::TRACE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(format!("play:{path}"));
            s.accepts
        })
    }
    pub fn music_pause(paused: bool) -> bool {
        super::TRACE.with(|s| s.borrow_mut().calls.push(format!("pause:{paused}")));
        true
    }
    pub fn music_stop() -> bool {
        super::TRACE.with(|s| s.borrow_mut().calls.push("stop".into()));
        true
    }
    pub fn music_finished() -> bool {
        super::TRACE.with(|s| s.borrow().finished)
    }
    pub fn music_failed() -> bool {
        super::TRACE.with(|s| s.borrow().failed)
    }
}
mod physics {
    pub fn below(bound: u64) -> u64 {
        super::TRACE.with(|s| {
            let n = s
                .borrow_mut()
                .draws
                .pop_front()
                .expect("unexpected random draw");
            assert!(n < bound);
            n
        })
    }
}
#[path = "../ports/arkanoo/source/src/music.rs"]
mod music;
fn reset(draws: &[u64]) {
    TRACE.with(|s| {
        *s.borrow_mut() = Trace {
            accepts: true,
            draws: draws.iter().copied().collect(),
            ..Trace::default()
        }
    });
}
fn calls() -> Vec<String> {
    TRACE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
#[test]
fn saved_startup_submits_one_track_and_preserves_resume_when_initially_muted() {
    for muted in [false, true] {
        reset(&[0]);
        let mut music = music::Music::new(vec!["music/a.mp3".into()]);
        music.begin_tick();
        music.start_saved(32, muted);
        let trace = calls();
        assert_eq!(
            trace
                .iter()
                .filter(|call| call.starts_with("play:"))
                .count(),
            1
        );
        assert_eq!(trace[0], "volume:32");
        assert_eq!(music.get_music_volume(), 32);
        assert_eq!(music.is_music_muted(), muted);
        if muted {
            assert_eq!(&trace[trace.len() - 2..], ["volume:0", "pause:true"]);
        }
        music.begin_tick();
        music.set_music_muted(false);
        music.update();
        let trace = calls();
        assert!(!trace.iter().any(|call| call.starts_with("play:")));
        if muted {
            assert_eq!(trace, ["volume:32", "pause:false"]);
        }
    }
}
#[test]
fn startup_mute_resume_and_completion_follow_original_playlist_control() {
    reset(&[1, 0]);
    let mut music = music::Music::new(vec!["music/b.mp3".into(), "music/a.mp3".into()]);
    music.begin_tick();
    music.play_music();
    assert_eq!(calls(), ["volume:64", "play:music/b.mp3"]);
    music.set_music_volume(33);
    music.set_music_muted(true);
    music.play_music();
    music.update();
    assert_eq!(calls(), ["volume:33", "volume:0", "pause:true"]);
    music.begin_tick();
    music.set_music_muted(false);
    music.update();
    assert_eq!(calls(), ["volume:33", "pause:false"]);
    TRACE.with(|s| s.borrow_mut().finished = true);
    music.begin_tick();
    music.update();
    assert_eq!(calls(), ["volume:33", "play:music/a.mp3"]);
    music.update();
    assert!(
        calls().is_empty(),
        "staged replacement read old completion twice"
    );
    music.stop_music();
    music.begin_tick();
    music.update();
    assert_eq!(calls(), ["stop"]);
}
#[test]
fn empty_playlist_clamps_and_inactive_mute_do_not_issue_invalid_pause() {
    reset(&[]);
    let mut music = music::Music::from_package();
    assert!(!music.has_songs());
    assert!(!music.has_playlist());
    music.play_music();
    music.set_music_volume(-1);
    music.toggle_music_mute();
    music.toggle_music_mute();
    music.set_music_volume(999);
    music.update();
    assert_eq!(calls(), ["volume:0", "volume:0", "volume:0", "volume:128"]);
    assert_eq!(music.get_music_volume(), 128);
    assert!(!music.is_music_muted());
}
#[test]
fn failed_or_refused_play_retries_with_fresh_original_random_selection() {
    reset(&[0, 1, 0]);
    let mut music = music::Music::new(vec!["music/a.mp3".into(), "music/b.mp3".into()]);
    music.play_music();
    calls();
    music.begin_tick();
    TRACE.with(|s| s.borrow_mut().failed = true);
    music.update();
    assert_eq!(calls(), ["volume:64", "play:music/b.mp3"]);
    music.update();
    assert!(calls().is_empty());
    music.begin_tick();
    TRACE.with(|s| {
        let mut s = s.borrow_mut();
        s.failed = false;
        s.accepts = false;
        s.finished = true;
    });
    music.update();
    assert_eq!(calls(), ["volume:64", "play:music/a.mp3"]);
}

#[test]
fn interrupted_playlist_read_retains_packaged_audio_without_partial_music() {
    for offset in [0, 1024] {
        reset(&[]);
        TRACE.with(|state| {
            let mut state = state.borrow_mut();
            state.playlist = Some(b"music/a.mp3\n".repeat(200));
            state.fail_offset = Some(offset);
        });
        let mut music = music::Music::from_package();
        assert_eq!(music.has_playlist(), offset != 0);
        assert!(
            !music.has_songs(),
            "Partial input must not become a playable playlist"
        );
        music.play_music();
        music.update();
        assert!(calls().is_empty());
    }
}

#[test]
fn package_playlist_is_sorted_portable_utf8_and_bounded_across_reads() {
    reset(&[1]);
    TRACE.with(|state|state.borrow_mut().playlist=Some(b"music/z.mp3\nmusic/a..mp3\nmusic/../escape.mp3\nmusic/sub/m.mp3\nmusic/other.wav\nmusic/b.mp3\n".to_vec()));
    let mut music = music::Music::from_package();
    assert!(music.has_songs());
    assert!(music.has_playlist());
    music.play_music();
    assert_eq!(calls(), ["volume:64", "play:music/b.mp3"]);
    reset(&[0]);
    let mut playlist = Vec::new();
    for _ in 0..150 {
        playlist.extend_from_slice("music/café.mp3\n".as_bytes());
    }
    TRACE.with(|state| state.borrow_mut().playlist = Some(playlist));
    let mut music = music::Music::from_package();
    music.play_music();
    assert_eq!(calls(), ["volume:64", "play:music/café.mp3"]);
    for invalid in [vec![255], vec![b'a'; 1048577], Vec::new()] {
        reset(&[]);
        TRACE.with(|state| state.borrow_mut().playlist = Some(invalid));
        let mut music = music::Music::from_package();
        assert!(!music.has_songs());
        assert!(
            music.has_playlist(),
            "damaged or empty music must not select the legacy effect reader"
        );
        music.play_music();
        music.update();
        assert!(calls().is_empty());
    }
}
