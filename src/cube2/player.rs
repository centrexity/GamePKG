//! Signed-package playback through the cached reviewed engine.
use anyhow::{ensure, Context, Result};
use std::{path::Path, time::Duration};
pub(crate) struct Player {
    session: Option<super::session::Session>,
    save: Option<super::save::Store>,
    worker: Option<super::runner::Worker>,
    events: Vec<Vec<u8>>,
    pub frame: crate::drawing::Frame,
    pub audio: Vec<i16>,
    pub frame_size: (u32, u32),
    requested_size: (u32, u32),
    desktop: u32,
    desktop_pressed: u32,
    dx: i32,
    dy: i32,
    closed: bool,
    pub exited: bool,
    input_error: Option<String>,
}
impl Player {
    pub fn load(
        root: &Path,
        manifest: &crate::manifest::Manifest,
        digest: &str,
        settings: &crate::settings::Settings,
    ) -> Result<Self> {
        let memory = manifest
            .limits
            .hard_memory_bytes
            .min(settings.memory_ceiling_mib * 1048576);
        ensure!(memory>=super::MEMORY, "Cube 2 needs at least 2048 MiB of address space. Increase Memory ceiling in Settings before playing.");
        let data = crate::game_data::load(root, manifest)?
            .context("Choose the original Sauerbraten game-data folder on this game's page")?;
        let save = super::save::Store::open(
            root,
            &manifest.id,
            &manifest.creator_key,
            manifest.save_schema,
        )?;
        let (permissions, commands) = crate::server_selections::effective(root, manifest)?;
        let selection = if manifest.uses_selected_servers() {
            crate::server_selections::load(root, manifest)?
        } else {
            Default::default()
        };
        let master = if selection.master.is_empty() {
            None
        } else {
            Some(super::discovery::Master::resolve(
                &selection.master,
                selection.allow_local,
            )?)
        };
        let config = super::network::Config {
            permissions,
            master,
        };
        let mut session = super::session::Session::start_cached(
            &data,
            memory,
            Duration::from_secs(60),
            &save.seed,
            &super::cache_path(root, digest),
            &config,
            commands,
        )?;
        let first = session.tick(0, 0, 0)?;
        session.set_deadline(Duration::from_millis(settings.tick_deadline_ms))?;
        Ok(Self {
            session: Some(session),
            save: Some(save),
            worker: None,
            events: Vec::new(),
            frame_size: (first.width, first.height),
            requested_size: (first.width, first.height),
            frame: first.drawing_frame()?,
            audio: Vec::new(),
            desktop: 0,
            desktop_pressed: 0,
            dx: 0,
            dy: 0,
            closed: false,
            exited: false,
            input_error: None,
        })
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.requested_size != (width, height) {
            self.events.retain(|e| !e.starts_with(b"GPD1"));
            self.queue(super::protocol::resize_event(width, height));
            self.requested_size = (width, height);
        }
    }
    pub fn desktop(&mut self, buttons: u32, dx: i32, dy: i32) {
        let buttons = buttons & super::protocol::MAX_BUTTONS;
        self.desktop_pressed |= buttons & !self.desktop;
        self.desktop = buttons;
        self.dx = self.dx.saturating_add(dx).clamp(-2048, 2048);
        self.dy = self.dy.saturating_add(dy).clamp(-2048, 2048);
    }
    pub fn clear_input(&mut self) {
        self.events.retain(|e| e.starts_with(b"GPD1"));
        if let Some(worker) = &self.worker {
            worker.reset();
        } else {
            self.events.push(super::protocol::RESET_INPUT.to_vec());
        }
        self.desktop = 0;
        self.desktop_pressed = 0;
        self.dx = 0;
        self.dy = 0;
    }
    pub fn key(&mut self, scancode: u32, pressed: bool, repeat: bool, modifiers: u32) {
        self.queue(super::protocol::key_event(
            scancode, pressed, repeat, modifiers,
        ));
    }
    pub fn modifiers(&mut self, modifiers: u32) {
        self.queue(super::protocol::modifiers_event(modifiers));
    }
    pub fn mouse_button(&mut self, button: u32, pressed: bool, modifiers: u32) {
        self.queue(super::protocol::mouse_button_event(
            button, pressed, modifiers,
        ));
    }
    pub fn wheel(&mut self, x: i32, y: i32, modifiers: u32) {
        self.queue(super::protocol::wheel_event(x, y, modifiers));
    }
    pub fn text(&mut self, text: &str) {
        // SDL's 32-byte text field includes a NUL. Split at UTF-8 boundaries.
        let mut remaining = text;
        while !remaining.is_empty() {
            let mut end = remaining.len().min(31);
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            self.queue(super::protocol::text_event(&remaining[..end]));
            if self.input_error.is_some() {
                break;
            }
            remaining = &remaining[end..];
        }
    }
    fn queue(&mut self, packet: Result<Vec<u8>>) {
        match packet {
            Ok(packet) if self.events.len() < super::protocol::MAX_EVENTS => {
                self.events.push(packet)
            }
            Ok(_) => self.input_error = Some("Cube 2 input event queue exceeded its bound".into()),
            Err(error) => self.input_error = Some(error.to_string()),
        }
    }
    pub fn background(&mut self, progress: Option<crate::progress::Session>) -> Result<()> {
        if self.worker.is_none() {
            let session = self
                .session
                .take()
                .context("Cube 2 session is unavailable")?;
            let save = self.save.take().context("Cube 2 save is unavailable")?;
            self.worker = Some(super::runner::Worker::start(session, save, progress)?);
        }
        Ok(())
    }
    #[cfg(all(feature = "audio", feature = "gui"))]
    pub fn audio_sink(&self, sink: Option<crate::audio::Sink>) {
        if let Some(worker) = &self.worker {
            worker.audio(sink);
        }
    }
    pub fn step(&mut self, input: u32) -> Result<()> {
        if let Some(error) = self.input_error.take() {
            anyhow::bail!(error);
        }
        use crate::input_buttons as b;
        let mut buttons = self.desktop | self.desktop_pressed;
        self.desktop_pressed = 0;
        for (mask, bit) in [
            (b::UP, 0),
            (b::DOWN, 1),
            (b::LEFT, 2),
            (b::RIGHT, 3),
            (b::B, 16),
            (b::Y, 4),
            (b::A, 17),
            (b::START, 6),
            (b::SELECT, 7),
            (b::L, 5),
        ] {
            if b::held(input, mask) || b::pressed(input, mask) {
                buttons |= 1 << bit;
            }
        }
        let dx = std::mem::take(&mut self.dx);
        let dy = std::mem::take(&mut self.dy);
        let events = std::mem::take(&mut self.events);
        if let Some(worker) = &self.worker {
            worker.submit(buttons, dx, dy, events)?;
            let output = worker.poll()?;
            self.exited = output.exited;
            if let Some(error) = output.error {
                anyhow::bail!(error);
            }
            if let Some(frame) = output.frame {
                self.frame_size = output.frame_size;
                self.frame = frame;
            }
            self.audio = output.audio;
            return Ok(());
        }
        let session = self
            .session
            .as_mut()
            .context("Cube 2 session is unavailable")?;
        for event in events {
            session.queue(event)?;
        }
        let result = session.tick_update(buttons, dx, dy);
        let Some(frame) = result? else {
            self.exited = true;
            self.audio.clear();
            return Ok(());
        };
        self.frame_size = (frame.width, frame.height);
        self.frame = frame.drawing_frame()?;
        self.audio = frame.audio;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if let Some(worker) = &mut self.worker {
            return worker.finish();
        }
        let session = self
            .session
            .as_mut()
            .context("Cube 2 session is unavailable")?;
        let snapshot = session.snapshot();
        let close = session.close();
        self.save
            .as_mut()
            .context("Cube 2 save is unavailable")?
            .commit(&snapshot?)?;
        close
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
