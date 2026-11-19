//! First-frame notification and play history for the dedicated executable.
use super::tail;
use anyhow::Result;
use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HistoryEvent {
    Presented,
    Achievement(usize),
}

pub(super) fn record_history(
    mut history: crate::progress::Session,
    events: mpsc::Receiver<HistoryEvent>,
    signed_achievements: Vec<String>,
) -> Result<()> {
    let mut started = false;
    let mut pending = std::collections::BTreeSet::new();
    loop {
        match events.recv_timeout(Duration::from_secs(10)) {
            Ok(HistoryEvent::Presented) if !started => {
                history.start();
                started = true;
                history.tick(&std::mem::take(&mut pending).into_iter().collect::<Vec<_>>())?;
            }
            Ok(HistoryEvent::Presented) => {}
            Ok(HistoryEvent::Achievement(index)) => {
                if let Some(id) = signed_achievements.get(index) {
                    if started {
                        history.tick(std::slice::from_ref(id))?;
                    } else {
                        pending.insert(id.clone());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) if started => history.tick(&[])?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return history.finish(),
        }
    }
}

pub(super) fn read_game_output(
    mut pipe: impl Read,
    log: &Arc<Mutex<VecDeque<u8>>>,
    marker: &[u8],
    mut notify: impl FnMut(HistoryEvent),
) {
    let mut buffer = [0; 4096];
    let mut line = Vec::with_capacity(96);
    let mut overflow = false;
    loop {
        let count = match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        tail(log, &buffer[..count]);
        for &byte in &buffer[..count] {
            if byte == b'\n' {
                if !overflow {
                    let line = line.strip_suffix(b"\r").unwrap_or(&line);
                    if line == marker {
                        notify(HistoryEvent::Presented);
                    } else if let Some(index) = line
                        .strip_prefix(b"GamePKG achievement ")
                        .and_then(|text| std::str::from_utf8(text).ok())
                        .and_then(|index| index.parse().ok())
                    {
                        notify(HistoryEvent::Achievement(index));
                    }
                }
                overflow = false;
                line.clear();
            } else if line.len() < 96 {
                line.push(byte);
            } else {
                overflow = true;
            }
        }
    }
}

pub(super) fn drain_local_log(mut pipe: impl Read + Send + 'static, log: Arc<Mutex<VecDeque<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        while let Ok(count) = pipe.read(&mut buffer) {
            if count == 0 {
                break;
            }
            tail(&log, &buffer[..count]);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_frame_signal_requires_a_complete_exact_line() {
        let log = Arc::new(Mutex::new(VecDeque::new()));
        let mut data = vec![b'x'; 8191];
        data.extend(b"GamePKG presented expected\nGamePKG presented another\n");
        let mut signaled = false;
        read_game_output(&data[..], &log, b"GamePKG presented expected", |event| {
            assert_eq!(event, HistoryEvent::Presented);
            signaled = true
        });
        assert!(!signaled);
        data.extend(b"GamePKG presented expected\r\n");
        read_game_output(&data[..], &log, b"GamePKG presented expected", |event| {
            assert_eq!(event, HistoryEvent::Presented);
            signaled = true
        });
        assert!(signaled);
    }

    #[test]
    fn native_history_ignores_startup_exit_and_flushes_presented_play() {
        let library = tempfile::tempdir().unwrap();
        let root = library.path();
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        record_history(
            crate::progress::Session::open(root, "game", "creator").unwrap(),
            receiver,
            vec![],
        )
        .unwrap();
        assert_eq!(
            crate::progress::load(root, "game", "creator")
                .unwrap()
                .played_ms,
            0
        );

        let (sender, receiver) = mpsc::channel();
        let session = crate::progress::Session::open(root, "game", "creator").unwrap();
        let worker = std::thread::spawn(move || record_history(session, receiver, vec![]));
        sender.send(HistoryEvent::Presented).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        drop(sender);
        worker.join().unwrap().unwrap();
        assert!(
            crate::progress::load(root, "game", "creator")
                .unwrap()
                .played_ms
                > 0
        );
        assert_eq!(
            crate::progress::load(root, "game", "another")
                .unwrap()
                .played_ms,
            0
        );
    }
}
