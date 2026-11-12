//! Borrowed decoding of the original little-endian Zelda desktop replay file.
//! Untrusted lengths never allocate storage; successful decoding borrows input.
//! This validates framing and commands, not the embedded engine snapshots.
//! It is a host API; guest packages cannot import this crate.
use anyhow::{ensure, Context, Result};
use std::io::{Seek, SeekFrom, Write};

const HEADER: usize = 32;
pub const RAM_SIZE: usize = 131072;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Info {
    pub frames: u32,
    pub log_bytes: u32,
    pub recorded_inputs: u16,
    pub frames_since_last: u32,
    pub resume_offset: u32,
    pub resume_frame: u32,
    pub snapshot_bytes: u32,
    pub has_base_snapshot: bool,
    pub commands: u64,
    pub memory_patches: u64,
}
pub struct LegacyReplay<'a> {
    info: Info,
    log: &'a [u8],
    base: Option<&'a [u8]>,
    current: &'a [u8],
}
impl<'a> LegacyReplay<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        ensure!(bytes.len() >= HEADER, "truncated replay header");
        let mut h = [0u32; 8];
        for (value, word) in h.iter_mut().zip(bytes[..HEADER].as_chunks::<4>().0.iter()) {
            *value = u32::from_le_bytes(*word);
        }
        ensure!(h[0] == 1, "unsupported original replay version");
        let recorded_inputs = u16::try_from(h[3]).context("invalid recorded input mask")?;
        ensure!(h[6] != 0, "missing replay engine snapshot");
        let log_end = HEADER
            .checked_add(h[2] as usize)
            .context("replay extent overflow")?;
        let base_end = log_end
            .checked_add(if h[5] & 1 != 0 { h[6] as usize } else { 0 })
            .context("replay extent overflow")?;
        let end = base_end
            .checked_add(h[6] as usize)
            .context("replay extent overflow")?;
        ensure!(end == bytes.len(), "truncated replay or trailing data");
        let log = &bytes[HEADER..log_end];
        let resume_offset = h[5] >> 1;
        ensure!(
            resume_offset as usize <= log.len(),
            "replay resume offset exceeds log"
        );
        ensure!(h[7] <= h[1], "replay resume frame exceeds duration");
        ensure!(
            h[7] != 0 || resume_offset == 0,
            "replay offset without an active resume frame"
        );
        let mut offset = 0;
        let mut boundary = resume_offset == 0;
        let mut at = 0u64;
        let mut commands = 0;
        let mut patches = 0;
        while offset < log.len() {
            let command = decode(log, &mut offset)?;
            at = at
                .checked_add(u64::from(command.delay))
                .context("replay timeline overflow")?;
            commands += 1;
            patches += u64::from(matches!(command.action, Action::Patch { .. }));
            boundary |= offset == resume_offset as usize;
        }
        ensure!(boundary, "replay resume offset splits a command");
        Ok(Self {
            info: Info {
                frames: h[1],
                log_bytes: h[2],
                recorded_inputs,
                frames_since_last: h[4],
                resume_offset,
                resume_frame: h[7],
                snapshot_bytes: h[6],
                has_base_snapshot: h[5] & 1 != 0,
                commands,
                memory_patches: patches,
            },
            log,
            base: (h[5] & 1 != 0).then_some(&bytes[log_end..base_end]),
            current: &bytes[base_end..end],
        })
    }
    pub fn info(&self) -> Info {
        self.info
    }
    /// Untrusted original payload; it must be validated before engine restore.
    pub fn base_snapshot(&self) -> Option<&'a [u8]> {
        self.base
    }
    /// Untrusted original payload; framing validation is not engine validation.
    pub fn current_snapshot(&self) -> &'a [u8] {
        self.current
    }
    /// Begin from frame zero, as the original Replay action does. The caller
    /// supplies the already initialized/restored 128-KiB game RAM separately.
    /// The original reader performs one read even when the saved duration is zero.
    pub fn cursor(&self) -> Cursor<'a> {
        Cursor {
            log: self.log,
            offset: 0,
            pending: None,
            at: 0,
            frame: 0,
            frames: self.info.frames.max(1),
            inputs: 0,
        }
    }
}
#[derive(Clone, Copy)]
enum Action<'a> {
    Toggle(u16),
    Patch { address: usize, bytes: &'a [u8] },
}
#[derive(Clone, Copy)]
struct Command<'a> {
    delay: u32,
    action: Action<'a>,
}
fn byte(log: &[u8], offset: &mut usize) -> Result<u8> {
    let value = *log.get(*offset).context("truncated replay command")?;
    *offset += 1;
    Ok(value)
}
fn extended(log: &[u8], offset: &mut usize, mut value: u32) -> Result<u32> {
    loop {
        let part = byte(log, offset)?;
        value = value
            .checked_add(u32::from(part))
            .filter(|n| *n <= i32::MAX as u32)
            .context("replay command exceeds original signed range")?;
        if part != 255 {
            return Ok(value);
        }
    }
}
fn decode<'a>(log: &'a [u8], offset: &mut usize) -> Result<Command<'a>> {
    let command = byte(log, offset)?;
    ensure!(command < 0xd0, "invalid replay command");
    let mask = if command < 0xc0 { 15 } else { 1 };
    let mut delay = u32::from(command & mask);
    if delay == u32::from(mask) {
        delay = extended(log, offset, delay)?;
    }
    let action = if command < 0xc0 {
        Action::Toggle(1u16 << (command >> 4))
    } else {
        let mut count = 1 + u32::from((command >> 2) & 3);
        if count == 4 {
            count = extended(log, offset, count)?;
        }
        let address = (usize::from((command >> 1) & 1) << 16)
            | (usize::from(byte(log, offset)?) << 8)
            | usize::from(byte(log, offset)?);
        let end = offset
            .checked_add(count as usize)
            .context("replay patch extent overflow")?;
        let bytes = log
            .get(*offset..end)
            .context("truncated replay RAM patch")?;
        *offset = end;
        Action::Patch { address, bytes }
    };
    Ok(Command { delay, action })
}
pub struct Cursor<'a> {
    log: &'a [u8],
    offset: usize,
    pending: Option<Command<'a>>,
    at: u64,
    frame: u32,
    frames: u32,
    inputs: u16,
}
impl Cursor<'_> {
    pub fn frame(&self) -> u32 {
        self.frame
    }
    /// Apply exactly this frame's original toggle/patch commands. Patch writes
    /// wrap at 17 bits, matching the original reader. No engine is called here.
    pub fn next_frame(&mut self, ram: &mut [u8; RAM_SIZE]) -> Result<Option<u16>> {
        self.next_frame_with_patches(|mut address, bytes| {
            for value in bytes {
                ram[address] = *value;
                address = (address + 1) & (RAM_SIZE - 1);
            }
            Ok(())
        })
    }
    /// Deliver patches directly to a host-owned engine bridge. A failed callback
    /// leaves the frame unadvanced and the failed patch pending; earlier commands
    /// remain applied. Retrying resumes that patch before processing later ones.
    pub fn next_frame_with_patches(
        &mut self,
        mut apply: impl FnMut(usize, &[u8]) -> Result<()>,
    ) -> Result<Option<u16>> {
        if self.frame >= self.frames {
            return Ok(None);
        }
        loop {
            if self.pending.is_none() && self.offset < self.log.len() {
                let next = decode(self.log, &mut self.offset)?;
                self.at = self
                    .at
                    .checked_add(u64::from(next.delay))
                    .context("replay timeline overflow")?;
                self.pending = Some(next);
            }
            let Some(next) = self.pending else { break };
            if self.at > u64::from(self.frame) {
                break;
            }
            match next.action {
                Action::Toggle(bit) => self.inputs ^= bit,
                Action::Patch { address, bytes } => apply(address, bytes)?,
            }
            self.pending = None;
        }
        self.frame += 1;
        Ok(Some(self.inputs))
    }
}

