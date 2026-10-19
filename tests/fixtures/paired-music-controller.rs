// Appended to the existing oracle's unchanged shim prefix by the maintainer tool.
mod ported {
    include!(env!("GAMEPKG_PORT_MUSIC_RS"));
}
thread_local! {static FAILED: std::cell::Cell<bool> = const {std::cell::Cell::new(false)};}
mod gamepkg {
    pub fn asset_read(_: &str, _: u32, _: &mut [u8]) -> Option<u32> {
        None
    }
    pub fn music_volume(value: u32) -> bool {
        super::mixer::Music::set_volume(value as i32);
        true
    }
    pub fn music_play(path: &str) -> bool {
        match super::mixer::Music::from_file(format!("assets/{path}")) {
            Ok(song) => {
                super::FAILED.with(|f| f.set(false));
                song.play(1).unwrap();
                true
            }
            Err(_) => {
                super::FAILED.with(|f| f.set(true));
                false
            }
        }
    }
    pub fn music_pause(value: bool) -> bool {
        if value {
            super::mixer::Music::pause();
        } else {
            super::mixer::Music::resume();
        }
        true
    }
    pub fn music_stop() -> bool {
        super::FAILED.with(|f| f.set(false));
        super::mixer::Music::halt();
        true
    }
    pub fn music_finished() -> bool {
        !super::mixer::Music::is_playing() && !super::FAILED.with(|f| f.get())
    }
    pub fn music_failed() -> bool {
        super::FAILED.with(|f| f.get())
    }
}
mod physics {
    pub fn below(bound: u64) -> u64 {
        super::Rng::gen_range(&mut super::Random, 0..bound as usize) as u64
    }
}
trait Control {
    fn begin(&mut self) {}
    fn play(&mut self);
    fn stop(&mut self);
    fn update(&mut self);
    fn gain(&mut self, n: i32);
    fn mute(&mut self, value: bool);
    fn toggle(&mut self);
    fn volume(&self) -> i32;
    fn muted(&self) -> bool;
}
impl Control for original::AudioManager {
    fn play(&mut self) {
        self.play_music()
    }
    fn stop(&mut self) {
        self.stop_music()
    }
    fn update(&mut self) {
        self.update()
    }
    fn gain(&mut self, n: i32) {
        self.set_music_volume(n)
    }
    fn mute(&mut self, v: bool) {
        self.set_music_muted(v)
    }
    fn toggle(&mut self) {
        self.toggle_music_mute()
    }
    fn volume(&self) -> i32 {
        self.get_music_volume()
    }
    fn muted(&self) -> bool {
        self.is_music_muted()
    }
}
impl Control for ported::Music {
    fn begin(&mut self) {
        self.begin_tick()
    }
    fn play(&mut self) {
        self.play_music()
    }
    fn stop(&mut self) {
        self.stop_music()
    }
    fn update(&mut self) {
        self.update()
    }
    fn gain(&mut self, n: i32) {
        self.set_music_volume(n)
    }
    fn mute(&mut self, v: bool) {
        self.set_music_muted(v)
    }
    fn toggle(&mut self) {
        self.toggle_music_mute()
    }
    fn volume(&self) -> i32 {
        self.get_music_volume()
    }
    fn muted(&self) -> bool {
        self.is_music_muted()
    }
}
#[derive(Debug, PartialEq)]
struct Snapshot {
    volume: i32,
    muted: bool,
    playing: bool,
    paused: bool,
    output_gain: i32,
    path: Option<String>,
    draws: Vec<usize>,
}
struct Observer {
    gain: i32,
    path: Option<String>,
}
impl Observer {
    fn snapshot(&mut self, control: &dyn Control) -> Snapshot {
        let mut draws = Vec::new();
        for call in take() {
            if let Some(n) = call.strip_prefix("volume:") {
                self.gain = n.parse().unwrap();
            }
            if call.starts_with("play:") {
                self.path = Some(call.clone());
            }
            if let Some(n) = call.strip_prefix("random:") {
                draws.push(n.parse().unwrap());
            }
        }
        let (playing, paused) = TRACE.with(|t| {
            let t = t.borrow();
            (t.playing, t.paused)
        });
        Snapshot {
            volume: control.volume(),
            muted: control.muted(),
            playing,
            paused,
            output_gain: self.gain,
            path: if playing { self.path.clone() } else { None },
            draws,
        }
    }
}
fn scenario(port: bool) -> Vec<Snapshot> {
    TRACE.with(|t| {
        *t.borrow_mut() = Trace {
            draws: [2, 0, 1, 2, 0].into(),
            ..Trace::default()
        }
    });
    FAILED.with(|f| f.set(false));
    let mut model: Box<dyn Control> = if port {
        Box::new(ported::Music::new(vec![
            "music/z.mp3".into(),
            "music/a.mp3".into(),
            "music/m.mp3".into(),
        ]))
    } else {
        Box::new(original::AudioManager::new().unwrap())
    };
    let mut observer = Observer {
        gain: 128,
        path: None,
    };
    let mut snapshots = vec![observer.snapshot(&*model)];
    for (operation, value) in [
        (0, 0),
        (1, 91),
        (2, 1),
        (0, 0),
        (3, 0),
        (1, 36),
        (2, 0),
        (3, 0),
        (4, 0),
        (3, 0),
        (3, 0),
        (2, 1),
        (4, 0),
        (3, 0),
        (2, 0),
        (3, 0),
        (5, 0),
        (3, 0),
        (0, 0),
        (4, 0),
        (6, 0),
        (3, 0),
        (3, 0),
        (1, 1000),
        (1, -1),
        (7, 0),
        (7, 0),
        (5, 0),
        (2, 1),
        (2, 0),
        (3, 0),
    ] {
        model.begin();
        match operation {
            0 => model.play(),
            1 => model.gain(value),
            2 => model.mute(value != 0),
            3 => model.update(),
            4 => complete(),
            5 => model.stop(),
            6 => TRACE.with(|t| t.borrow_mut().fail = true),
            7 => model.toggle(),
            _ => unreachable!(),
        };
        snapshots.push(observer.snapshot(&*model));
    }
    assert!(TRACE.with(|t| t.borrow().draws.is_empty()));
    snapshots
}
fn main() {
    let expected = scenario(false);
    let actual = scenario(true);
    assert_eq!(actual, expected);
    println!("Port playlist controller matches unchanged original across {} observed startup/gain/mute/pause/completion/stop/failure/retry states and random draws",actual.len());
}
