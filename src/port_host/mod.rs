//! Port-facing services. Only the host constructs grants or supplies a backend.
//! `Session::call` accepts copied bytes, not game pointers or OS/GPU handles.
pub mod admission;
pub mod audio_streams;
pub mod audit;
pub mod broker;
pub mod capture;
#[cfg(any(target_os = "linux", windows))]
mod coff_exclusions;
#[cfg(target_os = "linux")]
mod compiler_jobs;
#[cfg(feature = "gui")]
pub mod controllers;
#[cfg(feature = "gui")]
pub mod desktop;
pub mod diagnostics;
mod filesystem;
#[cfg(feature = "gui")]
pub mod graphics;
#[path = "../../engines/native-services/local_runtime.rs"]
pub mod local_runtime;
mod media;
#[cfg(any(target_os = "linux", windows))]
pub mod owner;
#[cfg(feature = "gui")]
pub(crate) mod physical_key;
#[cfg(all(feature = "gui", any(target_os = "linux", windows)))]
pub mod player;
#[cfg(any(target_os = "linux", windows))]
pub mod preparation;
#[cfg(any(target_os = "linux", windows))]
pub mod process;
mod protocol;
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
mod sdk_source;
#[cfg(test)]
mod tests;
pub mod timing;
#[cfg(feature = "gui")]
pub mod winit_input;

use crate::manifest::Permissions;
pub use filesystem::Access;
use gamepkg_native_input::Service as Input;
#[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
mod tick_picker;
mod tick_services;
pub use media::{Backend, Draw, Frame, Image, WindowCommand, WindowState};
pub use protocol::{serve, ABI, MAX_MESSAGE};
/// The launcher stages these audited bindings after checking game source;
/// packages must not supply replacement copies or Cargo dependencies.
pub const CLIENT_SOURCE: &str = include_str!("../../engines/native-services/client.rs");
pub const C_HEADER: &str = include_str!("../../include/gamepkg_port.h");
/// Include in the native recipe/cache receipt along with compiler, target and
/// host binary identities. Changing a service or binding invalidates old kits.
pub fn build_identity() -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    for source in [
        C_HEADER,
        CLIENT_SOURCE,
        include_str!("../../engines/native-services/game_state.rs"),
        include_str!("../../engines/native-services/tick2d.rs"),
        include_str!("../../engines/native-services/local_runtime.rs"),
        include_str!("../../engines/native-services/local_memory.rs"),
        include_str!("../../engines/native-services/performance_clock.rs"),
        include_str!("../../engines/native-services/local_timing.rs"),
        include_str!("../../engines/native-services/local_timing_windows.rs"),
        include_str!("../../engines/native-services/local_bindings.rs"),
        include_str!("mod.rs"),
        include_str!("filesystem.rs"),
        include_str!("filesystem/save_metadata.rs"),
        include_str!("filesystem/windows_replace.rs"),
        include_str!("media.rs"),
        include_str!("audio_streams.rs"),
        include_str!("capture.rs"),
        include_str!("diagnostics.rs"),
        include_str!("../audio/capture.rs"),
        include_str!("protocol.rs"),
        include_str!("tick_services.rs"),
        include_str!("tick_picker.rs"),
        include_str!("../guest_helpers.rs.txt"),
        include_str!("broker.rs"),
        include_str!("timing.rs"),
        include_str!("timing/platform.rs"),
        include_str!("audit.rs"),
        include_str!("admission.rs"),
        include_str!("preparation.rs"),
        include_str!("preparation_local.rs"),
        include_str!("sdk_source.rs"),
        include_str!("../../examples/maintainer/native_service_runtime.rs"),
        include_str!("compiler_jobs.rs"),
        include_str!("coff_exclusions.rs"),
        include_str!("../takedawn_windows_sdk.rs"),
        include_str!("../windows_compiler.rs"),
        include_str!("../compiler.rs"),
        include_str!("../files.rs"),
        include_str!("../archive.rs"),
        include_str!("../manifest.rs"),
        include_str!("../game_data.rs"),
        include_str!("../../engines/takedawn/windows-runner/src/lib.rs"),
        include_str!("../../engines/takedawn/windows-runner/src/rom_grants.rs"),
        include_str!("../../engines/takedawn/windows-runner/src/sandbox.rs"),
        include_str!("../../engines/takedawn/windows-runner/src/worker_pipe.rs"),
        include_str!("../../engines/takedawn/windows-runner/src/worker_process.rs"),
        include_str!("../../engines/native/windows-rust.json"),
        include_str!("../../engines/native/windows-compat-tools.json"),
        include_str!("../../engines/native/windows-auditor.json"),
        include_str!("process.rs"),
        include_str!("process_local.rs"),
        include_str!("process_history.rs"),
        include_str!("process_windows.rs"),
        include_str!("../saves.rs"),
        include_str!("owner.rs"),
        include_str!("../../engines/native-services/lockdown.rs"),
        include_str!("desktop.rs"),
        include_str!("graphics.rs"),
        include_str!("graphics/retained.rs"),
        include_str!("graphics/state.rs"),
        include_str!("graphics/variants.rs"),
        include_str!("graphics/verified.rs"),
        include_str!("graphics/takedawn.rs"),
        include_str!("winit_input.rs"),
        include_str!("controllers.rs"),
        include_str!("player.rs"),
        include_str!("physical_key.rs"),
        include_str!("../network.rs"),
        include_str!("../network/interfaces.rs"),
        include_str!("../source.rs"),
        include_str!("../launch_options.rs"),
        include_str!("../dependencies.rs"),
        include_str!("../build_policy.rs"),
        include_str!("../shaders.rs"),
        include_str!("../shaders/nv2a.rs"),
        include_str!("../../vendor/naga/src/back/wgsl/writer.rs"),
        include_str!("../gpu_vulkan.rs"),
        include_str!("../audio.rs"),
        include_str!("../audio/pcm.rs"),
        include_str!("../../engines/native-input/src/lib.rs"),
        include_str!("../../Cargo.toml"),
        include_str!("../../Cargo.lock"),
        include_str!("../../vendor/wgpu-hal/src/vulkan/adapter.rs"),
        include_str!("../../vendor/wgpu-hal/src/vulkan/command.rs"),
        include_str!("../../vendor/wgpu-hal/src/vulkan/device.rs"),
        include_str!("../../vendor/wgpu-hal/src/vulkan/mod.rs"),
        include_str!("../../vendor/wgpu-hal/src/gles/egl.rs"),
    ] {
        hash.update((source.len() as u64).to_le_bytes());
        hash.update(source.as_bytes());
    }
    hex::encode(hash.finalize())
}
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    Invalid = -1,
    Busy = -2,
    Failed = -3,
    Denied = -4,
    Unsupported = -5,
    /// Replacement is visible, but the subsequent directory flush failed.
    CommittedNotDurable = -6,
}
pub type Result<T> = std::result::Result<T, Error>;

