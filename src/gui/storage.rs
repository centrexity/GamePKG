//! Keep large compiler/save inventories off the window's first frame.
use crate::{
    manifest::Manifest,
    storage::{Space, Usage},
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::mpsc,
};

pub(super) struct Snapshot {
    pub space: Option<Space>,
    pub usage: HashMap<String, Usage>,
}
struct Request {
    generation: u64,
    root: PathBuf,
    games: Vec<Manifest>,
    context: eframe::egui::Context,
}
struct Worker {
    requests: mpsc::Sender<Request>,
    results: mpsc::Receiver<(u64, Snapshot)>,
}
impl Worker {
    fn spawn(compute: impl Fn(&Path, &[Manifest]) -> Snapshot + Send + 'static) -> Self {
        let (requests, input) = mpsc::channel::<Request>();
        let (output, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = input.recv() {
                // Repeated refreshes need only the newest queued inventory.
                for newer in input.try_iter() {
                    request = newer;
                }
                let snapshot = compute(&request.root, &request.games);
                if output.send((request.generation, snapshot)).is_err() {
                    break;
                }
                request.context.request_repaint();
            }
        });
        Self { requests, results }
    }
}
#[derive(Default)]
pub(super) struct Scanner {
    worker: Option<Worker>,
    generation: u64,
    pending: bool,
}
impl Scanner {
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = false;
    }
    pub fn request(&mut self, root: PathBuf, games: Vec<Manifest>, context: eframe::egui::Context) {
        self.invalidate();
        let worker = self.worker.get_or_insert_with(|| {
            Worker::spawn(|root, games| Snapshot {
                space: crate::storage::space(root).ok(),
                usage: games
                    .iter()
                    .filter_map(|m| {
                        crate::storage::usage(root, m)
                            .ok()
                            .map(|usage| (m.id.clone(), usage))
                    })
                    .collect(),
            })
        });
        self.pending = worker
            .requests
            .send(Request {
                generation: self.generation,
                root,
                games,
                context,
            })
            .is_ok();
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn poll(&mut self) -> Option<Snapshot> {
        let mut newest = None;
        if let Some(worker) = &self.worker {
            for (generation, snapshot) in worker.results.try_iter() {
                if generation == self.generation {
                    self.pending = false;
                    newest = Some(snapshot);
                }
            }
        }
        newest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn blocked_inventory_does_not_block_requests_and_stale_roots_cannot_replace_current_storage() {
        let (started, starts) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = Worker::spawn(move |root, _| {
            started.send(root.to_path_buf()).unwrap();
            released.recv().unwrap();
            Snapshot {
                space: Some(Space {
                    file_bytes: 17,
                    on_disk_bytes: Some(9),
                }),
                usage: HashMap::new(),
            }
        });
        let mut scanner = Scanner {
            worker: Some(worker),
            ..Default::default()
        };
        let context = eframe::egui::Context::default();
        scanner.request("old".into(), vec![], context.clone());
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(2)).unwrap(),
            Path::new("old")
        );
        scanner.request("superseded".into(), vec![], context.clone());
        scanner.request("current".into(), vec![], context);
        assert!(scanner.pending());
        assert!(scanner.poll().is_none());
        release.send(()).unwrap();
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(2)).unwrap(),
            Path::new("current")
        );
        assert!(
            scanner.poll().is_none(),
            "old root's completed scan must be ignored"
        );
        assert!(scanner.pending());
        release.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(2);
        let snapshot = loop {
            if let Some(value) = scanner.poll() {
                break value;
            }
            assert!(Instant::now() < until);
            std::thread::yield_now();
        };
        assert_eq!(snapshot.space.unwrap().on_disk_bytes, Some(9));
        assert!(!scanner.pending());
        scanner.request("discarded".into(), vec![], eframe::egui::Context::default());
        starts.recv_timeout(Duration::from_secs(2)).unwrap();
        scanner.invalidate();
        release.send(()).unwrap();
        assert!(!scanner.pending());
        // Disconnect and drain the worker, then verify that invalidation also
        // rejects results arriving after a library read error or root change.
        let worker = scanner.worker.take().unwrap();
        drop(worker.requests);
        for (generation, _) in worker.results {
            assert_ne!(generation, scanner.generation);
        }
    }
}
