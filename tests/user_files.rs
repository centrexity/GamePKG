use gamepkg::{
    manifest::UserFile,
    user_files::{self, Kind, Outcome, Service},
};
fn policy() -> UserFile {
    UserFile {
        id: "patterns".into(),
        title: "Level pattern".into(),
        extensions: vec!["txt".into()],
        max_bytes: 1024,
        read: true,
        write: true,
    }
}
fn enabled() -> Service {
    let mut service = Service::new(vec![policy()]);
    service.enable();
    service
}
fn start(service: &mut Service) -> u32 {
    service.begin(true);
    u32::from_le_bytes(
        service
            .call(0, &0u32.to_le_bytes(), 4)
            .unwrap()
            .try_into()
            .unwrap(),
    )
}
fn status(service: &mut Service, id: u32) -> u32 {
    u32::from_le_bytes(
        service.call(2, &id.to_le_bytes(), 12).unwrap()[..4]
            .try_into()
            .unwrap(),
    )
}
fn exporting(service: &mut Service, name: &str, bytes: &[u8]) -> Option<u32> {
    let mut request = 0u32.to_le_bytes().to_vec();
    request.extend((name.len() as u32).to_le_bytes());
    request.extend(name.as_bytes());
    request.extend(bytes);
    service
        .call(1, &request, 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}
#[test]
fn requests_require_signed_permissions_host_enablement_and_a_fresh_gesture() {
    let mut service = Service::new(vec![policy()]);
    service.begin(true);
    assert!(service.call(0, &[0; 4], 4).is_none());
    service.enable();
    service.begin(false);
    assert!(service.call(0, &[0; 4], 4).is_none());
    service.begin(true);
    for (op, request, cap) in [
        (0, vec![0; 3], 4),
        (0, vec![1, 0, 0, 0], 4),
        (0, vec![0; 4], 3),
        (6, vec![0; 4], 4),
        (0, vec![0; 4], 65537),
    ] {
        assert!(service.call(op, &request, cap).is_none());
    }
    let handle = start(&mut service);
    assert_eq!(status(&mut service, handle), 1);
    assert!(service.take_job().is_none());
    assert!(service.call(5, &handle.to_le_bytes(), 0).is_none());
    service.finish(false);
    assert!(service.take_job().is_none());
    let next = start(&mut service);
    assert_ne!(handle, next);
    service.finish(true);
    let job = service.take_job().unwrap();
    assert!(service.take_job().is_none());
    assert!(service.complete(&job, Outcome::Cancelled));
    assert_eq!(status(&mut service, next), 4);
    assert!(!service.complete(&job, Outcome::Exported));
    assert_eq!(service.call(5, &next.to_le_bytes(), 0), Some(vec![]));
    assert!(
        service.call(0, &[0; 4], 4).is_none(),
        "an earlier gesture was reused"
    );
    let mut denied = Service::new(vec![]);
    denied.enable();
    denied.begin(true);
    assert!(denied.call(0, &[0; 4], 4).is_none());
}
#[test]
fn imported_bytes_names_and_partial_reads_are_bounded_and_nonconsuming() {
    let mut service = enabled();
    let handle = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    assert!(service.complete(
        &job,
        Outcome::Imported {
            name: "café.TXT".into(),
            bytes: b"0123456789".to_vec()
        }
    ));
    assert_eq!(status(&mut service, handle), 2);
    assert!(service.call(3, &handle.to_le_bytes(), 2).is_none());
    assert_eq!(
        service.call(3, &handle.to_le_bytes(), 256).unwrap(),
        "café.TXT".as_bytes()
    );
    let mut request = handle.to_le_bytes().to_vec();
    request.extend(4u32.to_le_bytes());
    assert_eq!(service.call(4, &request, 3).unwrap(), b"456");
    assert_eq!(service.call(4, &request, 99).unwrap(), b"456789");
    request[4..].copy_from_slice(&11u32.to_le_bytes());
    assert!(service.call(4, &request, 3).is_none());
    assert_eq!(status(&mut service, handle), 2);
    assert!(service.call(2, &handle.to_le_bytes(), 11).is_none());
    assert_eq!(status(&mut service, handle), 2);
}
#[test]
fn delayed_replies_cannot_cross_games_handles_or_closed_sessions() {
    let mut first = enabled();
    let handle = start(&mut first);
    first.finish(true);
    let old = first.take_job().unwrap();
    let mut second = enabled();
    assert_eq!(start(&mut second), handle);
    second.finish(true);
    let new = second.take_job().unwrap();
    assert!(!second.complete(&old, Outcome::Cancelled));
    assert!(second.complete(&new, Outcome::Cancelled));
    drop(first);
    assert!(!old.active());
    assert!(matches!(
        user_files::execute(&old, None),
        Outcome::Cancelled
    ));
    second.call(5, &handle.to_le_bytes(), 0).unwrap();
    let _ = start(&mut second);
    second.finish(true);
    let current = second.take_job().unwrap();
    assert!(!second.complete(&new, Outcome::Cancelled));
    assert!(second.complete(&current, Outcome::Cancelled));
}
#[test]
fn portable_names_reject_traversal_devices_streams_controls_and_ambiguous_suffixes() {
    for name in [
        "../x.txt",
        "a/b.txt",
        r"a\b.txt",
        r"C:\x.txt",
        "/tmp/x.txt",
        "x.txt:stream",
        "CON.txt",
        "con .txt",
        "CONIN$.txt",
        "LPT¹.txt",
        "COM9.txt",
        "NUL.txt",
        "x.txt.",
        "x.txt ",
        "x\0.txt",
        "x\u{85}.txt",
        "x?.txt",
        "..",
        ".",
    ] {
        assert!(!user_files::portable_name(name), "accepted {name:?}");
    }
    for name in [
        "normal.txt",
        ".hidden.txt",
        "café😀.TXT",
        "safe%2Fname.txt",
        "console.txt",
        "COM10.txt",
    ] {
        assert!(user_files::portable_name(name), "rejected {name}");
    }
    let mut service = enabled();
    service.begin(true);
    for name in ["../outside.txt", "a.exe", "x.txt:stream"] {
        assert!(exporting(&mut service, name, b"data").is_none());
    }
    assert!(exporting(&mut service, "valid.txt", &[0; 1025]).is_none());
    assert!(exporting(&mut service, "valid.txt", b"data").is_some());
}
#[test]
fn only_selected_regular_files_are_read_and_exports_replace_only_the_chosen_file() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input.txt");
    std::fs::write(&input, b"original ASCII").unwrap();
    let untouched = root.path().join("untouched.txt");
    std::fs::write(&untouched, b"keep").unwrap();
    let mut service = enabled();
    let handle = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    let outcome = user_files::execute(&job, Some(&input));
    assert!(
        matches!(&outcome,Outcome::Imported{name,bytes} if name=="input.txt" && bytes==b"original ASCII")
    );
    service.complete(&job, outcome);
    assert_eq!(status(&mut service, handle), 2);
    assert_eq!(std::fs::read(&input).unwrap(), b"original ASCII");
    service.call(5, &handle.to_le_bytes(), 0).unwrap();
    service.begin(true);
    let id = exporting(&mut service, "suggested.txt", b"exported").unwrap();
    service.finish(true);
    let job = service.take_job().unwrap();
    assert!(matches!(job.kind, Kind::Export { .. }));
    assert!(matches!(
        user_files::execute(&job, Some(&input)),
        Outcome::Exported
    ));
    assert_eq!(std::fs::read(&input).unwrap(), b"exported");
    assert_eq!(std::fs::read(&untouched).unwrap(), b"keep");
    service.complete(&job, Outcome::Exported);
    assert_eq!(status(&mut service, id), 3);
    assert!(
        !root.path().join("suggested.txt").exists(),
        "guest's suggestion became an automatic destination"
    );
}
#[cfg(unix)]
#[test]
fn symlinks_directories_and_fifos_never_become_import_or_export_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target.txt");
    std::fs::write(&target, b"keep").unwrap();
    let link = root.path().join("link.txt");
    symlink(&target, &link).unwrap();
    let folder = root.path().join("folder.txt");
    std::fs::create_dir(&folder).unwrap();
    let fifo = root.path().join("pipe.txt");
    let c = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let mut service = enabled();
    let handle = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    for path in [&link, &folder, &fifo] {
        assert!(matches!(
            user_files::execute(&job, Some(path)),
            Outcome::Failed
        ));
    }
    service.complete(&job, Outcome::Cancelled);
    service.call(5, &handle.to_le_bytes(), 0).unwrap();
    service.begin(true);
    exporting(&mut service, "x.txt", b"replace").unwrap();
    service.finish(true);
    let job = service.take_job().unwrap();
    for path in [&link, &folder, &fifo] {
        assert!(matches!(
            user_files::execute(&job, Some(path)),
            Outcome::Failed
        ));
    }
    assert_eq!(std::fs::read(target).unwrap(), b"keep");
}
#[test]
fn malformed_declarations_and_completed_file_payloads_fail_closed() {
    let mut p = policy();
    p.extensions = vec!["../txt".into()];
    assert!(user_files::validate(&[p.clone()]).is_err());
    let mut service = Service::new(vec![p]);
    service.enable();
    service.begin(true);
    assert!(service.call(0, &[0; 4], 4).is_none());
    let mut service = enabled();
    let id = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    service.complete(
        &job,
        Outcome::Imported {
            name: "not.exe".into(),
            bytes: vec![1],
        },
    );
    assert_eq!(status(&mut service, id), 5);
    service.call(5, &id.to_le_bytes(), 0).unwrap();
    let id = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    service.complete(
        &job,
        Outcome::Imported {
            name: "large.txt".into(),
            bytes: vec![0; 1025],
        },
    );
    assert_eq!(status(&mut service, id), 5);
}

