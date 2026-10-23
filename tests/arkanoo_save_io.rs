//! Compile the actual game's streaming codec against a bounded host fixture.
#[allow(dead_code)]
#[path = "../ports/arkanoo/source/src/editor.rs"]
mod editor;
#[allow(dead_code)]
#[path = "../ports/arkanoo/source/src/save_io.rs"]
mod save_io;
mod gamepkg {
    use std::cell::RefCell;
    #[derive(Default)]
    pub struct Host {
        pub bytes: Vec<u8>,
        pub pending: Vec<u8>,
        pub reads: usize,
        pub writes: usize,
        pub fail_read: Option<usize>,
        pub fail_write: Option<usize>,
        pub short_read: bool,
    }
    thread_local! { pub static HOST: RefCell<Host> = RefCell::default(); }
    pub fn reset(bytes: Vec<u8>) {
        HOST.with(|h| {
            *h.borrow_mut() = Host {
                bytes,
                ..Host::default()
            }
        });
    }
    pub fn save_read(offset: u32, output: &mut [u8]) -> Option<u32> {
        HOST.with(|h| {
            let mut h = h.borrow_mut();
            h.reads += 1;
            assert!(
                output.len() <= 4096,
                "streaming loader exceeded its cache size"
            );
            if h.fail_read == Some(h.reads) {
                return if h.short_read {
                    Some(output.len().saturating_sub(1) as u32)
                } else {
                    None
                };
            }
            let end = (offset as usize).checked_add(output.len())?;
            output.copy_from_slice(h.bytes.get(offset as usize..end)?);
            Some(output.len() as u32)
        })
    }
    pub fn save_chunk(offset: u32, total: u32, bytes: &[u8]) -> bool {
        HOST.with(|h| {
            let mut h = h.borrow_mut();
            h.writes += 1;
            assert!(bytes.len() <= 4096);
            assert!(total <= 1048576);
            if h.fail_write == Some(h.writes) {
                return false;
            }
            if offset == 0 {
                h.pending.clear();
            }
            assert_eq!(offset as usize, h.pending.len());
            h.pending.extend_from_slice(bytes);
            assert!(h.pending.len() <= total as usize);
            if h.pending.len() == total as usize {
                h.bytes = std::mem::take(&mut h.pending);
            }
            true
        })
    }
}
fn check_decoder(bytes: &[u8]) {
    let mut reference = editor::Editor::new();
    let expected = reference.save_decode(bytes);
    gamepkg::reset(bytes.to_vec());
    let actual = save_io::load_saved_editor(bytes.len() as u32);
    assert_eq!(actual.as_ref().map(|(_, metadata)| *metadata), expected);
    if let Some((loaded, _)) = actual {
        assert_eq!(
            loaded.save_encode_audio_settings(
                0,
                1,
                false,
                [loaded.sfx_volume, loaded.music_volume],
                [loaded.sfx_muted, loaded.music_muted]
            ),
            reference.save_encode_audio_settings(
                0,
                1,
                false,
                [reference.sfx_volume, reference.music_volume],
                [reference.sfx_muted, reference.music_muted]
            )
        );
    }
    gamepkg::HOST.with(|h| {
        let h = h.borrow();
        assert_eq!(h.bytes, bytes);
        assert_eq!(h.writes, 0);
    });
}
#[test]
fn streaming_load_matches_legacy_validation_and_rejects_damage_without_writes() {
    let mut edit = editor::Editor::new();
    for name in ["alpha", "café", "🕹"] {
        edit.name = editor::Name::from_string(name.to_string());
        edit.cells[0] = 6;
        assert!(edit.editor_store());
        edit.editor_commit(true);
    }
    let archives = [
        edit.save_encode(42, 9, true),
        edit.save_encode_settings(42, 9, true, 17, true),
        edit.save_encode_audio_settings(42, 9, true, [17, 89], [true, true]),
    ];
    for bytes in archives {
        check_decoder(&bytes);
        for length in 0..bytes.len() {
            check_decoder(&bytes[..length]);
        }
        for index in 0..bytes.len() {
            let mut damaged = bytes.clone();
            damaged[index] ^= 255;
            check_decoder(&damaged);
        }
        let mut trailing = bytes;
        trailing.push(0);
        check_decoder(&trailing);
    }
    let mut legacy = b"ARK1".to_vec();
    for word in [42u32, 9, 1] {
        legacy.extend_from_slice(&word.to_le_bytes());
    }
    check_decoder(&legacy);
    check_decoder(&vec![0; 1048577]);
}
#[test]
fn streaming_io_preserves_full_archives_and_stops_on_each_host_failure() {
    let mut edit = editor::Editor::new();
    edit.name = editor::Name::from_string("x".repeat(editor::SAVE_BYTES - 224));
    edit.cells[0] = 6;
    assert!(edit.editor_store());
    edit.editor_commit(true);
    let expected = edit.save_encode_audio_settings(42, 9, true, [17, 89], [true, false]);
    check_decoder(&expected);
    gamepkg::HOST.with(|h| assert_eq!(h.borrow().reads, 256));
    for failed in [1, 2, 128, 256] {
        gamepkg::reset(expected.clone());
        gamepkg::HOST.with(|h| {
            let mut h = h.borrow_mut();
            h.fail_read = Some(failed);
            h.short_read = true;
        });
        assert!(save_io::load_saved_editor(editor::SAVE_BYTES as u32).is_none());
        gamepkg::HOST.with(|h| assert_eq!(h.borrow().reads, failed));
        gamepkg::reset(expected.clone());
        gamepkg::HOST.with(|h| h.borrow_mut().fail_read = Some(failed));
        assert!(save_io::load_saved_editor(editor::SAVE_BYTES as u32).is_none());
        gamepkg::HOST.with(|h| assert_eq!(h.borrow().reads, failed));
        gamepkg::reset(b"old save".to_vec());
        gamepkg::HOST.with(|h| h.borrow_mut().fail_write = Some(failed));
        assert!(!save_io::save_stream_audio_settings(
            &edit,
            42,
            9,
            true,
            [17, 89],
            [true, false]
        ));
        gamepkg::HOST.with(|h| {
            let h = h.borrow();
            assert_eq!(h.bytes, b"old save");
            assert_eq!(h.writes, failed);
        });
    }
    gamepkg::reset(b"old save".to_vec());
    assert!(save_io::save_stream_audio_settings(
        &edit,
        42,
        9,
        true,
        [17, 89],
        [true, false]
    ));
    gamepkg::HOST.with(|h| {
        let h = h.borrow();
        assert_eq!(h.bytes, expected);
        assert_eq!(h.writes, 256);
    });
    gamepkg::reset(Vec::new());
    assert!(save_io::save_stream_settings(&edit, 42, 9, true, 17, true));
    gamepkg::HOST.with(|h| {
        assert_eq!(
            h.borrow().bytes,
            edit.save_encode_settings(42, 9, true, 17, true)
        )
    });
}
