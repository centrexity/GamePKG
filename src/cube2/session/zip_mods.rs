//! Exercise the unchanged upstream virtual ZIP filesystem inside the real sandbox.
use super::*;

fn name(session: &mut Session) -> String {
    let saved = session.snapshot().unwrap();
    let files = protocol::save_files(&saved).unwrap();
    let config = files.iter().find(|file| file.path == "config.cfg").unwrap();
    std::str::from_utf8(config.bytes)
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("name \"")
                .and_then(|s| s.strip_suffix('"'))
        })
        .unwrap()
        .to_owned()
}
fn tap(session: &mut Session, key: u32) {
    session.key(key, true, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
    session.key(key, false, false, 0).unwrap();
    session.tick(0, 0, 0).unwrap();
}
fn assert_zip_image(session: &mut Session) {
    fn colors(frame: &protocol::Frame) -> [usize; 2] {
        let mut counts = [0; 2];
        for pixel in frame.rgba.chunks_exact(4) {
            if pixel[0] > 180 && pixel[1] < 60 && pixel[2] > 180 {
                counts[0] += 1;
            }
            if pixel[0] < 60 && pixel[1] > 180 && pixel[2] > 180 {
                counts[1] += 1;
            }
        }
        counts
    }
    tap(session, 69); // F12 closes the original GUI.
    let hidden = colors(&session.tick(0, 0, 0).unwrap());
    tap(session, 68); // F11 opens the original GUI containing the ZIP-only PNG.
    for _ in 0..5 {
        session.tick(0, 0, 0).unwrap();
    }
    let shown = colors(&session.tick(0, 0, 0).unwrap());
    for channel in 0..2 {
        assert!(shown[channel] > hidden[channel] + 500, "ZIP PNG color {channel} absent from actual rendered GUI: hidden={hidden:?}, shown={shown:?}");
    }
    tap(session, 69);
}
#[test]
#[ignore = "requires original Cube 2 data and compiler/runtime namespaces"]
fn original_zip_mount_script_unmount_and_autoexec_restart() {
    let original = std::env::var_os("GAMEPKG_CUBE2_DATA").expect("select original data");
    let fixture = tempfile::tempdir().unwrap();
    // Real/reflink copies never add hardlinks to or mutate the selected original media.
    for part in ["data", "packages"] {
        assert!(std::process::Command::new("cp")
            .args(["--reflink=auto", "-R", "--"])
            .arg(Path::new(&original).join(part))
            .arg(fixture.path().join(part))
            .status()
            .unwrap()
            .success());
    }
    let mods = fixture.path().join("packages/mods");
    fs::create_dir_all(&mods).unwrap();
    let binary = super::super::build::compile().unwrap();
    for (label, method) in [
        ("Stored", zip::CompressionMethod::Stored),
        ("Deflated", zip::CompressionMethod::Deflated),
    ] {
        let archive = mods.join("qualification.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .start_file(
                "bundle/packages/qualification/start.cfg",
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer
            .write_all(format!("name Zip{label}\nnewgui ZIPAsset [guiimage packages/qualification/check.png \"\" 4 0]\n").as_bytes())
            .unwrap();
        let pixels = image::RgbaImage::from_fn(32, 32, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                image::Rgba([255, 0, 255, 255])
            } else {
                image::Rgba([0, 255, 255, 255])
            }
        });
        let mut png = std::io::Cursor::new(Vec::new());
        pixels.write_to(&mut png, image::ImageFormat::Png).unwrap();
        writer
            .start_file(
                "bundle/packages/qualification/check.png",
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(png.get_ref()).unwrap();
        // Preserve the original map and its complete original material configuration.
        writer
            .start_file(
                "bundle/packages/qualification/arena.ogz",
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer
            .write_all(&fs::read(Path::new(&original).join("packages/base/complex.ogz")).unwrap())
            .unwrap();
        writer
            .start_file(
                "bundle/packages/qualification/arena.cfg",
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer
            .write_all(&fs::read(Path::new(&original).join("packages/base/complex.cfg")).unwrap())
            .unwrap();
        writer
            .write_all(format!("\nname ZipMap{label}\n").as_bytes())
            .unwrap();
        writer.finish().unwrap();
        let mut overlay = zip::ZipWriter::new(fs::File::create(mods.join("overlay.zip")).unwrap());
        overlay
            .start_file(
                "packages/qualification/start.cfg",
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        overlay
            .write_all(format!("name Ovr{label}\n").as_bytes())
            .unwrap();
        overlay.set_comment(format!(
            "{}PK\u{5}\u{6}{}{}",
            "q".repeat(700),
            "\0".repeat(18),
            "q".repeat(301)
        ));
        overlay.finish().unwrap();
        let complete = fs::read(mods.join("overlay.zip")).unwrap();
        let mut bad_signature = complete.clone();
        let central = complete
            .windows(4)
            .position(|w| w == b"PK\x01\x02")
            .unwrap();
        bad_signature[central..central + 4].fill(0);
        let mut bad_size = complete.clone();
        let mut bad_offset = complete.clone();
        let end = complete.len() - 1023 - 22;
        assert_eq!(&complete[end..end + 4], b"PK\x05\x06");
        bad_size[end + 12..end + 16].copy_from_slice(&u32::MAX.to_le_bytes());
        bad_offset[end + 16..end + 20].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut bad_name = complete.clone();
        bad_name[central + 28..central + 30].copy_from_slice(&u16::MAX.to_le_bytes());
        let mut bad_extra = complete.clone();
        bad_extra[central + 10..central + 12].copy_from_slice(&99u16.to_le_bytes());
        bad_extra[central + 30..central + 32].copy_from_slice(&u16::MAX.to_le_bytes());
        let mut bad_comment = complete.clone();
        bad_comment[central + 32..central + 34].copy_from_slice(&u16::MAX.to_le_bytes());
        let mut bad_later_entry = fs::read(&archive).unwrap();
        let directory_end = bad_later_entry
            .windows(4)
            .rposition(|w| w == b"PK\x05\x06")
            .unwrap();
        let first = u32::from_le_bytes(
            bad_later_entry[directory_end + 16..directory_end + 20]
                .try_into()
                .unwrap(),
        ) as usize;
        let second = first
            + 46
            + [28, 30, 32]
                .into_iter()
                .map(|offset| {
                    u16::from_le_bytes(
                        bad_later_entry[first + offset..first + offset + 2]
                            .try_into()
                            .unwrap(),
                    ) as usize
                })
                .sum::<usize>();
        assert_eq!(&bad_later_entry[second..second + 4], b"PK\x01\x02");
        bad_later_entry[second..second + 4].fill(0);
        for (index, bytes) in [
            Vec::new(),
            complete[..end].to_vec(),
            bad_signature,
            bad_offset,
            bad_size,
            bad_name,
            bad_extra,
            bad_comment,
            bad_later_entry,
        ]
        .iter()
        .enumerate()
        {
            fs::write(mods.join(format!("bad{index}.zip")), bytes).unwrap();
        }
        // These directories mount successfully; opening their local payload must
        // fail before executing the overriding script or reading beyond the file.
        let mut local_bad = Vec::new();
        for (offset, bytes) in [
            (26, u16::MAX.to_le_bytes().to_vec()),
            (28, u16::MAX.to_le_bytes().to_vec()),
            (6, 1u16.to_le_bytes().to_vec()),
            (8, 99u16.to_le_bytes().to_vec()),
            (central + 24, u32::MAX.to_le_bytes().to_vec()),
        ] {
            let mut bad = complete.clone();
            bad[offset..offset + bytes.len()].copy_from_slice(&bytes);
            local_bad.push(bad);
        }
        // The final case must exceed the payload extent for compressed files too.
        local_bad[4][central + 20..central + 24].copy_from_slice(&u32::MAX.to_le_bytes());
        for (index, bytes) in local_bad.iter().enumerate() {
            fs::write(mods.join(format!("localbad{index}.zip")), bytes).unwrap();
        }
        let startup = b"addzip packages/mods/qualification.zip packages/ bundle/packages/; exec packages/qualification/start.cfg";
        let config = b"exec data/defaults.cfg\nname BeforeZip\nbind F3 [loop i 9 [addzip (format packages/mods/bad%1.zip $i)]; addzip packages/mods/missing.zip; exec packages/qualification/start.cfg]\nbind F4 [addzip packages/mods/qualification.zip packages/ bundle/packages/; exec packages/qualification/start.cfg]\nbind F5 [addzip packages/mods/overlay.zip; exec packages/qualification/start.cfg]\nbind F6 [removezip packages/mods/overlay.zip; exec packages/qualification/start.cfg]\nbind F8 [map qualification/arena]\nbind F7 [name (format %1_%2_%3 (div (at (getcampos) 0) 1) (div (at (getcampos) 1) 1) (div (at (getcampos) 2) 1))]\nbind F9 [removezip packages/mods/qualification.zip; name Unmounted; exec packages/qualification/start.cfg]\nbind F10 [addzip packages/mods/qualification.zip packages/ bundle/packages/; exec packages/qualification/start.cfg]\nbind F11 [showgui ZIPAsset]\nbind F12 [showgui \"\"]\n";
        let config = [config.as_slice(), b"badlocalindex = 0\nbind F2 [addzip (format packages/mods/localbad%1.zip $badlocalindex); exec packages/qualification/start.cfg; removezip (format packages/mods/localbad%1.zip $badlocalindex); badlocalindex = (+ $badlocalindex 1)]\n"].concat();
        let seed = protocol::test_tree(&[("autoexec.cfg", startup), ("config.cfg", &config)]);
        let mut session = Session::start(
            fixture.path(),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &seed,
            || Ok(binary.clone()),
            None,
            "; mode 0; map complex".into(),
        )
        .unwrap();
        session.tick(0, 0, 0).unwrap();
        session.set_deadline(Duration::from_millis(500)).unwrap();
        for _ in 0..20 {
            session.tick(0, 0, 0).unwrap();
        }
        assert_eq!(name(&mut session), format!("Zip{label}"));
        assert_zip_image(&mut session);
        for index in 0..local_bad.len() {
            tap(&mut session, 59); // F2 tries exactly one malformed local payload.
            assert_eq!(
                name(&mut session),
                format!("Zip{label}"),
                "Malformed local header {index} executed the overriding script"
            );
        }
        tap(&mut session, 60); // F3 attempts malformed and unavailable archives.
        assert_eq!(name(&mut session), format!("Zip{label}"));
        let diagnostics_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let diagnostics =
                String::from_utf8_lossy(&session.diagnostics.lock().unwrap()).into_owned();
            let failures_reported = (0..9).all(|i| {
                diagnostics.contains(&format!(
                    "could not read directory in zip packages/mods/bad{i}.zip"
                ))
            }) && diagnostics
                .contains("could not open file packages/mods/missing.zip");
            if failures_reported {
                break;
            }
            assert!(
                Instant::now() < diagnostics_deadline,
                "Original ZIP rejection diagnostics missing: {diagnostics}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        for _ in 0..3 {
            tap(&mut session, 62); // Last-mounted ZIP overrides the same virtual path.
            assert_eq!(name(&mut session), format!("Ovr{label}"));
            tap(&mut session, 61); // Re-adding an already-mounted ZIP must not reorder it.
            assert_eq!(name(&mut session), format!("Ovr{label}"));
            tap(&mut session, 63); // Removing the overlay reveals the prior archive.
            assert_eq!(name(&mut session), format!("Zip{label}"));
            tap(&mut session, 63); // A redundant removal must leave other mounts intact.
            assert_eq!(name(&mut session), format!("Zip{label}"));
        }
        tap(&mut session, 66);
        assert_eq!(
            name(&mut session),
            "Unmounted",
            "Unmounted ZIP script remained accessible"
        );
        tap(&mut session, 67);
        assert_eq!(name(&mut session), format!("Zip{label}"));
        tap(&mut session, 66);
        let saved = session.snapshot().unwrap();
        assert_eq!(name(&mut session), "Unmounted");
        session.close().unwrap();
        let mut restored = Session::start(
            fixture.path(),
            2 * 1024 * 1048576,
            Duration::from_secs(60),
            &saved,
            || Ok(binary.clone()),
            None,
            "; mode 0; map complex".into(),
        )
        .unwrap();
        restored.tick(0, 0, 0).unwrap();
        restored.set_deadline(Duration::from_millis(500)).unwrap();
        for _ in 0..10 {
            restored.tick(0, 0, 0).unwrap();
        }
        assert_eq!(name(&mut restored), format!("Zip{label}"));
        assert_zip_image(&mut restored);
        let saved = restored.snapshot().unwrap();
        assert_eq!(
            protocol::save_files(&saved)
                .unwrap()
                .iter()
                .find(|f| f.path == "autoexec.cfg")
                .unwrap()
                .bytes,
            startup
        );
        tap(&mut restored, 65); // F8 loads the map that exists only inside this ZIP.
        for _ in 0..20 {
            restored.tick(0, 0, 0).unwrap();
        }
        assert_eq!(
            name(&mut restored),
            format!("ZipMap{label}"),
            "Original ZIP map/config load failed"
        );
        tap(&mut restored, 64); // Original camera position, rounded for saved-name telemetry.
        let before = name(&mut restored);
        restored.key(26, true, false, 0).unwrap(); // Physical W uses the original movement binding.
        for _ in 0..30 {
            restored.tick(0, 0, 0).unwrap();
        }
        restored.key(26, false, false, 0).unwrap();
        restored.tick(0, 0, 0).unwrap();
        tap(&mut restored, 64);
        assert_ne!(
            name(&mut restored),
            before,
            "Player did not move in the ZIP-loaded map"
        );
        restored.close().unwrap();
        assert!(
            !fixture.path().join("packages/qualification").exists(),
            "Virtual ZIP contents were unexpectedly extracted"
        );
    }
}
