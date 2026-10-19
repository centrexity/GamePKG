//! Maintainer control-flow oracle: includes pinned original source unchanged.
extern crate self as rand;
extern crate self as sdl2;
use std::{cell::RefCell, collections::VecDeque};
#[derive(Default)]
struct Trace {
    calls: Vec<String>,
    draws: VecDeque<usize>,
    playing: bool,
    paused: bool,
    fail: bool,
}
thread_local! { static TRACE: RefCell<Trace> = RefCell::new(Trace::default()); }
fn trace(call: String) {
    TRACE.with(|state| state.borrow_mut().calls.push(call));
}
fn take() -> Vec<String> {
    TRACE.with(|state| std::mem::take(&mut state.borrow_mut().calls))
}
fn expect(calls: &[&str]) {
    assert_eq!(take(), calls);
}
pub struct Random;
pub fn thread_rng() -> Random {
    Random
}
pub trait Rng {
    fn gen_range(&mut self, range: std::ops::Range<usize>) -> usize;
}
impl Rng for Random {
    fn gen_range(&mut self, range: std::ops::Range<usize>) -> usize {
        let value = TRACE.with(|state| {
            state
                .borrow_mut()
                .draws
                .pop_front()
                .expect("missing random fixture")
        });
        assert!(range.contains(&value));
        trace(format!("random:{value}"));
        value
    }
}
pub mod mixer {
    use super::{trace, TRACE};
    pub const AUDIO_S16LSB: u16 = 0x8010;
    pub fn open_audio(rate: i32, format: u16, channels: i32, frames: i32) -> Result<(), String> {
        trace(format!("open:{rate}:{format}:{channels}:{frames}"));
        Ok(())
    }
    pub fn allocate_channels(count: i32) -> i32 {
        trace(format!("channels:{count}"));
        count
    }
    pub struct Chunk;
    impl Chunk {
        pub fn from_file(_: impl AsRef<std::path::Path>) -> Result<Self, String> {
            Ok(Self)
        }
    }
    pub struct Channel(pub i32);
    impl Channel {
        pub fn all() -> Self {
            Self(-1)
        }
        pub fn set_volume(&self, volume: i32) -> i32 {
            trace(format!("sfx-volume:{volume}"));
            volume
        }
        pub fn play(&self, _: &Chunk, loops: i32) -> Result<Self, String> {
            trace(format!("effect:{loops}"));
            Ok(Self(0))
        }
    }
    pub struct Music {
        path: String,
    }
    impl Music {
        pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
            let path = path.as_ref().to_str().unwrap().to_owned();
            trace(format!("load:{path}"));
            if TRACE.with(|state| std::mem::take(&mut state.borrow_mut().fail)) {
                return Err("fixture rejection".into());
            }
            Ok(Self { path })
        }
        pub fn set_volume(volume: i32) -> i32 {
            trace(format!("volume:{volume}"));
            volume
        }
        pub fn play(&self, loops: i32) -> Result<(), String> {
            trace(format!("play:{}:{loops}", self.path));
            TRACE.with(|state| {
                let mut state = state.borrow_mut();
                state.playing = true;
                state.paused = false;
            });
            Ok(())
        }
        pub fn is_playing() -> bool {
            TRACE.with(|state| state.borrow().playing)
        }
        pub fn pause() {
            trace("pause".into());
            TRACE.with(|state| state.borrow_mut().paused = true);
        }
        pub fn resume() {
            trace("resume".into());
            TRACE.with(|state| state.borrow_mut().paused = false);
        }
        pub fn halt() {
            trace("halt".into());
            TRACE.with(|state| {
                let mut state = state.borrow_mut();
                state.playing = false;
                state.paused = false;
            });
        }
    }
}
mod original {
    include!(env!("GAMEPKG_ORIGINAL_AUDIO_RS"));
}
fn complete() {
    TRACE.with(|state| {
        let mut state = state.borrow_mut();
        state.playing = false;
        state.paused = false;
    });
}
fn main() {
    assert_eq!(
        std::env::current_dir().unwrap(),
        std::path::PathBuf::from(
            std::env::var_os("GAMEPKG_ORIGINAL_MUSIC_FIXTURE_HOME")
                .expect("use the private fixture tool")
        )
    );
    TRACE.with(|state| state.borrow_mut().draws.extend([2, 0, 1, 2, 0]));
    let mut music = original::AudioManager::new().unwrap();
    expect(&["open:44100:32784:2:1024", "channels:4", "random:2"]);
    music.play_music();
    expect(&[
        "load:assets/music/z.mp3",
        "volume:64",
        "play:assets/music/z.mp3:1",
    ]);
    music.set_music_volume(91);
    music.set_music_muted(true);
    music.play_music();
    music.update();
    expect(&["volume:91", "volume:0", "pause"]);
    assert!(music.is_music_muted());
    music.set_music_volume(36);
    expect(&["volume:36"]);
    music.set_music_muted(false);
    music.update();
    expect(&["volume:36", "resume"]);
    assert!(!music.is_music_muted());
    assert_eq!(music.get_music_volume(), 36);
    complete();
    music.update();
    expect(&[
        "random:0",
        "load:assets/music/a.mp3",
        "volume:36",
        "play:assets/music/a.mp3:1",
    ]);
    music.update();
    expect(&[]);
    music.set_music_muted(true);
    expect(&["volume:0", "pause"]);
    complete();
    music.update();
    expect(&[]);
    music.set_music_muted(false);
    music.update();
    expect(&[
        "volume:36",
        "resume",
        "random:1",
        "load:assets/music/m.mp3",
        "volume:36",
        "play:assets/music/m.mp3:1",
    ]);
    music.stop_music();
    music.update();
    expect(&["halt"]);
    music.play_music();
    expect(&[
        "load:assets/music/m.mp3",
        "volume:36",
        "play:assets/music/m.mp3:1",
    ]);
    complete();
    TRACE.with(|state| state.borrow_mut().fail = true);
    music.update();
    expect(&["random:2", "load:assets/music/z.mp3"]);
    music.update();
    expect(&[
        "random:0",
        "load:assets/music/a.mp3",
        "volume:36",
        "play:assets/music/a.mp3:1",
    ]);
    music.set_music_volume(1000);
    assert_eq!(music.get_music_volume(), 128);
    expect(&["volume:128"]);
    music.set_music_volume(-1);
    assert_eq!(music.get_music_volume(), 0);
    expect(&["volume:0"]);
    music.toggle_music_mute();
    music.toggle_music_mute();
    expect(&["volume:0", "pause", "volume:0", "resume"]);
    music.set_sfx_volume(71);
    music.set_sfx_muted(true);
    music.set_sfx_muted(false);
    expect(&["sfx-volume:71", "sfx-volume:0", "sfx-volume:71"]);
    assert_eq!(music.get_sfx_volume(), 71);
    assert!(!music.is_sfx_muted());
    music.toggle_sfx_mute();
    music.toggle_sfx_mute();
    expect(&["sfx-volume:0", "sfx-volume:71"]);
    music.play_bounce();
    music.play_oh();
    music.play_load();
    music.play_breaking_glass();
    music.play_explosion();
    expect(&["effect:0", "effect:0", "effect:0", "effect:0", "effect:0"]);
    music.stop_music();
    expect(&["halt"]);
    std::fs::rename("assets/music", "assets/music-present").unwrap();
    let mut empty = original::AudioManager::new().unwrap();
    expect(&["open:44100:32784:2:1024", "channels:4"]);
    empty.play_music();
    empty.update();
    expect(&[]);
    std::fs::rename("assets/music-present", "assets/music").unwrap();
    println!("Unchanged original audio: sorted/random playlist, startup-muted pause/resume, completion, stopped/empty playlists, load retry, gain clamps and all effect controls passed.");
}
