//! Independent logical cursors over one already-open package file.
//! Cloning never reopens a path or shares an unprotected seek position.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    sync::{Arc, Mutex},
};
#[derive(Clone, Debug)]
pub(super) struct CursorFile {
    file: Arc<Mutex<File>>,
    position: u64,
}
impl CursorFile {
    pub(super) fn new(file: File) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
            position: 0,
        }
    }
    pub(super) fn length(&self) -> io::Result<u64> {
        self.file
            .lock()
            .map_err(|_| io::Error::other("package file unavailable"))?
            .metadata()
            .map(|metadata| metadata.len())
    }
}
impl Read for CursorFile {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        let limit = output.len().min(65536);
        let count = {
            let mut file = self
                .file
                .lock()
                .map_err(|_| io::Error::other("package file unavailable"))?;
            file.seek(SeekFrom::Start(self.position))?;
            file.read(&mut output[..limit])?
        };
        self.position = self.position.checked_add(count as u64).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "package position overflow")
        })?;
        Ok(count)
    }
}
impl Seek for CursorFile {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.length()?.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid package seek"))?;
        self.position = next;
        Ok(next)
    }
}
