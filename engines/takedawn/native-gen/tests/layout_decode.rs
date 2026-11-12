use gamepkg_takedawn_media::Budget;
use gamepkg_takedawn_native_gen::{
    decode::{blocks, Decoder, MAX_FUNCTION},
    xbe::Image,
};
use std::collections::BTreeSet;

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn fixture(debug: bool) -> Vec<u8> {
    let mut bytes = vec![0; 0x3000];
    bytes[..4].copy_from_slice(b"XBEH");
    for (at, value) in [
        (0x104, 0x10000),
        (0x108, 0x1000),
        (0x10c, 0x4000),
        (0x11c, 2),
        (0x120, 0x10200),
    ] {
        put(&mut bytes, at, value);
    }
    put(
        &mut bytes,
        0x128,
        0x11000 ^ if debug { 0x94859d4b } else { 0xa8fc57ab },
    );
    put(
        &mut bytes,
        0x158,
        0x12000 ^ if debug { 0xefb1f152 } else { 0x5b6d40b6 },
    );
    for (i, va, raw, size, name) in [
        (0, 0x11000, 0x1000, 0x80, 0x10300),
        (1, 0x12000, 0x2000, 0x100, 0x10310),
    ] {
        let at = 0x200 + i * 56;
        for (offset, value) in [
            (0, 4),
            (4, va),
            (8, 0x100),
            (12, raw),
            (16, size),
            (20, name),
        ] {
            put(&mut bytes, at + offset, value);
        }
    }
    bytes[0x300..0x306].copy_from_slice(b".text\0");
    bytes[0x310..0x317].copy_from_slice(b".rdata\0");
    bytes[0x1000] = 0xc3;
    bytes
}
#[test]
fn retail_debug_layout_and_backed_code_data_bss_spans() {
    for debug in [false, true] {
        let bytes = fixture(debug);
        let image = Image::parse(&bytes).unwrap();
        assert_eq!(
            (image.entry, image.kernel_thunk, image.sections.len()),
            (0x11000, 0x12000, 2)
        );
        assert!(image.sections[0].is_code);
        assert!(!image.sections[1].is_code);
        assert_eq!(image.code_bytes(0x11000, 1).unwrap(), [0xc3]);
        assert!(image.bytes_at(0x12000, 4).is_ok());
        for (address, size) in [(0x11080, 1), (0x1107f, 2), (0xffffffff, 2), (0, 1)] {
            assert!(image.code_bytes(address, size).is_err());
        }
        assert!(image.code_bytes(0x12000, 4).is_err());
    }
}
#[test]
fn malformed_headers_names_overlaps_extents_and_entries_are_rejected() {
    let baseline = fixture(false);
    for (at, value) in [
        (0, 0),
        (0x104, 0xfffff000),
        (0x108, 0x3001),
        (0x11c, 0),
        (0x11c, 65),
        (0x120, 0xffff),
        (0x120, 0x10fff),
        (0x128, 0),
        (0x128, 0x12000 ^ 0xa8fc57ab),
        (0x204, 0xfffffff0),
        (0x208, u32::MAX),
        (0x20c, 0x800),
        (0x20c, 0x3000),
        (0x214, 0xffff),
        (0x214, 0x11000),
        (0x23c, 0x11040),
        (0x244, 0x1040),
        (0x158, 0x120ff ^ 0x5b6d40b6),
    ] {
        let mut broken = baseline.clone();
        put(&mut broken, at, value);
        assert!(
            Image::parse(&broken).is_err(),
            "field {at:#x} value {value:#x}"
        );
    }
    for length in [0, 3, 0x177, 0x1000, 0x20ff] {
        assert!(Image::parse(&baseline[..length]).is_err());
    }
    let mut broken = baseline.clone();
    broken[0x300..0x320].fill(b'X');
    assert!(Image::parse(&broken).is_err());
    let mut broken = baseline;
    broken[0x300] = b'\n';
    assert!(Image::parse(&broken).is_err());
}
#[test]
fn operands_register_effects_segments_negative_displacements_and_branches() {
    let budget = Budget::new(128 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let bytes = hex::decode("5589e5648b05000000008b448bf0b8ffffffffe800000000e2fec3").unwrap();
    let decoded = decoder.linear(&bytes, 0x1000, &[]).unwrap();
    let insns = decoded.instructions();
    assert_eq!(insns.len(), 8);
    assert!(insns[0].regs_written.iter().any(|reg| reg == "esp"));
    assert!(insns[1].regs_written.iter().any(|reg| reg == "ebp"));
    assert_eq!(insns[2].operands[1].mem_seg.as_deref(), Some("fs"));
    assert_eq!(insns[2].operands[1].mem_size, 4);
    assert!(insns[2].memory_refs.is_empty());
    let mem = &insns[3].operands[1];
    assert_eq!(
        (
            mem.mem_base.as_deref(),
            mem.mem_index.as_deref(),
            mem.mem_scale,
            mem.mem_disp
        ),
        (Some("ebx"), Some("ecx"), 4, -16)
    );
    assert_eq!(insns[4].imm_values, [u32::MAX]);
    assert_eq!(insns[5].call_target, Some(insns[5].end()));
    assert!(insns[6].is_conditional());
    assert_eq!(insns[6].jump_target, Some(insns[6].address));
    assert!(insns[7].is_ret());
    let held = budget.used();
    drop(decoder);
    assert!(budget.used() > 0 && budget.used() < held);
    drop(decoded);
    assert_eq!(budget.used(), 0);
}
#[test]
fn cfg_blocks_calls_and_resynchronization_preserve_control_flow() {
    let budget = Budget::new(128 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let stops = BTreeSet::new();
    let bytes = hex::decode("85c0740190c3").unwrap();
    let decoded = decoder
        .cfg(&bytes, 0x1000, &[0x1000], &BTreeSet::new(), &stops)
        .unwrap();
    let basic = blocks(&decoded, 0x1000, 0x1006, &[], &stops).unwrap();
    assert_eq!(
        basic.iter().map(|b| b.start).collect::<Vec<_>>(),
        [0x1000, 0x1004, 0x1005]
    );
    assert_eq!(basic[0].successors, [0x1004, 0x1005]);
    assert_eq!(basic[1].successors, [0x1005]);
    assert!(basic[2].successors.is_empty());
    let bytes = hex::decode("eb040fffffff b801000000c3".replace(' ', "")).unwrap();
    let decoded = decoder
        .cfg(&bytes, 0x1000, &[0x1000], &BTreeSet::new(), &stops)
        .unwrap();
    assert_eq!(
        decoded
            .instructions()
            .iter()
            .map(|i| i.address)
            .collect::<Vec<_>>(),
        [0x1000, 0x1006, 0x100b]
    );
    let decoded = decoder
        .linear(&[0xb8, 0x90, 0x90, 0x90, 0xc3], 0x1000, &[0x1001])
        .unwrap();
    assert_eq!(
        decoded
            .instructions()
            .iter()
            .map(|i| i.address)
            .collect::<Vec<_>>(),
        [0x1001, 0x1002, 0x1003, 0x1004]
    );
    let basic = blocks(&decoded, 0x1000, 0x1005, &[0x1001], &stops).unwrap();
    assert_eq!(basic.len(), 1);
    assert!(basic[0].successors.is_empty());
    let decoded = decoder
        .linear(&hex::decode("e80000000090c3").unwrap(), 0x1000, &[])
        .unwrap();
    let basic = blocks(&decoded, 0x1000, 0x1007, &[], &stops).unwrap();
    assert_eq!(basic.len(), 2);
    assert_eq!(basic[0].successors, [0x1005]);
    let stop_names = BTreeSet::from(["int3".to_owned()]);
    let decoded = decoder
        .cfg(
            &[0xcc, 0x90, 0xc3],
            0x1000,
            &[0x1000],
            &BTreeSet::new(),
            &stop_names,
        )
        .unwrap();
    assert_eq!(decoded.instructions().len(), 1);
}
#[test]
fn invalid_spans_metadata_and_budget_fail_without_leaking_reservations() {
    assert!(Decoder::new(&Budget::new(1)).is_err());
    let budget = Budget::new(8 * 1024 * 1024);
    let decoder = Decoder::new(&budget).unwrap();
    let held = budget.used();
    assert!(decoder.linear(&[], 0x1000, &[]).is_err());
    assert!(decoder.linear(&[0x90, 0x90], u32::MAX - 1, &[]).is_err());
    assert!(decoder
        .linear(&vec![0x90; MAX_FUNCTION + 1], 0x1000, &[])
        .is_err());
    assert!(decoder
        .linear(&[0x90], 0x1000, &vec![0; MAX_FUNCTION + 1])
        .is_err());
    assert!(decoder
        .linear(&vec![0x90; MAX_FUNCTION], 0x1000, &[])
        .is_err());
    assert_eq!(budget.used(), held);
    drop(decoder);
    assert_eq!(budget.used(), 0);
}

#[test]
fn reference_input_caps_counts_addresses_and_holds_its_reservation() {
    use gamepkg_takedawn_native_gen::reference::References;
    let budget = Budget::new(128 * 1024 * 1024);
    let good = br#"[{"start":"0x11000","end":"0x11010","ignored":[1,2,3]}]"#;
    let refs = References::read(&good[..], &budget).unwrap();
    assert_eq!(
        (refs.functions()[0].start, refs.functions()[0].end),
        (0x11000, 0x11010)
    );
    assert_eq!(budget.used(), 48 * 1024 * 1024);
    drop(refs);
    for bad in [
        "[]",
        "{}",
        r#"[{"start":"0x100000000","end":"0x11010"}]"#,
        r#"[{"start":"0x11000","end":"0x11000"}]"#,
        r#"[{"start":"1234","end":"0x11010"}]"#,
        r#"[{"start":"0xzz","end":"0x11010"}]"#,
        r#"[{"start":"0x1","start":"0x2","end":"0x3"}]"#,
    ] {
        assert!(References::read(bad.as_bytes(), &budget).is_err(), "{bad}");
        assert_eq!(budget.used(), 0);
    }
    let too_many = format!(
        "[{}]",
        vec![r#"{"start":"0x1","end":"0x2"}"#; 32769].join(",")
    );
    assert!(References::read(too_many.as_bytes(), &budget).is_err());
    assert!(References::read(std::io::repeat(b' '), &budget).is_err());
    assert!(References::read(&good[..], &Budget::new(1)).is_err());
    assert_eq!(budget.used(), 0);
}

#[test]
fn validated_kernel_imports_cover_empty_duplicate_unknown_and_unaligned_slots() {
    use gamepkg_takedawn_native_gen::xbe::KernelImport;
    for debug in [false, true] {
        let mut bytes = fixture(debug);
        assert_eq!(
            Image::parse(&bytes)
                .unwrap()
                .kernel_imports()
                .unwrap()
                .len(),
            0
        );
        for offset in [0, 1] {
            let key = if debug { 0xefb1f152 } else { 0x5b6d40b6 };
            put(&mut bytes, 0x158, (0x12000 + offset) ^ key);
            for (i, ordinal) in [255, 322, 322, 0x01234567, 0x7fffffff]
                .into_iter()
                .enumerate()
            {
                put(
                    &mut bytes,
                    0x2000 + offset as usize + i * 4,
                    0x80000000 | ordinal,
                );
            }
            put(&mut bytes, 0x2014 + offset as usize, 0);
            let image = Image::parse(&bytes).unwrap();
            let imports = image.kernel_imports().unwrap();
            assert_eq!(imports.len(), 5);
            let expected: Vec<_> = [255, 322, 322, 0x01234567, 0x7fffffff]
                .into_iter()
                .enumerate()
                .map(|(i, ordinal)| KernelImport {
                    slot: 0x12000 + offset + i as u32 * 4,
                    ordinal,
                })
                .collect();
            assert_eq!(imports.clone().collect::<Vec<_>>(), expected);
            assert_eq!(imports.collect::<Vec<_>>(), expected);
        }
    }
}
#[test]
fn malformed_import_tables_fail_before_exposing_slots_or_binding() {
    use gamepkg_takedawn_native_gen::kernel_data::bind_hardware_info;
    for invalid in [1, 322, 0x80000000] {
        let mut bytes = fixture(false);
        put(&mut bytes, 0x2000, 0x80000142);
        put(&mut bytes, 0x2004, invalid);
        let image = Image::parse(&bytes).unwrap();
        assert!(image.kernel_imports().is_err());
        let mut ram = bytes.clone();
        let before = ram.clone();
        assert!(bind_hardware_info(&image, 0x10000, &mut ram, 0x80010000).is_err());
        assert_eq!(ram, before);
    }
    let mut bytes = fixture(false);
    for offset in (0x2000..0x2100).step_by(4) {
        put(&mut bytes, offset, 0x80000142);
    }
    let image = Image::parse(&bytes).unwrap();
    assert!(image.kernel_imports().is_err()); // BSS zeros are not a terminator.
}
#[test]
fn hardware_data_binding_is_transactional_and_never_changes_other_imports() {
    use gamepkg_takedawn_native_gen::kernel_data::{
        bind_hardware_info, HardwareInfo, DEVKIT_KERNEL_FLAG,
    };
    assert_eq!(DEVKIT_KERNEL_FLAG, 2);
    assert_eq!(
        HardwareInfo {
            flags: 0x11223344,
            gpu_revision: 0xa1,
            mcp_revision: 0xb1
        }
        .to_le_bytes(),
        [0x44, 0x33, 0x22, 0x11, 0xa1, 0xb1, 0, 0]
    );
    let mut bytes = fixture(false);
    for (i, ordinal) in [255, 322, 322, 156].into_iter().enumerate() {
        put(&mut bytes, 0x2000 + i * 4, 0x80000000 | ordinal);
    }
    let image = Image::parse(&bytes).unwrap();
    let original = bytes.clone();
    for address in [0, 0x80000142, 0xffff_fffc, 0x11000, 0x13fff] {
        let mut ram = original.clone();
        assert!(bind_hardware_info(&image, 0x10000, &mut ram, address).is_err());
        assert_eq!(ram, original);
    }
    for mode in 0..3 {
        let mut ram = original.clone();
        if mode == 0 {
            put(&mut ram, 0x2008, 0);
        } // A later bad slot cannot patch the earlier one.
        if mode == 1 {
            ram.truncate(0x200b);
        }
        let before = ram.clone();
        assert!(bind_hardware_info(
            &image,
            if mode == 2 { 0x12005 } else { 0x10000 },
            &mut ram,
            0x80010000
        )
        .is_err());
        assert_eq!(ram, before);
    }
    let mut ram = original.clone();
    put(&mut ram, 0x2000, 0x12340000); // An unrelated function may already be resolved.
    let mut expected = ram.clone();
    put(&mut expected, 0x2004, 0x80010000);
    put(&mut expected, 0x2008, 0x80010000);
    assert_eq!(
        bind_hardware_info(&image, 0x10000, &mut ram, 0x80010000).unwrap(),
        2
    );
    assert_eq!(ram, expected);
    assert!(bind_hardware_info(&image, 0x10000, &mut ram, 0x80010000).is_err());
    assert_eq!(ram, expected);
    assert_eq!(bytes, original);
}

#[test]
fn import_words_can_cross_adjacent_raw_sections_but_never_a_bss_gap() {
    use gamepkg_takedawn_native_gen::kernel_data::bind_hardware_info;
    let mut bytes = fixture(false);
    put(&mut bytes, 0x210, 0x100); // Fully back first section through 0x110ff.
    put(&mut bytes, 0x23c, 0x11100); // Adjacent virtual data, discontiguous raw bytes.
    put(&mut bytes, 0x158, 0x110ff ^ 0x5b6d40b6);
    bytes[0x10ff] = 0x42;
    bytes[0x2000..0x2003].copy_from_slice(&[1, 0, 0x80]);
    let image = Image::parse(&bytes).unwrap();
    let imports: Vec<_> = image.kernel_imports().unwrap().collect();
    assert_eq!(imports.len(), 1);
    assert_eq!((imports[0].slot, imports[0].ordinal), (0x110ff, 322));
    let mut ram = vec![0u8; 0x4000];
    for section in &image.sections {
        let offset = (section.virtual_address - image.base) as usize;
        ram[offset..offset + section.raw_size as usize].copy_from_slice(
            &bytes[section.raw_address as usize..(section.raw_address + section.raw_size) as usize],
        );
    }
    assert_eq!(
        bind_hardware_info(&image, 0x10000, &mut ram, 0x80010000).unwrap(),
        1
    );
    assert_eq!(&ram[0x10ff..0x1103], &0x80010000u32.to_le_bytes());
    put(&mut bytes, 0x210, 0xff); // Same virtual range; last code byte is now BSS.
    assert!(Image::parse(&bytes).is_err());
}

#[test]
fn combined_static_data_binding_validates_both_exports_before_any_write() {
    use gamepkg_takedawn_native_gen::kernel_data::{
        bind_platform_data, KernelDataAddresses, KernelVersion,
    };
    assert_eq!(
        KernelVersion {
            major: 0x1122,
            minor: 0x3344,
            build: 0x5566,
            qfe: 0x7788
        }
        .to_le_bytes(),
        [0x22, 0x11, 0x44, 0x33, 0x66, 0x55, 0x88, 0x77]
    );
    assert_eq!(
        KernelVersion {
            major: 1,
            minor: 0,
            build: 5838,
            qfe: 1
        }
        .to_le_bytes(),
        [1, 0, 0, 0, 0xce, 0x16, 1, 0]
    );
    let mut bytes = fixture(false);
    for (i, ordinal) in [322, 255, 324, 322].into_iter().enumerate() {
        put(&mut bytes, 0x2000 + i * 4, 0x80000000 | ordinal);
    }
    let image = Image::parse(&bytes).unwrap();
    let targets = KernelDataAddresses {
        hardware_info: 0x80010000,
        kernel_version: 0x80010010,
    };
    let mut corrupt = bytes.clone();
    put(&mut corrupt, 0x2008, 0);
    let before = corrupt.clone();
    assert!(bind_platform_data(&image, 0x10000, &mut corrupt, targets).is_err());
    assert_eq!(corrupt, before);
    for address in [0x80010000, 0x80010004, 0x80000144, 0x11000, 0xffff_fffc, 0] {
        let mut ram = bytes.clone();
        assert!(bind_platform_data(
            &image,
            0x10000,
            &mut ram,
            KernelDataAddresses {
                kernel_version: address,
                ..targets
            }
        )
        .is_err());
        assert_eq!(ram, bytes);
    }
    let mut ram = bytes.clone();
    let mut expected = bytes.clone();
    put(&mut expected, 0x2000, targets.hardware_info);
    put(&mut expected, 0x2008, targets.kernel_version);
    put(&mut expected, 0x200c, targets.hardware_info);
    assert_eq!(
        bind_platform_data(&image, 0x10000, &mut ram, targets).unwrap(),
        3
    );
    assert_eq!(ram, expected);
}
