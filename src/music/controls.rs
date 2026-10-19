//! Host-owned music controls staged until execution and persistence succeed.
use super::{Command, Output, Service};
use anyhow::{ensure, Result};
const CAPACITY: usize = 32;
#[derive(Clone, Copy)]
enum Operation {
    Play,
    Volume(u32),
    Pause(bool),
    Stop,
}
/// One owned service permits one transaction buffer. Its fixed memory is charged
/// in Service's existing host reservation, including the temporary command array.
pub struct Transaction {
    service: Service,
    pending: [Option<Operation>; CAPACITY],
    path: [u8; 1024],
    path_length: usize,
    length: usize,
    active: bool,
    active_generation: Option<u64>,
}
impl Transaction {
    pub(super) const MEMORY_BYTES: usize =
        std::mem::size_of::<Self>() + std::mem::size_of::<[Command<'static>; CAPACITY]>();
    pub fn new(service: Service) -> Self {
        Self {
            service,
            pending: [None; CAPACITY],
            path: [0; 1024],
            path_length: 0,
            length: 0,
            active: false,
            active_generation: None,
        }
    }
    pub fn output(&self) -> Output {
        self.service.output()
    }
    pub fn error(&self) -> Option<String> {
        self.service.error()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn begin(&mut self) -> Result<()> {
        ensure!(!self.active, "music transaction already active");
        self.active = true;
        let snapshot = self
            .service
            .output
            .0
            .status
            .load(std::sync::atomic::Ordering::Acquire);
        self.active_generation = super::Status::from_tag(snapshot)
            .active()
            .then_some(snapshot >> super::GENERATION_SHIFT);
        Ok(())
    }
    fn push(&mut self, operation: Operation) -> Result<()> {
        ensure!(self.active, "music transaction is not active");
        ensure!(self.length < CAPACITY, "music control queue is full");
        self.pending[self.length] = Some(operation);
        self.length += 1;
        Ok(())
    }
    pub fn play(&mut self, path: &str) -> Result<()> {
        ensure!(crate::archive::valid_path(path), "invalid music asset path");
        self.push(Operation::Play)?;
        // Only the last replacement loads at batch commit. All earlier Play
        // commands still retain their ordering and generation increments.
        self.path[..path.len()].copy_from_slice(path.as_bytes());
        self.path_length = path.len();
        Ok(())
    }
    pub fn volume(&mut self, volume: u32) -> Result<()> {
        ensure!(volume <= 128, "music volume must be 0 to 128");
        self.push(Operation::Volume(volume))
    }
    pub fn pause(&mut self, paused: bool) -> Result<()> {
        self.push(Operation::Pause(paused))
    }
    pub fn stop(&mut self) -> Result<()> {
        self.push(Operation::Stop)
    }
    /// Failed ticks and rejected successful-tick batches are consumed and cannot
    /// replay. Prior music continues unchanged when a tick fails.
    pub fn finish(&mut self, succeeded: bool) -> Result<()> {
        self.finish_with(succeeded, || true)
    }
    pub(crate) fn finish_with(
        &mut self,
        succeeded: bool,
        publish: impl FnOnce() -> bool,
    ) -> Result<()> {
        ensure!(self.active, "music transaction is not active");
        self.active = false;
        let result = if succeeded {
            let mut commands = [Command::Stop; CAPACITY];
            let path =
                std::str::from_utf8(&self.path[..self.path_length]).expect("copied music path");
            for (command, operation) in commands.iter_mut().zip(&self.pending[..self.length]) {
                *command = match operation.expect("staged music control") {
                    Operation::Play => Command::Play(path),
                    Operation::Volume(volume) => Command::Volume(volume),
                    Operation::Pause(paused) => Command::Pause(paused),
                    Operation::Stop => Command::Stop,
                };
            }
            self.service
                .batch_for_tick(&commands[..self.length], publish, self.active_generation)
        } else {
            Ok(())
        };
        self.pending[..self.length].fill(None);
        self.length = 0;
        self.path_length = 0;
        self.active_generation = None;
        result
    }
}