#[test]
fn signed_file_declarations_are_strict_and_legacy_packages_remain_default_denied() {
    use gamepkg::manifest::Manifest;
    let old = Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    assert!(old.permissions.files.is_empty());
    assert!(!toml::to_string(&old).unwrap().contains("permissions.files"));
    let declaration="\n[[permissions.files]]\nid='patterns'\ntitle='Pattern text'\nextensions=['txt']\nmax_bytes=1024\nread=true\nwrite=true\n";
    let text = format!(
        "{}{}",
        include_str!("../examples/minimal/manifest.toml"),
        declaration
    );
    assert!(Manifest::parse(text.as_bytes()).is_ok());
    assert!(Manifest::parse(text.replace("input = true", "input = false").as_bytes()).is_err());
    assert!(Manifest::parse(format!("{text}directory=true\n").as_bytes()).is_err());
    for change in 0..10 {
        let mut p = policy();
        match change {
            0 => p.id = "../escape".into(),
            1 => p.title = "Pattern\nSystem prompt".into(),
            2 => p.title = " ".into(),
            3 => p.extensions = vec!["*".into()],
            4 => p.extensions = vec!["txt".into(), "txt".into()],
            5 => p.max_bytes = u64::MAX,
            6 => p.max_bytes = 0,
            7 => {
                p.read = false;
                p.write = false;
            }
            8 => p.max_bytes = user_files::MAX_EXPORT as u64 + 1,
            _ => p.extensions = vec!["txt".repeat(100)],
        };
        assert!(
            user_files::validate(&[p]).is_err(),
            "accepted malformed declaration {change}"
        );
    }
    let p = policy();
    assert!(user_files::validate(&[p.clone(), p]).is_err());
    let mut p = policy();
    p.write = false;
    p.max_bytes = user_files::MAX_IMPORT as u64;
    assert!(user_files::validate(&[p]).is_ok());
}

#[test]
fn host_failures_discard_undispatched_requests_but_not_a_dialog_already_open() {
    let mut service = enabled();
    start(&mut service);
    service.finish(true);
    service.finish(false);
    assert!(service.take_job().is_none());
    let handle = start(&mut service);
    service.finish(true);
    let job = service.take_job().unwrap();
    service.begin(false);
    service.finish(false);
    assert!(service.complete(&job, Outcome::Cancelled));
    assert_eq!(status(&mut service, handle), 4);
}