/// One game process, one session. Drop revokes all grants and resources.
/// This object and its backend are never made available to game source.
pub struct Session<B: Backend> {
    backend: B,
    permissions: Permissions,
    files: Arc<std::sync::Mutex<filesystem::Files>>,
    input: Input,
    network: Arc<std::sync::Mutex<crate::network::Service>>,
    images: BTreeMap<u64, Arc<Image>>,
    image_bytes: usize,
    window: WindowState,
    timing: timing::Clock,
    streams: Option<audio_streams::Mixer>,
    leases: Vec<Arc<std::fs::File>>,
    diagnostics: diagnostics::Diagnostics,
    launch_options: Option<[u8; 8]>,
    tick: Option<tick_services::Services>,
}
impl<B: Backend> Session<B> {
    pub fn new(permissions: Permissions, backend: B) -> Self {
        let streams = backend.audio_streams();
        Self {
            network: Arc::new(std::sync::Mutex::new(crate::network::Service::new(
                permissions.clone(),
            ))),
            permissions,
            backend,
            files: Arc::new(std::sync::Mutex::new(filesystem::Files::default())),
            input: Input::new(),
            images: BTreeMap::new(),
            image_bytes: 0,
            window: WindowState::default(),
            timing: timing::Clock::new(),
            streams,
            leases: Vec::new(),
            diagnostics: diagnostics::Diagnostics::default(),
            launch_options: None,
            tick: None,
        }
    }
    /// Host-only capability grant. Paths never cross the game-facing protocol.
    /// Writable roots require the signed save permission.
    pub fn grant_directory(&mut self, directory: cap_std::fs::Dir, access: Access) -> Result<u64> {
        if matches!(access, Access::Save | Access::PreviousSave) && !self.permissions.save {
            return Err(Error::Denied);
        }
        self.files
            .lock()
            .map_err(|_| Error::Failed)?
            .grant_directory(directory, access)
    }
    /// An already opened, user-selected regular ROM. Reads stay in place and
    /// use 64-bit offsets. No path, writable operation or image copy is exposed.
    pub fn grant_rom(&mut self, file: std::fs::File) -> Result<u64> {
        self.files
            .lock()
            .map_err(|_| Error::Failed)?
            .grant_rom(file)
    }
    /// Host-only Linux disposable RAM cache, separately charged before launch.
    /// No persistent path, directory traversal or game-side allocation request.
    pub fn grant_volatile_cache(&mut self, bytes: u64) -> Result<u64> {
        self.files
            .lock()
            .map_err(|_| Error::Failed)?
            .grant_volatile(bytes)
    }
    /// Revoke a mount and every open handle derived from it.
    pub fn revoke(&mut self, grant: u64) -> Result<()> {
        self.files.lock().map_err(|_| Error::Failed)?.revoke(grant)
    }
    /// Host policy revocation stops capture and discards pending microphone
    /// data. Output audio/network permissions do not confer capture access.
    pub fn revoke_microphone(&mut self) {
        self.permissions.microphone = false;
        self.backend.revoke_capture();
    }
    pub fn grant_save_quota(&mut self, directory: cap_std::fs::Dir, bytes: u64) -> Result<u64> {
        if !self.permissions.save {
            return Err(Error::Denied);
        }
        self.files
            .lock()
            .map_err(|_| Error::Failed)?
            .grant_save_quota(directory, bytes)
    }
    /// Trusted device adapters feed the existing shared service per session.
    /// Games can only query snapshots/events through the protocol.
    pub fn input_backend(&mut self) -> &mut Input {
        &mut self.input
    }
    /// Observations come from the actual host window, never from a game request.
    pub fn observe_window(&mut self, state: WindowState) {
        self.input.focus(state.focused && !state.closed);
        if state.closed {
            self.timing.close();
            self.backend.revoke_capture();
            if let Some(streams) = &self.streams {
                streams.close();
            }
        } else if state != self.window {
            let _ = self.timing.wake();
        }
        self.window = state;
    }
    /// Host-only drain of this session's numeric events and count evicted since
    /// the previous drain. No game API reads host logs or other sessions.
    pub fn take_diagnostics(&mut self) -> (Vec<diagnostics::Event>, u64) {
        self.diagnostics.take()
    }
    /// Read-only bounded numeric summary for existing host error reporting.
    pub fn diagnostic_summary(&self) -> String {
        self.diagnostics.summary()
    }
    pub fn backend(&mut self) -> &mut B {
        &mut self.backend
    }
    /// Clone for the private connection worker and host event-loop wakeups.
    pub fn timing(&self) -> timing::Clock {
        self.timing.clone()
    }
    pub fn worker_services(&self) -> broker::Services {
        broker::Services {
            timing: self.timing.clone(),
            files: self.files.clone(),
            network: (self.network.clone(), self.permissions.network),
            leases: self.leases.clone(),
        }
    }
    pub(super) fn hold_lease(&mut self, file: Arc<std::fs::File>) {
        self.leases.push(file);
    }
    pub fn call(&mut self, operation: u32, request: &[u8], capacity: u32) -> Result<Vec<u8>> {
        if request.len() > MAX_MESSAGE || capacity as usize > MAX_MESSAGE {
            return Err(Error::Invalid);
        }
        let reply = protocol::dispatch(self, operation, request, capacity as usize)?;
        if reply.len() > capacity as usize {
            return Err(Error::Failed);
        }
        Ok(reply)
    }
}

// IDs are not reused by another resource/session in this host process. No
// generation wraparound can turn a stale file or texture handle into a new one.
fn identifier() -> Result<u64> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |id| id.checked_add(1),
    )
    .map_err(|_| Error::Failed)
}
impl<B: Backend> Drop for Session<B> {
    fn drop(&mut self) {
        self.timing.close();
        self.backend.revoke_capture();
        if let Some(streams) = &self.streams {
            streams.close();
        }
    }
}
