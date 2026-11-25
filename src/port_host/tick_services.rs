//! Per-tick transactions for the shared 2D compatibility profile.
#![cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
use super::{Backend, Error, Result, Session, WindowCommand};
use crate::manifest::Manifest;
use std::collections::BTreeSet;

pub(super) struct Services {
    pub active: bool,
    pub window: crate::window_control::Service,
    pub files: crate::user_files::Service,
    applied_window: crate::window_control::State,
    achievement_count: usize,
    staged: BTreeSet<u32>,
    achievements: BTreeSet<u32>,
    #[cfg(feature = "gui")]
    reported: BTreeSet<u32>,
    pub vsync: bool,
}

impl Services {
    pub fn new(manifest: &Manifest) -> anyhow::Result<Self> {
        Ok(Self {
            active: false,
            window: crate::window_control::Service::with_homepage(
                true,
                manifest.homepage.as_deref(),
            )?,
            files: crate::user_files::Service::new(manifest.permissions.files.clone()),
            applied_window: Default::default(),
            achievement_count: manifest.achievements.len(),
            staged: BTreeSet::new(),
            achievements: BTreeSet::new(),
            #[cfg(feature = "gui")]
            reported: BTreeSet::new(),
            vsync: true,
        })
    }

    #[cfg(feature = "gui")]
    pub fn take_achievements(&mut self) -> Vec<u32> {
        let new: Vec<_> = self
            .achievements
            .difference(&self.reported)
            .copied()
            .collect();
        self.reported.extend(new.iter().copied());
        new
    }

    pub fn achievement(&mut self, index: u32) -> bool {
        if index as usize >= self.achievement_count {
            return false;
        }
        self.staged.insert(index);
        true
    }
}

impl<B: Backend> Session<B> {
    pub(super) fn begin_tick(&mut self, gesture: bool) -> Result<()> {
        let Some(tick) = self.tick.as_mut() else {
            return Ok(());
        };
        self.backend.tick_begin()?;
        let size = [self.window.width, self.window.height];
        tick.window.observe_size(Some(size));
        tick.window.observe_fullscreen(Some(self.window.fullscreen));
        tick.window.observe_vsync(Some(tick.vsync));
        tick.window.begin(self.window.focused && gesture);
        tick.files.begin(self.window.focused && gesture);
        tick.active = true;
        Ok(())
    }

    pub(super) fn finish_tick(&mut self) -> Result<()> {
        let Some(tick) = self.tick.as_mut().filter(|tick| tick.active) else {
            return Ok(());
        };
        let audio = self.backend.tick_finish();
        let before = tick.applied_window;
        tick.window.finish(audio.is_ok());
        tick.files.finish(audio.is_ok());
        tick.active = false;
        if audio.is_ok() {
            tick.achievements.append(&mut tick.staged);
        } else {
            tick.staged.clear();
        }
        let state = tick.window.state();
        tick.applied_window = state;
        tick.vsync = state.vsync.unwrap_or(tick.vsync);
        if state.fullscreen_toggle != before.fullscreen_toggle {
            self.backend
                .window(WindowCommand::Fullscreen(!self.window.fullscreen))?;
        }
        if state.visible != before.visible {
            self.backend
                .window(WindowCommand::CursorVisible(state.visible))?;
        }
        if state.confined != before.confined {
            match self
                .backend
                .window(WindowCommand::CursorLocked(state.confined))
            {
                Ok(()) | Err(Error::Unsupported) => {}
                Err(error) => return Err(error),
            }
        }
        if state.resize_serial != before.resize_serial {
            if let Some([w, h]) = state.resize {
                self.backend.window(WindowCommand::Resize(w, h))?;
            }
        }
        if let Some(url) = tick.window.take_homepage() {
            self.backend.tick_homepage(url);
        }
        if state.closed {
            self.backend.window(WindowCommand::Close)?;
        }
        audio
    }
}
