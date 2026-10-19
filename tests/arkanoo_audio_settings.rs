//! Portable save codec checks; no SDL or audio device is needed.
#[allow(dead_code)]
#[path = "../ports/arkanoo/source/src/editor.rs"]
mod editor;


#[test]
fn recorded_music_settings_round_trip_and_legacy_defaults_are_atomic() {
    let mut edit = editor::Editor::new();
    edit.name = "café".into();
    edit.cells[0] = 7;
    assert!(edit.editor_store());
    for sfx in [0, 1, 64, 128, u32::MAX] {
        for music in [0, 1, 64, 128, u32::MAX] {
            for sfx_muted in [false, true] {
                for music_muted in [false, true] {
                    let bytes = edit.save_encode_audio_settings(42, 9, true, [sfx, music], [sfx_muted, music_muted]);
                    assert_eq!(&bytes[..4], b"ARK4");
                    let mut loaded = editor::Editor::new();
                    assert_eq!(loaded.save_decode(&bytes), Some([42, 9, 1]));
                    assert_eq!(loaded.patterns[0].name, "café");
                    assert_eq!(loaded.patterns[0].cells[0], 7);
                    assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (sfx.min(128), sfx_muted));
                    assert_eq!((loaded.music_volume, loaded.music_muted), (music.min(128), music_muted));
                    for legacy in [edit.save_encode(1, 2, false), edit.save_encode_settings(1, 2, false, 17, true)] {
                        assert!(loaded.save_decode(&legacy).is_some());
                        assert_eq!((loaded.music_volume, loaded.music_muted), (64, false));
                    }
                }
            }
        }
    }
    let valid = edit.save_encode_audio_settings(42, 9, true, [17, 89], [true, true]);
    let mut loaded = editor::Editor::new();
    assert!(loaded.save_decode(&valid).is_some());
    for flags in (129u32..=255).flat_map(|gain| [gain << 1, gain << 10]).chain((19..32).map(|bit| 1u32 << bit)) {
        let mut invalid = valid.clone();
        invalid[12..16].copy_from_slice(&flags.to_le_bytes());
        assert_eq!(loaded.save_decode(&invalid), None);
        assert_eq!((loaded.music_volume, loaded.music_muted, loaded.sfx_volume, loaded.sfx_muted), (89, true, 17, true));
    }
    for length in 0..valid.len() {
        assert_eq!(loaded.save_decode(&valid[..length]), None);
        assert_eq!((loaded.music_volume, loaded.music_muted), (89, true));
    }
}

#[test]
fn recorded_music_settings_keep_full_archive_capacity_and_ark1_defaults() {
    let mut full = editor::Editor::new();
    full.cells[0] = 6;
    full.name = "x".repeat(editor::SAVE_BYTES - 224);
    assert!(full.editor_store());
    let bytes = full.save_encode_audio_settings(42, 9, false, [128, 128], [true, true]);
    assert_eq!(bytes.len(), editor::SAVE_BYTES);
    let mut loaded = editor::Editor::new();
    assert_eq!(loaded.save_decode(&bytes), Some([42, 9, 0]));
    assert_eq!(loaded.patterns[0].name, full.name);
    let mut legacy = b"ARK1".to_vec();
    for word in [42u32, 9, 0] { legacy.extend_from_slice(&word.to_le_bytes()); }
    assert_eq!(loaded.save_decode(&legacy), Some([42, 9, 0]));
    assert_eq!((loaded.music_volume, loaded.music_muted), (64, false));
    assert!(loaded.patterns.is_empty());
}
