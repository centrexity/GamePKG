//! The game window requests a picker on a private inherited socket. Only the
//! unconfined launcher sees selected paths. The audited game worker sees handles.
use crate::{
    manifest::Manifest,
    user_files::{self, Job, Kind, Outcome, Service},
};
use std::{
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    sync::mpsc,
};

fn write(socket: &mut UnixStream, operation: u32, payload: &[u8]) -> io::Result<()> {
    socket.write_all(&operation.to_le_bytes())?;
    socket.write_all(&(payload.len() as u32).to_le_bytes())?;
    socket.write_all(payload)
}

fn read(socket: &mut UnixStream, maximum: usize) -> io::Result<(u32, Vec<u8>)> {
    let mut header = [0; 8];
    socket.read_exact(&mut header)?;
    let operation = u32::from_le_bytes(header[..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    if len > maximum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "oversized picker message",
        ));
    }
    let mut bytes = vec![0; len];
    socket.read_exact(&mut bytes)?;
    Ok((operation, bytes))
}

fn named(name: &str, bytes: &[u8]) -> Vec<u8> {
    [
        (name.len() as u32).to_le_bytes().as_slice(),
        name.as_bytes(),
        bytes,
    ]
    .concat()
}

fn imported(bytes: Vec<u8>) -> Option<Outcome> {
    let len = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
    if len > 256 {
        return None;
    }
    let name = std::str::from_utf8(bytes.get(4..4 + len)?).ok()?.to_owned();
    Some(Outcome::Imported {
        name,
        bytes: bytes.get(4 + len..)?.to_vec(),
    })
}

fn exchange(socket: &mut UnixStream, index: u32, kind: &Kind, max_bytes: usize) -> Option<Outcome> {
    let mut bytes = index.to_le_bytes().to_vec();
    let operation = match kind {
        Kind::Import => 0,
        Kind::Export {
            name,
            bytes: contents,
        } => {
            bytes.extend(named(name, contents));
            1
        }
    };
    write(socket, operation, &bytes).ok()?;
    let (status, bytes) = read(socket, max_bytes.min(user_files::MAX_IMPORT) + 260).ok()?;
    match status {
        2 => imported(bytes),
        3 if bytes.is_empty() => Some(Outcome::Exported),
        4 if bytes.is_empty() => Some(Outcome::Cancelled),
        _ => Some(Outcome::Failed),
    }
}

fn alive(socket: &UnixStream) -> bool {
    let mut poll = libc::pollfd {
        fd: socket.as_raw_fd(),
        events: libc::POLLRDHUP,
        revents: 0,
    };
    let result = unsafe { libc::poll(&mut poll, 1, 0) };
    result == 0 && poll.revents == 0
}

pub(super) fn serve(socket: UnixStream, manifest: Manifest) -> io::Result<()> {
    let title = manifest.display_title().to_owned();
    serve_with_picker(socket, manifest, |job| {
        let dialog = rfd::FileDialog::new()
            .set_title(format!("GamePKG: {} — {}", title, job.policy.title))
            .add_filter(&job.policy.title, &job.policy.extensions);
        match &job.kind {
            Kind::Import => dialog.pick_file(),
            Kind::Export { name, .. } => dialog.set_file_name(name).save_file(),
        }
    })
}

fn serve_with_picker(
    mut socket: UnixStream,
    manifest: Manifest,
    mut choose: impl FnMut(&Job) -> Option<std::path::PathBuf>,
) -> io::Result<()> {
    while let Ok((operation, bytes)) = read(&mut socket, user_files::MAX_REQUEST) {
        if operation > 1 {
            break;
        }
        let mut service = Service::new(manifest.permissions.files.clone());
        service.enable();
        service.begin(true);
        if service.call(operation, &bytes, 4).is_none() {
            write(&mut socket, 5, &[])?;
            continue;
        }
        service.finish(true);
        let job = service
            .take_job()
            .expect("validated request has a picker job");
        let path = choose(&job);
        if !alive(&socket) {
            break;
        }
        match user_files::execute(&job, path.as_deref()) {
            Outcome::Imported { name, bytes } => write(&mut socket, 2, &named(&name, &bytes))?,
            Outcome::Exported => write(&mut socket, 3, &[])?,
            Outcome::Cancelled => write(&mut socket, 4, &[])?,
            Outcome::Failed => write(&mut socket, 5, &[])?,
        }
    }
    Ok(())
}

