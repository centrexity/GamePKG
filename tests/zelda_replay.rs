use gamepkg::zelda_replay::{LegacyReplay, Recorder, RAM_SIZE};
use std::io::Cursor;
fn file(log: &[u8], frames: u32, base: bool) -> Vec<u8> {
    let h = [
        1,
        frames,
        log.len() as u32,
        0xffff,
        0,
        u32::from(base),
        8,
        0,
    ];
    let mut data = Vec::new();
    for value in h {
        data.extend(value.to_le_bytes());
    }
    data.extend(log);
    if base {
        data.extend([0x31; 8]);
    }
    data.extend([0x72; 8]);
    data
}
#[test]
fn original_timing_toggles_extended_delays_and_wrapped_patches_are_preserved() {
    let log = [
        0x00, 0x23, 0xc2, 0xff, 0xff, 7, 0xce, 0, 0xff, 0xff, 1, 2, 3, 4, 0x0f, 255, 0,
    ];
    let data = file(&log, 280, true);
    let replay = LegacyReplay::parse(&data).unwrap();
    assert_eq!(replay.info().commands, 5);
    assert_eq!(replay.info().memory_patches, 2);
    assert_eq!(replay.base_snapshot(), Some(&[0x31; 8][..]));
    assert_eq!(replay.current_snapshot(), [0x72; 8]);
    let mut cursor = replay.cursor();
    let mut ram = [0; RAM_SIZE];
    for frame in 0..280 {
        let inputs = cursor.next_frame(&mut ram).unwrap().unwrap();
        assert_eq!(
            inputs,
            if frame < 3 {
                1
            } else if frame < 273 {
                5
            } else {
                4
            }
        );
        if frame < 3 {
            assert_eq!(ram[RAM_SIZE - 1], 0);
        } else {
            assert_eq!(ram[RAM_SIZE - 1], 1);
            assert_eq!(&ram[..3], &[2, 3, 4]);
        }
    }
    assert_eq!(cursor.next_frame(&mut ram).unwrap(), None);
    assert_eq!(cursor.frame(), 280);
}
#[test]
fn zero_frame_recording_still_gets_the_original_single_replay_read() {
    let data = file(&[0xb0], 0, false);
    let replay = LegacyReplay::parse(&data).unwrap();
    let mut cursor = replay.cursor();
    let mut ram = [0; RAM_SIZE];
    assert_eq!(cursor.next_frame(&mut ram).unwrap(), Some(1 << 11));
    assert_eq!(cursor.next_frame(&mut ram).unwrap(), None);
}
#[test]
fn failed_engine_patch_delivery_does_not_advance_or_skip_the_pending_patch() {
    let data = file(&[0, 0xc0, 0, 1, 7, 0xc0, 0, 2, 8], 1, false);
    let replay = LegacyReplay::parse(&data).unwrap();
    let mut cursor = replay.cursor();
    let mut applied = Vec::new();
    assert!(cursor
        .next_frame_with_patches(|address, bytes| {
            if address == 2 {
                anyhow::bail!("synthetic transport failure");
            }
            applied.push((address, bytes.to_vec()));
            Ok(())
        })
        .is_err());
    assert_eq!(cursor.frame(), 0);
    assert_eq!(applied, [(1, vec![7])]);
    assert_eq!(
        cursor
            .next_frame_with_patches(|address, bytes| {
                applied.push((address, bytes.to_vec()));
                Ok(())
            })
            .unwrap(),
        Some(1)
    );
    assert_eq!(applied, [(1, vec![7]), (2, vec![8])]);
    assert_eq!(cursor.frame(), 1);
    assert_eq!(
        cursor
            .next_frame_with_patches(|_, _| panic!("callback after final frame"))
            .unwrap(),
        None
    );
}
#[test]
fn replay_framing_and_resume_offsets_fail_before_mutation() {
    let valid = file(&[0x0f, 1, 0xc0, 0, 0, 42], 20, true);
    for end in 0..valid.len() {
        assert!(LegacyReplay::parse(&valid[..end]).is_err(), "prefix {end}");
    }
    let mut extra = valid.clone();
    extra.push(0);
    assert!(LegacyReplay::parse(&extra).is_err());
    for (word, value) in [
        (0, 2),
        (2, u32::MAX),
        (3, 65536),
        (5, 2),
        (6, 0),
        (6, u32::MAX),
        (7, 21),
    ] {
        let mut bad = valid.clone();
        bad[word * 4..word * 4 + 4].copy_from_slice(&value.to_le_bytes());
        assert!(LegacyReplay::parse(&bad).is_err(), "header word {word}");
    }
    let mut resumed = valid;
    resumed[20..24].copy_from_slice(&5u32.to_le_bytes());
    resumed[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(
        LegacyReplay::parse(&resumed).unwrap().info().resume_offset,
        2
    );
    resumed[20..24].copy_from_slice(&3u32.to_le_bytes());
    assert!(
        LegacyReplay::parse(&resumed).is_err(),
        "offset inside delay extension"
    );
}
#[test]
fn every_command_prefix_and_payload_truncation_is_checked() {
    for command in 0u8..=255 {
        let mut log = vec![command];
        if command & if command < 0xc0 { 15 } else { 1 } == if command < 0xc0 { 15 } else { 1 } {
            log.push(0);
        }
        if (0xc0..0xd0).contains(&command) {
            let length = 1 + ((command >> 2) & 3);
            if length == 4 {
                log.push(0);
            }
            log.extend([0xff, 0xff]);
            log.extend(std::iter::repeat_n(0x55, length as usize));
        }
        assert_eq!(
            LegacyReplay::parse(&file(&log, 1, false)).is_ok(),
            command < 0xd0,
            "command {command:02x}"
        );
        for end in 1..log.len() {
            assert!(
                LegacyReplay::parse(&file(&log[..end], 1, false)).is_err(),
                "command {command:02x}, length {end}"
            );
        }
    }
    assert!(LegacyReplay::parse(&file(&[0x0f, 255], 1, false)).is_err());
    assert!(LegacyReplay::parse(&file(&[0xcc, 255], 1, false)).is_err());
}

fn vl(log: &mut Vec<u8>, mut value: u32) {
    while value >= 255 {
        log.push(255);
        value -= 255;
    }
    log.push(value as u8);
}
#[test]
fn streamed_original_recording_round_trips_inputs_patches_and_header() {
    let mut writer = Recorder::new(Cursor::new(Vec::new()), 4096).unwrap();
    writer.record_frame(0x805).unwrap();
    writer.record_patch(RAM_SIZE - 1, &[1, 2, 3, 4, 5]).unwrap();
    for _ in 0..270 {
        writer.record_frame(0x805).unwrap();
    }
    writer.record_frame(0x005).unwrap();
    writer.record_patch(65536, &vec![0x7a; 259]).unwrap();
    writer.record_frame(0x005).unwrap();
    let bytes = writer
        .finish(Some(&[0x31; 8]), &[0x72; 8])
        .unwrap()
        .into_inner();
    let mut original_wire = vec![
        0x00, 0x20, 0xb0, // three first-frame input toggles
        0xcf, 0, 1, 0xff, 0xff, 1, 2, 3, 4, 5, // next-frame five-byte patch
        0xbf, 255, 0, // input bit 11 clears after 270 unchanged frames
        0xcf, 0, 255, 0, 0, 0, // next-frame 259-byte patch header
    ];
    original_wire.extend([0x7a; 259]);
    assert_eq!(&bytes[32..32 + original_wire.len()], original_wire);
    let replay = LegacyReplay::parse(&bytes).unwrap();
    assert_eq!(replay.info().frames, 273);
    assert_eq!(replay.info().recorded_inputs, 0x005);
    assert_eq!(replay.info().frames_since_last, 1);
    assert_eq!(replay.info().commands, 6);
    assert_eq!(replay.info().memory_patches, 2);
    assert_eq!(replay.base_snapshot(), Some(&[0x31; 8][..]));
    assert_eq!(replay.current_snapshot(), &[0x72; 8]);
    let mut cursor = replay.cursor();
    let mut ram = [0; RAM_SIZE];
    for frame in 0..273 {
        let inputs = cursor.next_frame(&mut ram).unwrap().unwrap();
        assert_eq!(inputs, if frame >= 271 { 0x005 } else { 0x805 });
    }
    assert_eq!(ram[RAM_SIZE - 1], 1);
    assert_eq!(&ram[..4], &[2, 3, 4, 5]);
    assert_eq!(&ram[65536..65536 + 259], &[0x7a; 259]);
    assert_eq!(cursor.next_frame(&mut ram).unwrap(), None);
}
#[test]
fn recording_quota_is_atomic_for_a_multi_toggle_frame_and_large_patch() {
    let mut writer = Recorder::new(Cursor::new(Vec::new()), 2).unwrap();
    assert!(writer.record_frame(0x007).is_err());
    writer.record_frame(0x003).unwrap();
    assert!(writer.record_patch(1, &[42]).is_err());
    let bytes = writer.finish(None, &[1]).unwrap().into_inner();
    let replay = LegacyReplay::parse(&bytes).unwrap();
    assert_eq!(replay.info().frames, 1);
    assert_eq!(replay.info().commands, 2);
    assert_eq!(replay.info().memory_patches, 0);
}
#[test]
fn recording_rejects_out_of_range_inputs_addresses_and_snapshot_shape() {
    let mut writer = Recorder::new(Cursor::new(Vec::new()), 16).unwrap();
    assert!(writer.record_frame(0x1000).is_err());
    assert!(writer.record_patch(RAM_SIZE, &[1]).is_err());
    assert!(writer.record_patch(0, &[]).is_err());
    assert!(writer.finish(Some(&[1, 2]), &[3]).is_err());
}
#[test]
fn extended_counts_preserve_the_signed_limit_without_allocating_from_untrusted_counts() {
    let mut log = vec![0x0f];
    vl(&mut log, i32::MAX as u32 - 15);
    let valid = file(&log, 1, false);
    assert_eq!(LegacyReplay::parse(&valid).unwrap().info().commands, 1);
    let count = i32::MAX as usize / 255 + 1;
    log.clear();
    log.push(0x0f);
    log.resize(count + 1, 255);
    log.push(0);
    assert!(LegacyReplay::parse(&file(&log, 1, false)).is_err());
    log[0] = 0xcc;
    assert!(LegacyReplay::parse(&file(&log, 1, false)).is_err());
}
fn timed(log: &mut Vec<u8>, command: u8, delay: u32) {
    let mask = if command < 0xc0 { 15 } else { 1 };
    log.push(command | delay.min(mask) as u8);
    if delay >= mask {
        vl(log, delay - mask);
    }
}
#[test]
#[ignore = "requires the separately compiled pinned original C recorder"]
fn original_recorder_matches_streamed_wire_and_metadata() {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let oracle =
        std::env::var_os("GAMEPKG_ZELDA_REPLAY_ORACLE").expect("missing original recorder fixture");
    let mut child = Command::new(oracle)
        .arg("--record")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let input = child.stdin.as_mut().unwrap();
    input.write_all(&24u32.to_le_bytes()).unwrap();
    for seed in 0..24u32 {
        let mut events: Vec<(u32, u32, Vec<u8>)> = Vec::new();
        if seed & 1 == 0 {
            events.push((1, 131071u32, vec![seed as u8; 4]));
        }
        for frame in 0..512u32 {
            events.push((0, (frame * (seed + 1) + seed) & 0xfff, Vec::new()));
            if frame % 37 == seed % 37 {
                let length = [1, 2, 3, 4, 5, 259, 514][((seed + frame) % 7) as usize];
                let address = [0, 1, 65535, 65536, 131071][((seed + frame) % 5) as usize];
                events.push((
                    1,
                    address,
                    (0..length).map(|n| (n * 17 + frame + seed) as u8).collect(),
                ));
            }
        }
        input
            .write_all(&(events.len() as u32).to_le_bytes())
            .unwrap();
        let mut writer = Recorder::new(Cursor::new(Vec::new()), 65536).unwrap();
        for (tag, value, bytes) in events {
            input.write_all(&tag.to_le_bytes()).unwrap();
            input.write_all(&value.to_le_bytes()).unwrap();
            if tag == 0 {
                writer.record_frame(value as u16).unwrap();
            } else {
                input
                    .write_all(&(bytes.len() as u32).to_le_bytes())
                    .unwrap();
                input.write_all(&bytes).unwrap();
                writer.record_patch(value as usize, &bytes).unwrap();
            }
        }
        input.flush().unwrap();
        let data = writer.finish(None, &[1]).unwrap().into_inner();
        let info = LegacyReplay::parse(&data).unwrap().info();
        let output = child.stdout.as_mut().unwrap();
        let mut header = [0u8; 16];
        output.read_exact(&mut header).unwrap();
        let words: Vec<u32> = header
            .chunks_exact(4)
            .map(|part| u32::from_le_bytes(part.try_into().unwrap()))
            .collect();
        assert_eq!(
            words,
            [
                info.frames,
                info.frames_since_last,
                u32::from(info.recorded_inputs),
                info.log_bytes
            ],
            "seed {seed}"
        );
        let mut original_log = vec![0; info.log_bytes as usize];
        output.read_exact(&mut original_log).unwrap();
        assert_eq!(
            original_log,
            data[32..32 + original_log.len()],
            "seed {seed}"
        );
    }
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}
#[test]
#[ignore = "requires the separately compiled pinned original C reader"]
fn original_reader_matches_all_frame_inputs_ram_hashes_and_final_ram_bytes() {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    struct Oracle(std::process::Child);
    impl Drop for Oracle {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut cases = vec![(Vec::new(), 0), (vec![0xb0], 0), (Vec::new(), 512)];
    for seed in 0..24u32 {
        let mut log = Vec::new();
        for event in 0..48u32 {
            if event % 3 != 0 {
                timed(
                    &mut log,
                    ((seed + event) % 12) as u8 * 16,
                    if event == 47 { 270 } else { (seed + event) % 4 },
                );
            } else {
                let count = [1, 2, 3, 4, 5, 259, 514][((seed + event) / 3 % 7) as usize];
                let address = [0, 1, 65535, 65536, 131071][((seed + event) / 3 % 5) as usize];
                let count_code = (count - 1).min(3);
                timed(
                    &mut log,
                    0xc0 | (((address >> 16) as u8) * 2) | ((count_code as u8) * 4),
                    (seed + event) % 3,
                );
                if count_code == 3 {
                    vl(&mut log, count - 4);
                }
                log.extend([(address >> 8) as u8, address as u8]);
                log.extend((0..count).map(|i| (i * 37 + seed + event) as u8));
            }
        }
        cases.push((log, 512));
    }
    let oracle =
        std::env::var_os("GAMEPKG_ZELDA_REPLAY_ORACLE").expect("missing original reader fixture");
    let mut child = Oracle(
        Command::new(oracle)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&(cases.len() as u32).to_le_bytes())
        .unwrap();
    let mut compared = 0;
    for (log, frames) in cases {
        let data = file(&log, frames, false);
        let replay = LegacyReplay::parse(&data).unwrap();
        let mut cursor = replay.cursor();
        let mut ram = [0u8; RAM_SIZE];
        let input = child.0.stdin.as_mut().unwrap();
        input.write_all(&frames.to_le_bytes()).unwrap();
        input.write_all(&(log.len() as u32).to_le_bytes()).unwrap();
        input.write_all(&log).unwrap();
        input.flush().unwrap();
        let output = child.0.stdout.as_mut().unwrap();
        for _ in 0..frames.max(1) {
            let mut expected = [0u8; 10];
            output.read_exact(&mut expected).unwrap();
            assert_eq!(
                cursor.next_frame(&mut ram).unwrap(),
                Some(u16::from_le_bytes(expected[..2].try_into().unwrap()))
            );
            let hash = ram.iter().fold(14695981039346656037u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(1099511628211)
            });
            assert_eq!(hash, u64::from_le_bytes(expected[2..].try_into().unwrap()));
            compared += 1;
        }
        let mut expected = [0; RAM_SIZE];
        output.read_exact(&mut expected).unwrap();
        assert_eq!(ram, expected);
        assert_eq!(cursor.next_frame(&mut ram).unwrap(), None);
    }
    drop(child.0.stdin.take());
    let mut terminal = String::new();
    child
        .0
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut terminal)
        .unwrap();
    assert_eq!(terminal.trim(), "ORIGINAL_REPLAY_COMPLETE");
    assert!(child.0.wait().unwrap().success());
    assert_eq!(compared, 12802);
}