/// Stream the original version-1 replay format into a fresh, host-owned file.
/// The caller supplies a storage quota; a failed write leaves the file unusable.
/// Call `record_frame` before any patches made during that frame, as upstream does.
pub struct Recorder<W: Write + Seek> {
    output: W,
    max_log_bytes: u32,
    log_bytes: u32,
    frames: u32,
    frames_since_last: u32,
    inputs: u16,
    failed: bool,
}
impl<W: Write + Seek> Recorder<W> {
    pub fn new(mut output: W, max_log_bytes: u32) -> Result<Self> {
        ensure!(
            output.stream_position()? == 0,
            "replay output must be empty"
        );
        ensure!(
            output.seek(SeekFrom::End(0))? == 0,
            "replay output must be empty"
        );
        output.write_all(&[0; HEADER])?;
        Ok(Self {
            output,
            max_log_bytes,
            log_bytes: 0,
            frames: 0,
            frames_since_last: 0,
            inputs: 0,
            failed: false,
        })
    }
    fn write_command(&mut self, command: u8, delay: u32, payload: &[u8]) -> Result<()> {
        ensure!(!self.failed, "replay writer failed earlier");
        let mask = if command < 0xc0 { 15 } else { 1 };
        let extension = delay.saturating_sub(mask);
        let extension_bytes = if delay >= mask {
            u64::from(extension) / 255 + 1
        } else {
            0
        };
        let size = 1u64 + extension_bytes + payload.len() as u64;
        ensure!(
            u64::from(self.log_bytes) + size <= u64::from(self.max_log_bytes),
            "replay storage quota exceeded"
        );
        let result = (|| -> std::io::Result<()> {
            self.output
                .write_all(&[command | (delay.min(mask) as u8)])?;
            if delay >= mask {
                Self::write_extension(&mut self.output, extension)?;
            }
            self.output.write_all(payload)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result?;
        self.log_bytes += size as u32;
        self.frames_since_last = 0;
        Ok(())
    }
    fn write_extension(output: &mut W, mut remaining: u32) -> std::io::Result<()> {
        while remaining >= 255 {
            output.write_all(&[255])?;
            remaining -= 255;
        }
        output.write_all(&[remaining as u8])
    }
    pub fn record_frame(&mut self, inputs: u16) -> Result<()> {
        ensure!(!self.failed, "replay writer failed earlier");
        ensure!(inputs & !0x0fff == 0, "invalid replay input bits");
        ensure!(
            self.frames < u32::MAX && self.frames_since_last < u32::MAX,
            "replay duration exceeded"
        );
        let difference = self.inputs ^ inputs;
        let toggles = difference.count_ones() as u64;
        let first_size = if self.frames_since_last >= 15 {
            2 + u64::from(self.frames_since_last - 15) / 255
        } else {
            1
        };
        let size = if toggles == 0 {
            0
        } else {
            first_size + toggles - 1
        };
        ensure!(
            u64::from(self.log_bytes) + size <= u64::from(self.max_log_bytes),
            "replay storage quota exceeded"
        );
        for bit in 0..12 {
            if difference & (1 << bit) != 0 {
                self.write_command(bit << 4, self.frames_since_last, &[])?;
            }
        }
        self.inputs = inputs;
        self.frames_since_last += 1;
        self.frames += 1;
        Ok(())
    }
    pub fn record_patch(&mut self, address: usize, bytes: &[u8]) -> Result<()> {
        ensure!(address < RAM_SIZE, "replay RAM address exceeds 17 bits");
        ensure!(
            !bytes.is_empty() && bytes.len() <= i32::MAX as usize,
            "invalid replay patch length"
        );
        let count = bytes.len() as u32;
        let code = (count - 1).min(3);
        // Preflight the complete command before writing its first byte.
        let delay = self.frames_since_last;
        let extension_bytes = if delay >= 1 {
            u64::from(delay - 1) / 255 + 1
        } else {
            0
        };
        let count_bytes = if code == 3 {
            u64::from(count - 4) / 255 + 1
        } else {
            0
        };
        let size = 1 + extension_bytes + count_bytes + 2 + u64::from(count);
        ensure!(
            u64::from(self.log_bytes) + size <= u64::from(self.max_log_bytes),
            "replay storage quota exceeded"
        );
        let command = 0xc0 | (((address >> 16) as u8) << 1) | ((code as u8) << 2);
        let result = (|| -> std::io::Result<()> {
            self.output.write_all(&[command | delay.min(1) as u8])?;
            if delay >= 1 {
                Self::write_extension(&mut self.output, delay - 1)?;
            }
            if code == 3 {
                Self::write_extension(&mut self.output, count - 4)?;
            }
            self.output
                .write_all(&[(address >> 8) as u8, address as u8])?;
            self.output.write_all(bytes)
        })();
        if let Err(error) = result {
            self.failed = true;
            return Err(error.into());
        }
        self.log_bytes += size as u32;
        self.frames_since_last = 0;
        Ok(())
    }
    pub fn finish(mut self, base: Option<&[u8]>, current: &[u8]) -> Result<W> {
        ensure!(!self.failed, "replay writer failed earlier");
        ensure!(!current.is_empty(), "missing replay engine snapshot");
        ensure!(
            current.len() <= u32::MAX as usize,
            "replay snapshot too large"
        );
        ensure!(
            base.is_none_or(|bytes| bytes.len() == current.len()),
            "base snapshot size mismatch"
        );
        self.output.write_all(base.unwrap_or_default())?;
        self.output.write_all(current)?;
        let end = self.output.stream_position()?;
        self.output.seek(SeekFrom::Start(0))?;
        for word in [
            1,
            self.frames,
            self.log_bytes,
            u32::from(self.inputs),
            self.frames_since_last,
            u32::from(base.is_some()),
            current.len() as u32,
            0,
        ] {
            self.output.write_all(&word.to_le_bytes())?;
        }
        self.output.seek(SeekFrom::Start(end))?;
        self.output.flush()?;
        Ok(self.output)
    }
}