type Pending = (Job, mpsc::Receiver<(UnixStream, Outcome)>);

pub(super) struct Picker {
    socket: Option<UnixStream>,
    pending: Option<Pending>,
    policies: Vec<String>,
}

impl Picker {
    pub fn new(socket: UnixStream, manifest: &Manifest) -> Self {
        Self {
            socket: Some(socket),
            pending: None,
            policies: manifest
                .permissions
                .files
                .iter()
                .map(|file| file.id.clone())
                .collect(),
        }
    }

    pub fn tick(&mut self, service: &mut Service, wake: impl FnOnce() + Send + 'static) -> bool {
        if let Some((job, receive)) = &self.pending {
            match receive.try_recv() {
                Ok((socket, outcome)) => {
                    self.socket = Some(socket);
                    service.complete(job, outcome);
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    service.complete(job, Outcome::Failed);
                }
                Err(mpsc::TryRecvError::Empty) => return false,
            }
            self.pending = None;
        }
        let Some(job) = service.take_job() else {
            return false;
        };
        let Some(index) = self.policies.iter().position(|id| *id == job.policy.id) else {
            service.complete(&job, Outcome::Failed);
            return false;
        };
        let Some(mut socket) = self.socket.take() else {
            service.complete(&job, Outcome::Failed);
            return false;
        };
        let (send, receive) = mpsc::sync_channel(1);
        let request = job.clone();
        let worker = std::thread::Builder::new()
            .name("gamepkg-file-picker".into())
            .spawn(move || {
                let outcome = exchange(
                    &mut socket,
                    index as u32,
                    &request.kind,
                    request.policy.max_bytes as usize,
                )
                .unwrap_or(Outcome::Failed);
                let _ = send.send((socket, outcome));
                wake();
            });
        if worker.is_err() {
            service.complete(&job, Outcome::Failed);
            return false;
        }
        self.pending = Some((job, receive));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_picker_rechecks_policy_and_transfers_only_selected_file_contents() {
        let mut manifest =
            Manifest::parse(include_bytes!("../../examples/minimal/manifest.toml")).unwrap();
        manifest.permissions.files = vec![crate::manifest::UserFile {
            id: "patterns".into(),
            title: "Level pattern text".into(),
            extensions: vec!["txt".into()],
            max_bytes: 1024,
            read: true,
            write: true,
        }];
        let files = tempfile::tempdir().unwrap();
        let input = files.path().join("selected.txt");
        let output = files.path().join("exported.txt");
        std::fs::write(&input, b"1*2\n").unwrap();
        let exported = output.clone();
        let (mut client, server) = UnixStream::pair().unwrap();
        let task = std::thread::spawn(move || {
            serve_with_picker(server, manifest, |job| match job.kind {
                Kind::Import => Some(input.clone()),
                Kind::Export { .. } => Some(output.clone()),
            })
            .unwrap();
        });
        assert!(matches!(
            exchange(&mut client, 99, &Kind::Import, 1024),
            Some(Outcome::Failed)
        ));
        assert!(matches!(
            exchange(
                &mut client,
                0,
                &Kind::Export {
                    name: "../outside.txt".into(),
                    bytes: b"not written".to_vec(),
                },
                1024
            ),
            Some(Outcome::Failed)
        ));
        match exchange(&mut client, 0, &Kind::Import, 1024).unwrap() {
            Outcome::Imported { name, bytes } => {
                assert_eq!(name, "selected.txt");
                assert_eq!(bytes, b"1*2\n");
            }
            _ => panic!("expected a selected import"),
        }
        assert!(matches!(
            exchange(
                &mut client,
                0,
                &Kind::Export {
                    name: "exported.txt".into(),
                    bytes: b"3*4\n".to_vec(),
                },
                1024
            ),
            Some(Outcome::Exported)
        ));
        assert_eq!(std::fs::read(exported).unwrap(), b"3*4\n");
        drop(client);
        task.join().unwrap();
    }
}
