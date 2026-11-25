//! Bounded dispatch to the owning native window thread or service worker.
use super::{protocol, Backend, Error, Result, Session};
use std::{
    io::{Read, Write},
    sync::{mpsc, Arc},
};
struct Request {
    operation: u32,
    bytes: Vec<u8>,
    capacity: u32,
    reply: mpsc::SyncSender<Result<Vec<u8>>>,
}
pub struct Connection {
    _leases: Vec<Arc<std::fs::File>>,
    sender: mpsc::SyncSender<Request>,
    wake: Arc<dyn Fn() + Send + Sync>,
    timing: Option<super::timing::Clock>,
    files: Option<Arc<std::sync::Mutex<super::filesystem::Files>>>,
    network: Option<(Arc<std::sync::Mutex<crate::network::Service>>, bool)>,
}
pub struct Services {
    pub(super) leases: Vec<Arc<std::fs::File>>,
    pub(super) timing: super::timing::Clock,
    pub(super) files: Arc<std::sync::Mutex<super::filesystem::Files>>,
    pub(super) network: (Arc<std::sync::Mutex<crate::network::Service>>, bool),
}
pub struct PendingCalls {
    receiver: mpsc::Receiver<Request>,
}

/// The game thread can be confined before entry while permitted operations
/// still run on a trusted thread in the same executable.
#[cfg(all(feature = "gui", any(target_os = "linux", windows)))]
pub struct LocalConnection {
    sender: mpsc::SyncSender<Request>,
}

#[cfg(all(feature = "gui", any(target_os = "linux", windows)))]
impl LocalConnection {
    pub fn call(&self, operation: u32, bytes: &[u8], capacity: u32) -> Result<Vec<u8>> {
        send(&self.sender, operation, bytes, capacity, || {})
    }
}

/// `wake` requests a redraw/event-loop wake for the owned game window. It must
/// not call back into Session synchronously. At most eight copied requests are
/// queued. The client normally has only one outstanding serialized request.
pub fn channel(wake: impl Fn() + Send + Sync + 'static) -> (Connection, PendingCalls) {
    let (sender, receiver) = mpsc::sync_channel(8);
    (
        Connection {
            _leases: Vec::new(),
            sender,
            wake: Arc::new(wake),
            timing: None,
            files: None,
            network: None,
        },
        PendingCalls { receiver },
    )
}
/// Attach the SAME session clock to the private worker. Blocking timing calls
/// bypass the owner queue; all media/input/file requests still use dispatch.
pub fn channel_with_timing(
    clock: super::timing::Clock,
    wake: impl Fn() + Send + Sync + 'static,
) -> (Connection, PendingCalls) {
    let (mut connection, pending) = channel(wake);
    connection.timing = Some(clock);
    (connection, pending)
}
/// Actual launchers use this variant: all file I/O, directory enumeration,
/// flush/replace and blocking timing run on the dedicated connection worker.
pub fn channel_with_services(
    services: Services,
    wake: impl Fn() + Send + Sync + 'static,
) -> (Connection, PendingCalls) {
    let (mut connection, pending) = channel_with_timing(services.timing, wake);
    connection.files = Some(services.files);
    connection.network = Some(services.network);
    connection._leases = services.leases;
    (connection, pending)
}
impl Connection {
    #[cfg(all(feature = "gui", any(target_os = "linux", windows)))]
    pub(super) fn into_local_worker(self) -> std::io::Result<LocalConnection> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        std::thread::Builder::new()
            .name("gamepkg-services".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let reply = self.call(request.operation, &request.bytes, request.capacity);
                    let _ = request.reply.send(reply);
                }
            })?;
        Ok(LocalConnection { sender })
    }

    #[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
    pub(super) fn clock(&self) -> Option<super::timing::Clock> {
        self.timing.clone()
    }
    /// Run on the launcher-owned I/O worker, never inside the window update.
    pub fn serve(self, input: impl Read, output: impl Write) -> std::io::Result<()> {
        protocol::serve_with(input, output, |operation, bytes, capacity| {
            self.call(operation, bytes, capacity)
        })
    }

    /// Trusted compatibility-layer dispatcher. Run from a game worker in the
    /// process that owns the session. The window thread drains PendingCalls.
    pub fn call(&self, operation: u32, bytes: &[u8], capacity: u32) -> Result<Vec<u8>> {
        if bytes.len() > protocol::MAX_MESSAGE || capacity as usize > protocol::MAX_MESSAGE {
            return Err(Error::Invalid);
        }
        if let Some(clock) = &self.timing {
            clock.snapshot()?;
        }
        if operation == 50 {
            if let Some((network, allowed)) = &self.network {
                let result = protocol::network_dispatch(
                    &mut *network.lock().map_err(|_| Error::Failed)?,
                    *allowed,
                    bytes,
                    capacity as usize,
                );
                self.timing.as_ref().ok_or(Error::Denied)?.snapshot()?;
                return result;
            }
        }
        if protocol::file_operation(operation) {
            if let Some(files) = &self.files {
                let clock = self.timing.as_ref().ok_or(Error::Denied)?;
                clock.snapshot()?;
                let result = protocol::file_dispatch(
                    &mut *files.lock().map_err(|_| Error::Failed)?,
                    operation,
                    bytes,
                    capacity as usize,
                );
                // Never return data from an operation whose session was
                // revoked while the filesystem was completing its I/O.
                clock.snapshot()?;
                return result;
            }
        }
        if (60..=62).contains(&operation) || (65..=66).contains(&operation) {
            if let Some(clock) = &self.timing {
                return clock.call(operation, bytes, capacity, true);
            }
        }
        send(&self.sender, operation, bytes, capacity, || (self.wake)())
    }
}

