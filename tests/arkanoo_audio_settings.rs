//! Portable save codec checks; no SDL or audio device is needed.
#[allow(dead_code)]
#[path = "../ports/arkanoo/source/src/editor.rs"]
mod editor;

#[test]
fn recorded_music_settings_round_trip_and_legacy_defaults_are_atomic() {
    let mut edit = editor::Editor::new();
    edit.name = editor::Name::from_string(String::from("café"));
    edit.cells[0] = 7;
    assert!(edit.editor_store());
    for sfx in [0, 1, 64, 128, u32::MAX] {
        for music in [0, 1, 64, 128, u32::MAX] {
            for sfx_muted in [false, true] {
                for music_muted in [false, true] {
                    let bytes = edit.save_encode_audio_settings(
                        42,
                        9,
                        true,
                        [sfx, music],
                        [sfx_muted, music_muted],
                    );
                    assert_eq!(&bytes[..4], b"ARK4");
                    let mut loaded = editor::Editor::new();
                    assert_eq!(loaded.save_decode(&bytes), Some([42, 9, 1]));
                    assert_eq!(loaded.patterns[0].name.name_text(), "café");
                    assert_eq!(loaded.patterns[0].pattern_get_cell(0), 7);
                    assert_eq!(
                        (loaded.sfx_volume, loaded.sfx_muted),
                        (sfx.min(128), sfx_muted)
                    );
                    assert_eq!(
                        (loaded.music_volume, loaded.music_muted),
                        (music.min(128), music_muted)
                    );
                    for legacy in [
                        edit.save_encode(1, 2, false),
                        edit.save_encode_settings(1, 2, false, 17, true),
                    ] {
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
    for flags in (129u32..=255)
        .flat_map(|gain| [gain << 1, gain << 10])
        .chain((19..32).map(|bit| 1u32 << bit))
    {
        let mut invalid = valid.clone();
        invalid[12..16].copy_from_slice(&flags.to_le_bytes());
        assert_eq!(loaded.save_decode(&invalid), None);
        assert_eq!(
            (
                loaded.music_volume,
                loaded.music_muted,
                loaded.sfx_volume,
                loaded.sfx_muted
            ),
            (89, true, 17, true)
        );
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
    full.name = editor::Name::from_string("x".repeat(editor::SAVE_BYTES - 224));
    assert!(full.editor_store());
    let bytes = full.save_encode_audio_settings(42, 9, false, [128, 128], [true, true]);
    assert_eq!(bytes.len(), editor::SAVE_BYTES);
    assert_eq!(
        bytes.capacity(),
        editor::SAVE_BYTES,
        "a full archive must not reserve a second archive's worth of guest memory"
    );
    let mut loaded = editor::Editor::new();
    assert_eq!(loaded.save_decode(&bytes), Some([42, 9, 0]));
    assert_eq!(loaded.patterns[0].name.name_text(), full.name.name_text());
    let mut legacy = b"ARK1".to_vec();
    for word in [42u32, 9, 0] {
        legacy.extend_from_slice(&word.to_le_bytes());
    }
    assert_eq!(loaded.save_decode(&legacy), Some([42, 9, 0]));
    assert_eq!((loaded.music_volume, loaded.music_muted), (64, false));
    assert!(loaded.patterns.is_empty());
}

#[test]
fn owned_archive_reuses_the_large_name_buffer_and_rejects_damage_atomically() {
    let mut bytes = b"ARK4".to_vec();
    for word in [42u32, 9, 0, 1, (editor::SAVE_BYTES - 224) as u32] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.resize(editor::SAVE_BYTES - 200, b'x');
    bytes.resize(editor::SAVE_BYTES, 6);
    let pointer = bytes.as_ptr();
    let mut loaded = editor::Editor::new();
    assert_eq!(loaded.save_decode_owned(bytes.clone()), Some([42, 9, 0]));
    let mut damaged = bytes.clone();
    damaged[24] = 255;
    assert_eq!(loaded.save_decode_owned(damaged), None);
    assert_eq!(loaded.patterns[0].name.len(), editor::SAVE_BYTES - 224);
    assert_eq!(loaded.save_decode_owned(bytes), Some([42, 9, 0]));
    assert_eq!(loaded.patterns[0].name.name_segments()[0].as_ptr(), pointer);
    assert_eq!(loaded.patterns[0].pattern_unpack(), [6; 200]);
    let expected = loaded.save_encode_audio_settings(42, 9, true, [17, 89], [true, false]);
    let lent = loaded.save_take_encode_audio_settings(42, 9, true, [17, 89], [true, false]);
    assert_eq!(lent.as_ptr(), pointer);
    assert_eq!(lent, expected);
    loaded.save_restore_encoded(lent);
    assert_eq!(loaded.patterns[0].name.name_segments()[0].as_ptr(), pointer);
    assert_eq!(loaded.patterns[0].name.len(), editor::SAVE_BYTES - 224);
}

#[test]
fn lent_save_preserves_zero_multiple_and_failed_editor_transactions() {
    let mut edit = editor::Editor::new();
    for count in 0..3 {
        let expected = edit.save_encode_settings(7, 3, true, 17, true);
        let lent = edit.save_take_encode_settings(7, 3, true, 17, true);
        assert_eq!(lent, expected);
        edit.save_restore_encoded(lent);
        assert_eq!(edit.save_encode_settings(7, 3, true, 17, true), expected);
        edit.name = editor::Name::from_string(format!("pattern-{count}"));
        edit.cells[0] = count;
        assert!(edit.editor_store());
        edit.editor_commit(true);
    }
    let before = edit.save_encode(7, 3, false);
    edit.name = editor::Name::from_string(String::from("pattern-0"));
    edit.cells[0] = 8;
    assert!(edit.editor_store());
    let lent = edit.save_take_encode_settings(7, 3, false, 64, false);
    edit.save_restore_encoded(lent);
    edit.editor_commit(false);
    assert_eq!(edit.save_encode(7, 3, false), before);
}

#[test]
fn editor_undo_reverses_interleaved_insertions_and_repeated_replacements() {
    let mut edit = editor::Editor::new();
    for name in ["b", "d"] {
        edit.name = editor::Name::from_string(name.to_string());
        edit.cells[0] = 6;
        assert!(edit.editor_store());
        edit.editor_commit(true);
    }
    let before = edit.save_encode(7, 3, false);
    let names: Vec<_> = edit
        .patterns
        .iter()
        .map(|p| p.name.name_segments()[0].as_ptr())
        .collect();
    for (name, cell) in [("d", 1), ("a", 2), ("b", 3), ("c", 4), ("d", 5), ("a", 7)] {
        edit.name = editor::Name::from_string(name.to_string());
        edit.cells[0] = cell;
        assert!(edit.editor_store());
    }
    edit.editor_commit(false);
    assert_eq!(edit.save_encode(7, 3, false), before);
    assert_eq!(
        edit.patterns
            .iter()
            .map(|p| p.name.name_segments()[0].as_ptr())
            .collect::<Vec<_>>(),
        names,
        "rollback must retain existing name allocations"
    );
    assert_eq!(
        edit.cells[0], 7,
        "failed storage must retain the working board"
    );
}

#[test]
fn loading_shares_name_storage_and_typing_preserves_the_archived_identifier() {
    assert!(std::mem::size_of::<editor::Name>() <= std::mem::size_of::<String>());
    let mut edit = editor::Editor::new();
    edit.name = editor::Name::from_string(String::from("café🕹"));
    edit.cells[0] = 6;
    assert!(edit.editor_store());
    edit.editor_commit(true);
    let bytes = edit.save_encode(7, 3, false);
    let mut loaded = editor::Editor::new();
    loaded.save_decode(&bytes).unwrap();
    let pointer = loaded.patterns[0].name.name_segments()[0].as_ptr();
    loaded.editor_load();
    assert!(loaded.name.shares_storage(&loaded.patterns[0].name));
    assert_eq!(loaded.name.name_segments()[0].as_ptr(), pointer);
    loaded.naming = true;
    loaded.editor_key(8);
    loaded.editor_event(7, 0.0, 0.0, '😀' as u32);
    assert_eq!(loaded.name.name_text(), "café😀");
    assert_eq!(loaded.patterns[0].name.name_text(), "café🕹");
    assert_eq!(loaded.patterns[0].name.name_segments()[0].as_ptr(), pointer);
    assert_eq!(loaded.save_encode(7, 3, false), bytes);
}

#[test]
fn large_name_deletion_and_restoration_share_utf8_storage_without_changing_the_archive() {
    let original = "café🕹".repeat(10000);
    let mut working = editor::Name::from_string(original.clone());
    let archived = working.clone();
    let pointer = working.name_segments()[0].as_ptr();
    working.pop();
    assert_eq!(working.name_text(), &original[..original.len() - 4]);
    assert!(working.shares_storage(&archived));
    assert_eq!(working.name_segments()[0].as_ptr(), pointer);
    assert_eq!(archived.name_text(), original);
    working.push('🕹');
    assert_eq!(working.name_text(), original);
    assert!(working.shares_storage(&archived));
    working.pop();
    working.push('😀');
    assert_eq!(
        working.name_segments()[0].as_ptr(),
        pointer,
        "a replacement must retain the existing prefix allocation"
    );
    assert_eq!(working.name_segments()[1], "😀");
    assert_eq!(
        working.name_text(),
        format!("{}😀", &original[..original.len() - 4])
    );
    assert_eq!(archived.name_text(), original);
    assert!(!working.shares_storage(&archived));
    working.pop();
    let expected = working.name_text();
    assert_eq!(working.into_string(), expected);
}

#[test]
fn edited_name_segments_sort_and_serialize_like_flat_utf8_and_remain_isolated() {
    let original = format!("{}🕹", "a".repeat(200));
    let changed = format!("{}😀", "a".repeat(200));
    let mut edit = editor::Editor::new();
    edit.name = editor::Name::from_string(original.clone());
    edit.cells[0] = 6;
    assert!(edit.editor_store());
    edit.editor_commit(true);
    edit.editor_load();
    edit.name.pop();
    edit.name.push('😀');
    assert_eq!(edit.name.name_text(), changed);
    assert!(edit.editor_store());
    edit.editor_commit(true);
    let mut reference = editor::Editor::new();
    for label in [&original, &changed] {
        reference.name = editor::Name::from_string(label.clone());
        reference.cells[0] = 6;
        assert!(reference.editor_store());
        reference.editor_commit(true);
    }
    assert_eq!(
        edit.save_encode_audio_settings(7, 3, true, [17, 89], [true, false]),
        reference.save_encode_audio_settings(7, 3, true, [17, 89], [true, false])
    );
    let before = edit.save_encode(7, 3, false);
    edit.name.pop();
    edit.name.push('x');
    assert_eq!(
        edit.save_encode(7, 3, false),
        before,
        "typing into a shared edited name must preserve the stored suffix"
    );
    for left in [&original, &changed, "abc"] {
        for right in [&original, &changed, "abc"] {
            let a = editor::Name::from_string(left.to_string());
            let b = editor::Name::from_string(right.to_string());
            assert_eq!(a.equals(&b), left == right);
            assert_eq!(a.less_than(&b), left < right);
        }
    }
}

#[test]
fn packed_boards_keep_every_cell_and_wire_byte_and_reject_invalid_values_atomically() {
    assert!(std::mem::size_of::<editor::Pattern>() <= std::mem::size_of::<editor::Name>() + 104);
    for a in [0, 1, 2, 3, 4, 5, 6, 7, 8, 255] {
        for b in [0, 1, 2, 3, 4, 5, 6, 7, 8, 255] {
            let mut cells = [0; 200];
            for (index, cell) in cells.iter_mut().enumerate() {
                *cell = if index % 2 == 0 { a } else { b };
            }
            let pattern = editor::Pattern::pattern_from_cells(
                editor::Name::from_string(String::from("board")),
                cells,
            )
            .unwrap();
            assert_eq!(pattern.pattern_unpack(), cells);
            for (index, cell) in cells.iter().enumerate() {
                assert_eq!(pattern.pattern_get_cell(index), *cell);
            }
            let mut edit = editor::Editor::new();
            edit.patterns.push(pattern);
            let bytes = edit.save_encode(7, 3, false);
            assert_eq!(&bytes[29..], &cells);
        }
    }
    let cells = [6; 200];
    let mut pattern = editor::Pattern::pattern_from_cells(
        editor::Name::from_string(String::from("board")),
        cells,
    )
    .unwrap();
    for invalid in 9..255 {
        let mut damaged = cells;
        damaged[37] = invalid;
        assert!(!pattern.pattern_pack(damaged));
        assert_eq!(pattern.pattern_unpack(), cells);
        assert!(editor::Pattern::pattern_from_cells(
            editor::Name::from_string(String::from("bad")),
            damaged
        )
        .is_none());
    }
    let mut edit = editor::Editor::new();
    edit.cells[0] = 9;
    assert!(!edit.editor_store());
    assert!(edit.patterns.is_empty());
    assert!(!edit.save_requested);
}
