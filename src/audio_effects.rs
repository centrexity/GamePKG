//! Trusted, bounded recorded-effect cache and per-tick control transaction.
//! Runtime/SDK wiring remains separate; no guest receives a device or clip.
use crate::{
    archive::Package,
    audio_voices::{Clip, EffectCommand},
    memory_budget::{Budget, Lease},
};
use anyhow::{ensure, Context, Result};
use std::sync::Arc;

// Match the existing device control queue; cache metadata has fixed storage.
const CAPACITY: usize = 32;
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
