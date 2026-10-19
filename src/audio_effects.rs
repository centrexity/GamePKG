//! Trusted, bounded recorded-effect cache and per-tick control transaction.
//! Shared by desktop native/core/component hosts; no guest receives a device or clip.
use crate::{
    archive::Package,
    audio_voices::{Clip, EffectCommand},
    memory_budget::{Budget, Lease},
};
use anyhow::{ensure, Context, Result};
use std::sync::Arc;

// Match the existing device control queue; cache metadata has fixed storage.
const CAPACITY: usize = 32;
/// Trusted output attachment, never available to game source.
pub type Publisher = Arc<dyn Fn(&[EffectCommand]) -> bool + Send + Sync>;

#[derive(Default)]
pub(crate) struct Runtime {
    service: Option<Box<Service>>,
    budget: Option<Budget>,
    publisher: Option<Publisher>,
    _guest: Option<Lease>,
    _reservation: Option<Lease>,
}
impl Runtime {
    pub(crate) fn new(
        package: Option<&Package>,
        declared: Option<u64>,
        total: u64,
        allowed: bool,
    ) -> Result<(Self, u64)> {
        let Some(bytes) = declared else {
            return Ok((Self::default(), total));
        };
        ensure!(
            allowed && (65536..=4 * 1048576).contains(&bytes),
            "invalid effects memory reservation"
        );
        let guest_bytes = total
            .checked_sub(bytes)
            .filter(|n| *n >= 65536)
            .context("effects exceed the selected memory ceiling")?;
        let parent = Budget::new(total);
        let guest = parent.reserve(guest_bytes)?;
        let budget = parent.partition(bytes)?;
        let reservation = budget
            .reserve((std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64)?;
        let package = package
            .context("recorded effects require a verified package")?
            .try_clone_verified()?;
        let service = Service::new(package, allowed, budget.clone())?;
        Ok((
            Self {
                service: Some(Box::new(service)),
                budget: Some(budget),
                publisher: None,
                _guest: Some(guest),
                _reservation: Some(reservation),
            },
            guest_bytes,
        ))
    }
    pub(crate) fn enabled(&self) -> bool {
        self.service.is_some()
    }
    pub(crate) fn attach(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
    }
    pub(crate) fn playback(&self) -> Result<Option<crate::audio_mix_stream::Playback>> {
        self.budget
            .as_ref()
            .map(crate::audio_mix_stream::Playback::effects_only)
            .transpose()
    }
    pub(crate) fn begin(&mut self) -> Result<()> {
        if let Some(service) = &mut self.service {
            service.begin()?;
        }
        Ok(())
    }
    pub(crate) fn call(&mut self, operation: u32, path: &str, value: u32) -> bool {
        if self.publisher.is_none() {
            return false;
        }
        let Some(service) = &mut self.service else {
            return false;
        };
        match operation {
            0 if value == 0 => service.play(path),
            1 if path.is_empty() => service.volume(value),
            2 if path.is_empty() && value <= 1 => service.pause(value != 0),
            3 if path.is_empty() && value == 0 => service.stop(),
            _ => return false,
        }
        .is_ok()
    }
    pub(crate) fn finish(&mut self, succeeded: bool) -> Result<()> {
        if let Some(service) = &mut self.service {
            let accepted = service.finish(succeeded, |commands| {
                commands.is_empty()
                    || self
                        .publisher
                        .as_ref()
                        .is_some_and(|publish| publish(commands))
            });
            ensure!(
                !succeeded || accepted,
                "recorded effect output rejected the tick's control batch"
            );
        }
        Ok(())
    }
}
struct Recording {
    path: [u8; 1024],
    length: usize,
    clip: Arc<Clip>,
    committed: bool,
}
pub struct Service {
    package: Package,
    recordings: [Option<Recording>; CAPACITY],
    pending: Vec<EffectCommand>,
    active: bool,
    budget: Budget,
    _reservation: Lease,
}
impl Service {
    /// The caller transfers its verified package reader and existing shared
    /// memory budget. This service creates no independent memory allowance.
    pub fn new(package: Package, audio_allowed: bool, budget: Budget) -> Result<Self> {
        ensure!(audio_allowed, "effects require audio permission");
        ensure!(package.is_verified(), "effects require a verified package");
        let reservation = budget.reserve(
            (std::mem::size_of::<Self>() + CAPACITY * std::mem::size_of::<EffectCommand>()) as u64,
        )?;
        Ok(Self {
            package,
            recordings: std::array::from_fn(|_| None),
            pending: Vec::with_capacity(CAPACITY),
            active: false,
            budget,
            _reservation: reservation,
        })
    }
    pub fn begin(&mut self) -> Result<()> {
        ensure!(!self.active, "effect transaction already active");
        self.active = true;
        Ok(())
    }
    fn ready(&self) -> Result<()> {
        ensure!(self.active, "effect transaction is not active");
        ensure!(
            self.pending.len() < CAPACITY,
            "effect control queue is full"
        );
        Ok(())
    }
    pub fn play(&mut self, path: &str) -> Result<()> {
        self.ready()?;
        ensure!(
            crate::archive::valid_path(path),
            "invalid effect asset path"
        );
        let existing = self.recordings.iter().position(|entry| {
            entry
                .as_ref()
                .is_some_and(|recording| &recording.path[..recording.length] == path.as_bytes())
        });
        let slot = match existing {
            Some(index) => index,
            None => {
                let index = self
                    .recordings
                    .iter()
                    .position(Option::is_none)
                    .context("effect recording cache is full")?;
                let clip = Clip::from_verified_asset(&mut self.package, path, true, &self.budget)?;
                let mut name = [0; 1024];
                name[..path.len()].copy_from_slice(path.as_bytes());
                self.recordings[index] = Some(Recording {
                    path: name,
                    length: path.len(),
                    clip,
                    committed: false,
                });
                index
            }
        };
        self.pending.push(EffectCommand::Play(
            self.recordings[slot].as_ref().unwrap().clip.clone(),
        ));
        Ok(())
    }
    pub fn volume(&mut self, volume: u32) -> Result<()> {
        self.ready()?;
        ensure!(volume <= 128, "effect volume must be 0 to 128");
        self.pending.push(EffectCommand::Volume(volume as u8));
        Ok(())
    }
    pub fn pause(&mut self, paused: bool) -> Result<()> {
        self.ready()?;
        self.pending.push(EffectCommand::Pause(paused));
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.ready()?;
        self.pending.push(EffectCommand::Stop);
        Ok(())
    }
    /// Publish only after guest execution and persistence succeed. The trusted
    /// callback must accept the complete batch or leave its output unchanged.
    /// Rejected batches cannot replay and release all newly loaded recordings.
    pub fn finish(
        &mut self,
        succeeded: bool,
        publish: impl FnOnce(&[EffectCommand]) -> bool,
    ) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        let accepted = succeeded && publish(&self.pending);
        self.pending.clear();
        for entry in &mut self.recordings {
            if let Some(recording) = entry {
                if accepted {
                    recording.committed = true;
                } else if !recording.committed {
                    *entry = None;
                }
            }
        }
        accepted
    }
}
