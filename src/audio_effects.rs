//! Trusted, bounded recorded audio and per-tick control transactions.
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
    music: Option<Box<crate::music::controls::Transaction>>,
    publisher: Option<Publisher>,
    _guest: Option<Lease>,
    _reservation: Option<Lease>,
}
impl Runtime {
    pub(crate) fn new(
        package: Option<&Package>,
        declared: Option<u64>,
        music_declared: Option<u64>,
        total: u64,
        allowed: bool,
    ) -> Result<(Self, u64)> {
        if declared.is_none() && music_declared.is_none() {
            return Ok((Self::default(), total));
        }
        ensure!(allowed, "recorded audio requires audio permission");
        if let Some(bytes) = declared {
            ensure!(
                (65536..=4 * 1048576).contains(&bytes),
                "invalid effects memory reservation"
            );
        }
        if let Some(bytes) = music_declared {
            ensure!(
                (65536..=16 * 1048576).contains(&bytes),
                "invalid music memory reservation"
            );
        }
        let reserved = declared
            .unwrap_or(0)
            .checked_add(music_declared.unwrap_or(0))
            .context("audio reservation overflow")?;
        let guest_bytes = total
            .checked_sub(reserved)
            .filter(|n| *n >= 65536)
            .context("recorded audio exceeds the selected memory ceiling")?;
        let parent = Budget::new(total);
        let guest = parent.reserve(guest_bytes)?;
        let effects_budget = declared.map(|bytes| parent.partition(bytes)).transpose()?;
        let music_budget = music_declared
            .map(|bytes| parent.partition(bytes))
            .transpose()?;
        let budget = effects_budget
            .as_ref()
            .or(music_budget.as_ref())
            .unwrap()
            .clone();
        let reservation = budget
            .reserve((std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64)?;
        let package = package.context("recorded audio requires a verified package")?;
        let service = effects_budget
            .map(|budget| {
                Service::new(package.try_clone_verified()?, allowed, budget).map(Box::new)
            })
            .transpose()?;
        let music = music_budget
            .map(|budget| -> Result<_> {
                let factory = crate::audio_decode::Factory::bundled()?;
                let decoder_bytes = factory.maximum_memory_bytes();
                ensure!(
                    budget.limit() - budget.used()
                        >= decoder_bytes as u64
                            + crate::music::HOST_BYTES
                            + std::mem::size_of::<crate::audio_mix_stream::Playback>() as u64,
                    "music reservation cannot contain the decoder and output staging"
                );
                let service = crate::music::Service::new_original_rate(
                    factory,
                    package,
                    allowed,
                    budget,
                    decoder_bytes,
                )?;
                Ok(Box::new(crate::music::controls::Transaction::new(service)))
            })
            .transpose()?;
        Ok((
            Self {
                service,
                music,
                budget: Some(budget),
                publisher: None,
                _guest: Some(guest),
                _reservation: Some(reservation),
            },
            guest_bytes,
        ))
    }
    pub(crate) fn enabled(&self) -> bool {
        self.service.is_some() || self.music.is_some()
    }
    pub(crate) fn attach(&mut self, publisher: Publisher) {
        self.publisher = Some(publisher);
    }
    pub(crate) fn playback(&self) -> Result<Option<crate::audio_mix_stream::Playback>> {
        self.budget
            .as_ref()
            .map(|budget| {
                if let Some(music) = &self.music {
                    crate::audio_mix_stream::Playback::new(music.output(), budget)
                } else {
                    crate::audio_mix_stream::Playback::effects_only(budget)
                }
            })
            .transpose()
    }
    pub(crate) fn music_output(&self) -> Option<crate::music::Output> {
        self.music.as_ref().map(|music| music.output())
    }
    pub(crate) fn begin(&mut self) -> Result<()> {
        if let Some(service) = &mut self.service {
            service.begin()?;
        }
        if let Some(music) = &mut self.music {
            music.begin()?;
        }
        Ok(())
    }
    pub(crate) fn call(&mut self, operation: u32, path: &str, value: u32) -> bool {
        if self.publisher.is_none() {
            return false;
        }
        if operation >= 4 {
            let Some(music) = &mut self.music else {
                return false;
            };
            if path.is_empty() && value == 0 {
                match operation {
                    8 => return music.output().status() == crate::music::Status::Ended,
                    9 => return music.output().status() == crate::music::Status::Failed,
                    _ => {}
                }
            }
            return match operation {
                4 if value == 0 => music.play(path),
                5 if path.is_empty() => music.volume(value),
                6 if path.is_empty() && value <= 1 => music.pause(value != 0),
                7 if path.is_empty() && value == 0 => music.stop(),
                _ => return false,
            }
            .is_ok();
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
        if !succeeded {
            if let Some(service) = &mut self.service {
                service.finish(false, |_| false);
            }
            if let Some(music) = &mut self.music {
                music.finish(false)?;
            }
            return Ok(());
        }
        let require_output = self.music.as_ref().is_some_and(|music| !music.is_empty());
        let publisher = &self.publisher;
        let service = &mut self.service;
        let mut publish = || {
            let output = |commands: &[EffectCommand]| {
                (commands.is_empty() && !require_output)
                    || publisher.as_ref().is_some_and(|publish| publish(commands))
            };
            service
                .as_mut()
                .map_or_else(|| output(&[]), |service| service.finish(true, output))
        };
        let result = if let Some(music) = &mut self.music {
            music.finish_with(true, publish)
        } else {
            ensure!(
                publish(),
                "recorded effect output rejected the tick's control batch"
            );
            Ok(())
        };
        if result.is_err() {
            if let Some(service) = &mut self.service {
                service.finish(false, |_| false);
            }
        }
        result
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