fn send(
    sender: &mpsc::SyncSender<Request>,
    operation: u32,
    bytes: &[u8],
    capacity: u32,
    wake: impl FnOnce(),
) -> Result<Vec<u8>> {
    if bytes.len() > protocol::MAX_MESSAGE || capacity as usize > protocol::MAX_MESSAGE {
        return Err(Error::Invalid);
    }
    let (reply, receive) = mpsc::sync_channel(1);
    let request = Request {
        operation,
        bytes: bytes.to_vec(),
        capacity,
        reply,
    };
    match sender.try_send(request) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(_)) => return Err(Error::Busy),
        Err(mpsc::TrySendError::Disconnected(_)) => return Err(Error::Denied),
    }
    wake();
    receive.recv().unwrap_or(Err(Error::Failed))
}
impl PendingCalls {
    /// Nonblocking receive; all service/backend effects happen on this caller's
    /// thread. The caller chooses its per-update budget; no game FPS cap is set.
    pub fn dispatch<B: Backend>(&self, host: &mut Session<B>, budget: usize) -> usize {
        let mut count = 0;
        while count < budget {
            let Ok(request) = self.receiver.try_recv() else {
                break;
            };
            let reply = host
                .timing()
                .snapshot()
                .and_then(|_| host.call(request.operation, &request.bytes, request.capacity));
            let _ = request.reply.send(reply);
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn timing_waits_use_worker_without_owner_dispatch_and_host_wakes_are_not_lost() {
        let mut host = super::super::tests::host();
        let clock = host.timing();
        let (now, generation) = clock.snapshot().unwrap();
        let (connection, pending) = channel_with_timing(clock.clone(), || {});
        let mut input = b"GPH1".to_vec();
        input.extend(61u32.to_le_bytes());
        input.extend(16u32.to_le_bytes());
        input.extend(20u32.to_le_bytes());
        input.extend((now + 500_000_000).to_le_bytes());
        input.extend(generation.to_le_bytes());
        let worker = std::thread::spawn(move || {
            let mut output = Vec::new();
            connection.serve(input.as_slice(), &mut output).unwrap();
            output
        });
        // Either wakes a waiting worker or changes its generation before wait.
        clock.wake().unwrap();
        assert_eq!(host.call(0, &[], 4).unwrap(), 1u32.to_le_bytes());
        let output = worker.join().unwrap();
        assert_eq!(&output[..4], &0i32.to_le_bytes());
        assert_eq!(&output[24..28], &1u32.to_le_bytes());
        assert_eq!(pending.dispatch(&mut host, 8), 0);
        drop(host);
        assert_eq!(clock.snapshot(), Err(Error::Denied));
    }
    #[test]
    fn clock_samples_run_on_private_worker_without_window_owner_dispatch() {
        for (operation, capacity) in [(65u32, 32u32), (66, 24)] {
            let mut host = super::super::tests::host();
            let clock = host.timing();
            assert_eq!(host.call(operation, &[], capacity), Err(Error::Busy));
            let before = clock.sample().unwrap();
            let wakes = Arc::new(AtomicUsize::new(0));
            let count = wakes.clone();
            let (connection, pending) = channel_with_timing(clock.clone(), move || {
                count.fetch_add(1, Ordering::SeqCst);
            });
            let request = |capacity: u32| {
                let mut b = b"GPH1".to_vec();
                b.extend(operation.to_le_bytes());
                b.extend(0u32.to_le_bytes());
                b.extend(capacity.to_le_bytes());
                b
            };
            let packet = request(capacity);
            let worker = std::thread::spawn(move || {
                let mut out = Vec::new();
                connection.serve(packet.as_slice(), &mut out).unwrap();
                out
            });
            let output = worker.join().unwrap();
            let after = clock.sample().unwrap();
            assert_eq!(&output[..4], &0i32.to_le_bytes());
            assert_eq!(&output[4..8], &capacity.to_le_bytes());
            let word = |at: usize| u64::from_le_bytes(output[8 + at..16 + at].try_into().unwrap());
            assert!(word(0) >= before.counter && word(0) <= after.counter);
            assert_eq!(word(8), before.frequency);
            assert!(word(16) >= before.uptime_ms && word(16) <= after.uptime_ms);
            if operation == 65 {
                assert!(word(24) >= before.utc_100ns && word(24) <= after.utc_100ns);
            }
            assert_eq!(pending.dispatch(&mut host, 8), 0);
            assert_eq!(wakes.load(Ordering::SeqCst), 0);
            let (connection, _pending) = channel_with_timing(clock.clone(), || {});
            let mut output = Vec::new();
            connection
                .serve(request(capacity - 1).as_slice(), &mut output)
                .unwrap();
            assert_eq!(&output[..4], &(Error::Invalid as i32).to_le_bytes());
            clock.close();
            let (connection, _pending) = channel_with_timing(clock.clone(), || {});
            let mut output = Vec::new();
            connection
                .serve(request(capacity).as_slice(), &mut output)
                .unwrap();
            assert_eq!(&output[..4], &(Error::Denied as i32).to_le_bytes());
        }
    }
    #[test]
    fn pipe_worker_wakes_owner_thread_and_shutdown_revokes_pending_calls() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        let (connection, pending) = channel(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        let mut input = b"GPH1".to_vec();
        input.extend(0u32.to_le_bytes());
        input.extend(0u32.to_le_bytes());
        input.extend(4u32.to_le_bytes());
        let worker = std::thread::spawn(move || {
            let mut out = Vec::new();
            connection.serve(input.as_slice(), &mut out).unwrap();
            out
        });
        let mut host = super::super::tests::host();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !worker.is_finished() {
            pending.dispatch(&mut host, 1);
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let output = worker.join().unwrap();
        assert_eq!(&output[8..], &1u32.to_le_bytes());
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        let (connection, pending) = channel(|| {});
        drop(pending);
        let mut input = b"GPH1".to_vec();
        input.extend([0; 8]);
        input.extend(4u32.to_le_bytes());
        let mut output = Vec::new();
        connection.serve(input.as_slice(), &mut output).unwrap();
        assert_eq!(&output[..4], &(Error::Denied as i32).to_le_bytes());
    }
}
