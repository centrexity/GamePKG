//! Keep catalog reads and strict installed-package verification off the
//! window's first frame.
use crate::registry::LibraryInspection;
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

pub(super) struct Snapshot {
    pub root: PathBuf,
    pub sources: Result<Vec<crate::catalog::sources::List>, String>,
    pub catalog: Result<Vec<crate::catalog::Entry>, String>,
    pub inspection: Result<LibraryInspection, String>,
}
struct Request {
    generation: u64,
    root: PathBuf,
    context: eframe::egui::Context,
}
struct Worker {
    requests: mpsc::Sender<Request>,
    results: mpsc::Receiver<(u64, Snapshot)>,
}
impl Worker {
    fn spawn(compute: impl Fn(&Path) -> Snapshot + Send + 'static) -> Self {
        let (requests, input) = mpsc::channel::<Request>();
        let (output, results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(mut request) = input.recv() {
                // Repeated refreshes need only the newest queued inspection.
                for newer in input.try_iter() {
                    request = newer;
                }
                let snapshot = compute(&request.root);
                if output.send((request.generation, snapshot)).is_err() {
                    break;
                }
                request.context.request_repaint();
            }
        });
        Self { requests, results }
    }
}
fn inspect(root: &Path) -> Snapshot {
    Snapshot {
        root: root.to_path_buf(),
        sources: crate::catalog::sources::lists(root).map_err(|error| format!("{error:#}")),
        catalog: crate::catalog::entries(root).map_err(|error| format!("{error:#}")),
        inspection: crate::registry::inspect_library(root).map_err(|error| format!("{error:#}")),
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
    pub fn request(&mut self, root: PathBuf, context: eframe::egui::Context) {
        self.invalidate();
        let worker = self.worker.get_or_insert_with(|| Worker::spawn(inspect));
        self.pending = worker
            .requests
            .send(Request {
                generation: self.generation,
                root,
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
    #[cfg(test)]
    pub(super) fn with_compute(compute: impl Fn(&Path) -> Snapshot + Send + 'static) -> Self {
        Self {
            worker: Some(Worker::spawn(compute)),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn blocked_inspection_does_not_block_requests_and_stale_results_are_discarded() {
        let (started, starts) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = std::sync::Arc::new(std::sync::Mutex::new(released));
        let worker = Worker::spawn(move |root| {
            started.send(root.to_path_buf()).unwrap();
            released.lock().unwrap().recv().unwrap();
            Snapshot {
                root: root.to_path_buf(),
                sources: Ok(Vec::new()),
                catalog: Ok(Vec::new()),
                inspection: Ok(LibraryInspection {
                    games: Vec::new(),
                    verified: std::collections::BTreeMap::new(),
                    rejected: Vec::new(),
                }),
            }
        });
        let mut scanner = Scanner {
            worker: Some(worker),
            ..Default::default()
        };
        let context = eframe::egui::Context::default();
        scanner.request("old".into(), context.clone());
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(2)).unwrap(),
            Path::new("old")
        );
        scanner.request("superseded".into(), context.clone());
        scanner.request("current".into(), context);
        assert!(scanner.pending());
        assert!(scanner.poll().is_none());
        release.send(()).unwrap();
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(2)).unwrap(),
            Path::new("current")
        );
        assert!(
            scanner.poll().is_none(),
            "the old inspection's completed result must be ignored"
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
        assert_eq!(snapshot.root, Path::new("current"));
        assert!(!scanner.pending());
    }
}
