use super::media::Frame;
use super::*;
use gamepkg_native_input::{Event, Gamepad};
use std::io::{Read, Seek, SeekFrom, Write};

#[derive(Default)]
pub(super) struct Recording {
    windows: Vec<WindowCommand>,
    frames: Vec<Frame>,
    audio: Vec<i16>,
    rumble: Vec<u64>,
    busy: bool,
}
impl Backend for Recording {
    fn window(&mut self, c: WindowCommand) -> Result<()> {
        self.windows.push(c);
        Ok(())
    }
    fn present(&mut self, f: Frame) -> Result<()> {
        if self.busy {
            Err(Error::Busy)
        } else {
            self.frames.push(f);
            Ok(())
        }
    }
    fn audio(&mut self, s: &[i16], _: u32) -> Result<()> {
        if self.busy {
            Err(Error::Busy)
        } else {
            self.audio.extend(s);
            Ok(())
        }
    }
    fn rumble(&mut self, d: u64, _: u16, _: u16, _: u32) -> Result<()> {
        self.rumble.push(d);
        Ok(())
    }
}
fn permissions() -> Permissions {
    toml::from_str::<crate::manifest::Manifest>(include_str!(
        "../../examples/minimal/manifest.toml"
    ))
    .unwrap()
    .permissions
}
pub(super) fn host() -> Session<Recording> {
    let mut p = permissions();
    p.input = true;
    p.audio = true;
    p.save = true;
    Session::new(p, Recording::default())
}
fn number(bytes: Vec<u8>) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}
#[test]
fn launch_settings_are_read_only_and_report_unsupported_without_a_launch() {
    let mut host = host();
    assert_eq!(host.call(67, &[], 8), Err(Error::Unsupported));
    let snapshot = [0, 1, 3, 2, 1, 10, 1, 255];
    host.launch_options = Some(snapshot);
    assert_eq!(host.call(67, &[], 8), Ok(snapshot.to_vec()));
    assert_eq!(host.call(67, &[], 7), Err(Error::Invalid));
    assert_eq!(host.call(67, &[0], 8), Err(Error::Invalid));
    assert_eq!(host.call(67, &[], 8), Ok(snapshot.to_vec()));
}

#[test]
fn previous_save_grant_is_read_only_and_requires_save_permission() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy-save.blob");
    std::fs::write(&path, b"previous bytes").unwrap();
    let mut denied = Session::new(permissions(), Recording::default());
    assert_eq!(
        denied.grant_directory(
            cap_std::fs::Dir::open_ambient_dir(directory.path(), cap_std::ambient_authority())
                .unwrap(),
            Access::PreviousSave,
        ),
        Err(Error::Denied)
    );
    let mut host = host();
    let prior = grant(&mut host, directory.path(), Access::PreviousSave);
    let grants = host.call(13, &[], 16).unwrap();
    assert_eq!(&grants[..8], &prior.to_le_bytes());
    assert_eq!(&grants[8..12], &5u32.to_le_bytes());
    let open = |mode: u32| {
        [
            prior.to_le_bytes().as_slice(),
            &mode.to_le_bytes(),
            b"legacy-save.blob",
        ]
        .concat()
    };
    for mode in 1..=3 {
        assert_eq!(host.call(1, &open(mode), 8), Err(Error::Denied));
    }
    let handle = number(host.call(1, &open(0), 8).unwrap());
    let range = [
        handle.to_le_bytes().as_slice(),
        &0u64.to_le_bytes(),
        &14u32.to_le_bytes(),
    ]
    .concat();
    let modify = [handle.to_le_bytes().as_slice(), &0u64.to_le_bytes(), b"new"].concat();
    assert_eq!(host.call(2, &range, 14).unwrap(), b"previous bytes");
    assert_eq!(host.call(3, &modify, 0), Err(Error::Denied));
    host.revoke(prior).unwrap();
    assert_eq!(host.call(2, &range, 14), Err(Error::Denied));
    assert_eq!(std::fs::read(path).unwrap(), b"previous bytes");
}

#[test]
fn save_replace_is_capability_relative_flushed_atomic_and_nontruncating() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = host();
    let save = grant(&mut h, dir.path(), Access::Save);
    let assets = grant(&mut h, dir.path(), Access::Assets);
    let replace = |h: &mut Session<Recording>, root: u64, from: &str, to: &str, flags: u32| {
        h.call(
            14,
            &[
                root.to_le_bytes().as_slice(),
                &flags.to_le_bytes(),
                &(from.len() as u32).to_le_bytes(),
                from.as_bytes(),
                to.as_bytes(),
            ]
            .concat(),
            0,
        )
    };
    std::fs::write(dir.path().join("temp"), b"complete-new-save").unwrap();
    std::fs::write(dir.path().join("save"), b"old").unwrap();
    assert_eq!(
        replace(&mut h, assets, "temp", "save", 0),
        Err(Error::Denied)
    );
    for path in [
        "../save",
        "/save",
        "a/../save",
        "C:save",
        "a\\save",
        "NUL",
        "save.",
        "save ",
    ] {
        assert_eq!(replace(&mut h, save, "temp", path, 0), Err(Error::Invalid));
        assert_eq!(replace(&mut h, save, path, "save", 0), Err(Error::Invalid));
    }
    assert_eq!(
        replace(&mut h, save, "temp", "temp", 0),
        Err(Error::Invalid)
    );
    assert_eq!(
        replace(&mut h, save, "temp", "save", 2),
        Err(Error::Invalid)
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("victim"), b"private").unwrap();
        symlink(outside.path(), dir.path().join("escape")).unwrap();
        symlink(outside.path().join("victim"), dir.path().join("link")).unwrap();
        assert!(replace(&mut h, save, "temp", "escape/victim", 0).is_err());
        assert_eq!(replace(&mut h, save, "temp", "link", 0), Err(Error::Denied));
        assert_eq!(replace(&mut h, save, "link", "save", 0), Err(Error::Denied));
        std::fs::hard_link(outside.path().join("victim"), dir.path().join("hard")).unwrap();
        assert_eq!(replace(&mut h, save, "temp", "hard", 0), Err(Error::Denied));
        assert_eq!(replace(&mut h, save, "hard", "save", 0), Err(Error::Denied));
        assert_eq!(
            std::fs::read(outside.path().join("victim")).unwrap(),
            b"private"
        );
    }
    assert_eq!(std::fs::read(dir.path().join("save")).unwrap(), b"old");
    #[cfg(any(unix, windows))]
    let durable = 1;
    #[cfg(not(any(unix, windows)))]
    let durable = 0;
    replace(&mut h, save, "temp", "save", durable).unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("save")).unwrap(),
        b"complete-new-save"
    );
    assert!(!dir.path().join("temp").exists());
    h.revoke(save).unwrap();
    assert_eq!(replace(&mut h, save, "temp", "save", 0), Err(Error::Denied));
}
fn open(h: &mut Session<Recording>, root: u64, mode: u32, path: &str) -> Result<Vec<u8>> {
    h.call(
        1,
        &[
            root.to_le_bytes().as_slice(),
            &mode.to_le_bytes(),
            path.as_bytes(),
        ]
        .concat(),
        8,
    )
}
fn grant(h: &mut Session<Recording>, path: &std::path::Path, access: Access) -> u64 {
    h.grant_directory(
        cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority()).unwrap(),
        access,
    )
    .unwrap()
}

#[test]
fn file_grants_are_scoped_read_only_revocable_and_do_not_reuse_handles() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("asset"), b"hello").unwrap();
    let mut h = host();
    let assets = grant(&mut h, root.path(), Access::Assets);
    assert_eq!(open(&mut h, assets, 1, "asset"), Err(Error::Denied));
    let file = number(open(&mut h, assets, 0, "asset").unwrap());
    let request = [
        file.to_le_bytes().as_slice(),
        &1u64.to_le_bytes(),
        &4u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(h.call(2, &request, 4).unwrap(), b"ello");
    assert_eq!(h.call(2, &request, 3), Err(Error::Invalid));
    assert_eq!(
        h.call(
            3,
            &[file.to_le_bytes().as_slice(), &0u64.to_le_bytes(), b"evil"].concat(),
            0
        ),
        Err(Error::Denied)
    );
    assert_eq!(host().call(5, &file.to_le_bytes(), 8), Err(Error::Denied));
    h.revoke(assets).unwrap();
    assert_eq!(h.call(5, &file.to_le_bytes(), 8), Err(Error::Denied));
    let saves = grant(&mut h, root.path(), Access::Save);
    let next = number(open(&mut h, saves, 1, "save").unwrap());
    assert_ne!(next, file);
    h.call(
        3,
        &[next.to_le_bytes().as_slice(), &0u64.to_le_bytes(), b"saved"].concat(),
        0,
    )
    .unwrap();
    assert_eq!(std::fs::read(root.path().join("save")).unwrap(), b"saved");
    assert_eq!(
        h.call(
            3,
            &[next.to_le_bytes().as_slice(), &u64::MAX.to_le_bytes(), b"x"].concat(),
            0
        ),
        Err(Error::Invalid)
    );
    h.call(4, &next.to_le_bytes(), 0).unwrap();
    assert_eq!(h.call(5, &next.to_le_bytes(), 8), Err(Error::Denied));
}

#[cfg(unix)]
#[test]
fn user_game_data_is_pinned_read_only_and_distinct_from_package_assets() {
    use std::fs;
    use std::os::unix::fs::symlink;

    let selected = tempfile::tempdir().unwrap();
    let original = selected.path().join("packages");
    fs::create_dir_all(original.join("textures/maps")).unwrap();
    fs::write(original.join("textures/maps/face.png"), b"selected").unwrap();

    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("private"), b"outside").unwrap();
    symlink(outside.path(), original.join("outside")).unwrap();
    symlink("textures", original.join("within")).unwrap();
    symlink("face.png", original.join("textures/maps/link.png")).unwrap();
    fs::hard_link(
        outside.path().join("private"),
        original.join("textures/maps/alias.png"),
    )
    .unwrap();

    let mut h = host();
    let data = grant(&mut h, &original, Access::GameData);
    let assets = grant(&mut h, outside.path(), Access::Assets);
    let scopes = h.call(13, &[], 32).unwrap();
    assert_eq!(&scopes[0..8], &data.to_le_bytes());
    assert_eq!(&scopes[8..12], &6u32.to_le_bytes());
    assert_eq!(&scopes[16..24], &assets.to_le_bytes());
    assert_eq!(&scopes[24..28], &1u32.to_le_bytes());

    fs::rename(&original, selected.path().join("moved")).unwrap();
    symlink(outside.path(), &original).unwrap();

    let name = "textures/maps/face.png";
    let file = number(open(&mut h, data, 0, name).unwrap());
    let read = [
        file.to_le_bytes().as_slice(),
        &0u64.to_le_bytes(),
        &8u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(h.call(2, &read, 8).unwrap(), b"selected");
    for mode in [1, 2, 3] {
        assert_eq!(open(&mut h, data, mode, name), Err(Error::Denied));
    }
    for name in [
        "private",
        "outside/private",
        "within/maps/face.png",
        "textures/maps/link.png",
        "textures/maps/alias.png",
        "../private",
    ] {
        assert!(open(&mut h, data, 0, name).is_err(), "{name}");
    }

    let list = |h: &mut Session<Recording>, name: &str| {
        h.call(
            6,
            &[data.to_le_bytes().as_slice(), name.as_bytes()].concat(),
            128,
        )
    };
    let maps = list(&mut h, "textures/maps").unwrap();
    assert_eq!(
        maps,
        [
            1u32.to_le_bytes().as_slice(),
            &8u32.to_le_bytes(),
            b"face.png"
        ]
        .concat()
    );
    assert!(list(&mut h, "within/maps").is_err());
    assert!(list(&mut h, "outside").is_err());

    h.revoke(data).unwrap();
    assert_eq!(h.call(2, &read, 8), Err(Error::Denied));
    assert_eq!(h.call(13, &[], 16).unwrap().len(), 16);
}

#[test]
fn file_presence_distinguishes_absence_from_invalid_denied_and_nonfile_targets() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("save"), b"complete").unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    let mut h = host();
    let assets = grant(&mut h, root.path(), Access::Assets);
    let probe = |h: &mut Session<Recording>, grant: u64, path: &str, cap: u32| {
        h.call(
            17,
            &[grant.to_le_bytes().as_slice(), path.as_bytes()].concat(),
            cap,
        )
    };
    assert_eq!(
        probe(&mut h, assets, "save", 4).unwrap(),
        1u32.to_le_bytes()
    );
    assert_eq!(
        probe(&mut h, assets, "missing", 4).unwrap(),
        0u32.to_le_bytes()
    );
    assert_eq!(
        probe(&mut h, assets, "missing/child", 4).unwrap(),
        0u32.to_le_bytes()
    );
    assert_eq!(probe(&mut h, assets, "directory", 4), Err(Error::Denied));
    for path in [
        "",
        "../save",
        "/save",
        "C:/save",
        "a\\..\\save",
        "save:stream",
    ] {
        assert_eq!(probe(&mut h, assets, path, 4), Err(Error::Invalid));
    }
    assert_eq!(probe(&mut h, assets, "save", 3), Err(Error::Invalid));
    assert_eq!(
        h.call(17, &assets.to_le_bytes()[..7], 4),
        Err(Error::Invalid)
    );
    assert_eq!(probe(&mut host(), assets, "save", 4), Err(Error::Denied));
    h.revoke(assets).unwrap();
    assert_eq!(probe(&mut h, assets, "missing", 4), Err(Error::Denied));
}

#[test]
fn directory_removal_is_empty_only_writable_scoped_and_never_recursive() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("empty")).unwrap();
    std::fs::create_dir(root.path().join("occupied")).unwrap();
    std::fs::write(root.path().join("occupied/keep"), b"whole").unwrap();
    std::fs::write(root.path().join("file"), b"whole").unwrap();
    let mut h = host();
    let assets = grant(&mut h, root.path(), Access::Assets);
    let saves = grant(&mut h, root.path(), Access::Save);
    let remove = |h: &mut Session<Recording>, grant: u64, path: &str| {
        h.call(
            18,
            &[grant.to_le_bytes().as_slice(), path.as_bytes()].concat(),
            0,
        )
    };
    assert_eq!(remove(&mut h, assets, "empty"), Err(Error::Denied));
    for path in ["", "..", "../outside", "/outside", "C:/outside", "a\\b"] {
        assert_eq!(remove(&mut h, saves, path), Err(Error::Invalid));
    }
    assert_eq!(remove(&mut h, saves, "file"), Err(Error::Denied));
    assert_eq!(remove(&mut h, saves, "occupied"), Err(Error::Failed));
    assert_eq!(
        std::fs::read(root.path().join("occupied/keep")).unwrap(),
        b"whole"
    );
    remove(&mut h, saves, "empty").unwrap();
    assert!(!root.path().join("empty").exists());
    assert_eq!(remove(&mut host(), saves, "occupied"), Err(Error::Denied));
    assert_eq!(
        h.call(18, &saves.to_le_bytes()[..7], 0),
        Err(Error::Invalid)
    );
    h.revoke(saves).unwrap();
    assert_eq!(remove(&mut h, saves, "occupied"), Err(Error::Denied));
}

#[cfg(unix)]
#[test]
fn directory_removal_rejects_final_links_and_escaping_parent_links() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(outside.path().join("keep")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).unwrap();
    let mut h = host();
    let saves = grant(&mut h, root.path(), Access::Save);
    for path in ["escape", "escape/keep"] {
        assert!(h
            .call(
                18,
                &[saves.to_le_bytes().as_slice(), path.as_bytes()].concat(),
                0
            )
            .is_err());
    }
    assert!(outside.path().join("keep").is_dir());
}

#[cfg(unix)]
#[test]
fn file_presence_never_converts_links_or_escaping_parent_failures_to_absence() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("save"), b"safe").unwrap();
    std::fs::write(outside.path().join("secret"), b"outside").unwrap();
    symlink("missing", root.path().join("dangling")).unwrap();
    symlink("save", root.path().join("link")).unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    std::fs::hard_link(root.path().join("save"), root.path().join("hardlink")).unwrap();
    let mut h = host();
    let saves = grant(&mut h, root.path(), Access::Save);
    for path in [
        "dangling",
        "link",
        "hardlink",
        "save",
        "escape/secret",
        "escape/missing",
    ] {
        let reply = h.call(
            17,
            &[saves.to_le_bytes().as_slice(), path.as_bytes()].concat(),
            4,
        );
        assert!(
            reply.is_err(),
            "{path}: not permission to conclude absence: {reply:?}"
        );
    }
}

#[test]
fn duplicated_files_preserve_rights_identity_lifetime_and_revocation() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("asset"), b"old").unwrap();
    let mut h = host();
    let assets = grant(&mut h, root.path(), Access::Assets);
    let file = number(open(&mut h, assets, 0, "asset").unwrap());
    let duplicate = number(h.call(16, &file.to_le_bytes(), 8).unwrap());
    assert_ne!(file, duplicate);
    h.call(4, &file.to_le_bytes(), 0).unwrap();
    assert_eq!(h.call(5, &file.to_le_bytes(), 8), Err(Error::Denied));
    assert_eq!(number(h.call(5, &duplicate.to_le_bytes(), 8).unwrap()), 3);
    assert_eq!(h.call(12, &duplicate.to_le_bytes(), 0), Err(Error::Denied));
    assert_eq!(
        h.call(
            9,
            &[duplicate.to_le_bytes(), 0u64.to_le_bytes()].concat(),
            0
        ),
        Err(Error::Denied)
    );
    assert_eq!(
        h.call(
            3,
            &[
                duplicate.to_le_bytes().as_slice(),
                &0u64.to_le_bytes(),
                b"evil"
            ]
            .concat(),
            0
        ),
        Err(Error::Denied)
    );
    assert_eq!(
        host().call(16, &duplicate.to_le_bytes(), 8),
        Err(Error::Denied)
    );
    assert_eq!(
        host().call(15, &duplicate.to_le_bytes(), 24),
        Err(Error::Denied)
    );
    let rom = h
        .grant_rom(std::fs::File::open(root.path().join("asset")).unwrap())
        .unwrap();
    assert_eq!(h.call(16, &rom.to_le_bytes(), 8), Err(Error::Denied));
    assert_eq!(h.call(15, &rom.to_le_bytes(), 24), Err(Error::Denied));
    h.revoke(assets).unwrap();
    assert_eq!(h.call(16, &duplicate.to_le_bytes(), 8), Err(Error::Denied));
    assert_eq!(h.call(15, &duplicate.to_le_bytes(), 24), Err(Error::Denied));
    assert_eq!(number(h.call(5, &rom.to_le_bytes(), 8).unwrap()), 3);
}

#[test]
fn duplicate_handles_are_bounded_and_do_not_double_charge_retained_inodes() {
    let root = tempfile::tempdir().unwrap();
    let mut h = host();
    let saves = h
        .grant_save_quota(
            cap_std::fs::Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap(),
            8,
        )
        .unwrap();
    let original = number(open(&mut h, saves, 1, "shadow").unwrap());
    h.call(9, &[original.to_le_bytes(), 4u64.to_le_bytes()].concat(), 0)
        .unwrap();
    let volume = |h: &mut Session<_>| h.call(23, &saves.to_le_bytes(), 16);
    assert_eq!(
        volume(&mut h).unwrap(),
        [8u64.to_le_bytes(), 4u64.to_le_bytes()].concat()
    );
    for _ in 0..256 {
        assert_eq!(h.call(16, &original.to_le_bytes(), 7), Err(Error::Invalid));
    }
    let mut duplicates = Vec::new();
    for _ in 0..255 {
        duplicates.push(number(h.call(16, &original.to_le_bytes(), 8).unwrap()));
    }
    assert_eq!(h.call(16, &original.to_le_bytes(), 8), Err(Error::Busy));
    for duplicate in duplicates.drain(2..) {
        h.call(4, &duplicate.to_le_bytes(), 0).unwrap();
    }
    let retained = duplicates[0];
    h.call(4, &original.to_le_bytes(), 0).unwrap();
    h.call(8, &[saves.to_le_bytes().as_slice(), b"shadow"].concat(), 0)
        .unwrap();
    let next = number(open(&mut h, saves, 1, "next").unwrap());
    // The unlinked old file still counts, but only once despite duplicates.
    h.call(9, &[next.to_le_bytes(), 4u64.to_le_bytes()].concat(), 0)
        .unwrap();
    assert_eq!(
        volume(&mut h).unwrap(),
        [8u64.to_le_bytes(), 0u64.to_le_bytes()].concat()
    );
    assert_eq!(
        h.call(9, &[retained.to_le_bytes(), 5u64.to_le_bytes()].concat(), 0),
        Err(Error::Busy)
    );
    h.call(12, &retained.to_le_bytes(), 0).unwrap();
    assert_eq!(number(h.call(5, &retained.to_le_bytes(), 8).unwrap()), 4);
    h.call(4, &retained.to_le_bytes(), 0).unwrap();
    h.call(4, &duplicates[1].to_le_bytes(), 0).unwrap();
    assert_eq!(
        volume(&mut h).unwrap(),
        [8u64.to_le_bytes(), 4u64.to_le_bytes()].concat()
    );
    h.call(9, &[next.to_le_bytes(), 8u64.to_le_bytes()].concat(), 0)
        .unwrap();
}

#[test]
fn save_directory_and_volume_use_the_exact_grant_and_checked_wire() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let mut h = host();
    let saves = grant(&mut h, root.path(), Access::Save);
    let assets = grant(&mut h, outside.path(), Access::Assets);
    let request = |id: u64, mode: u32, name: &str| {
        [
            id.to_le_bytes().as_slice(),
            &mode.to_le_bytes(),
            name.as_bytes(),
        ]
        .concat()
    };
    let prepare = |h: &mut Session<_>, mode, name| h.call(24, &request(saves, mode, name), 4);
    assert_eq!(prepare(&mut h, 0, "").unwrap(), 1u32.to_le_bytes());
    assert_eq!(prepare(&mut h, 0, "UDATA").unwrap(), 0u32.to_le_bytes());
    assert_eq!(prepare(&mut h, 1, "UDATA/4541005b"), Err(Error::Failed));
    assert_eq!(
        h.call(24, &request(saves, 1, "UDATA"), 3),
        Err(Error::Invalid)
    );
    assert!(!root.path().join("UDATA").exists());
    assert_eq!(prepare(&mut h, 1, "UDATA").unwrap(), 2u32.to_le_bytes());
    assert_eq!(prepare(&mut h, 1, "UDATA").unwrap(), 1u32.to_le_bytes());
    assert_eq!(
        prepare(&mut h, 1, "UDATA/4541005b").unwrap(),
        2u32.to_le_bytes()
    );
    assert_eq!(prepare(&mut h, 1, ""), Err(Error::Invalid));
    assert_eq!(prepare(&mut h, 2, "bad-mode"), Err(Error::Invalid));
    assert_eq!(prepare(&mut h, 1, "UDATA/../escape"), Err(Error::Invalid));
    assert_eq!(prepare(&mut h, 1, "UDATA/COM1"), Err(Error::Invalid));
    std::fs::write(root.path().join("file"), b"not a directory").unwrap();
    assert!(prepare(&mut h, 1, "file").is_err());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(prepare(&mut h, 0, "link").is_err());
        assert!(prepare(&mut h, 1, "link/escape").is_err());
        assert!(!outside.path().join("escape").exists());
        assert_eq!(h.call(23, &saves.to_le_bytes(), 16), Err(Error::Denied));
    }
    assert_eq!(h.call(23, &assets.to_le_bytes(), 16), Err(Error::Denied));
    assert_eq!(h.call(24, &request(assets, 0, ""), 4), Err(Error::Denied));
    assert_eq!(h.call(23, &saves.to_le_bytes(), 15), Err(Error::Invalid));
    assert_eq!(h.call(23, &[0; 9], 16), Err(Error::Invalid));
    assert_eq!(
        host().call(24, &request(saves, 0, ""), 4),
        Err(Error::Denied)
    );
    h.revoke(saves).unwrap();
    assert_eq!(h.call(23, &saves.to_le_bytes(), 16), Err(Error::Denied));
    assert_eq!(prepare(&mut h, 0, ""), Err(Error::Denied));
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn private_save_file_info_follows_the_opened_identity_and_persists_attributes() {
    let root = tempfile::tempdir().unwrap();
    let mut h = host();
    let saves = grant(&mut h, root.path(), Access::Save);
    let opened = number(open(&mut h, saves, 3, "metadata.tmp").unwrap());
    let observed = |h: &mut Session<_>, id: u64| h.call(25, &id.to_le_bytes(), 48);
    let attrs = |h: &mut Session<_>, id: u64, flags: u32| {
        h.call(
            26,
            &[id.to_le_bytes().as_slice(), &flags.to_le_bytes()].concat(),
            0,
        )
    };
    assert_eq!(h.call(25, &opened.to_le_bytes(), 47), Err(Error::Invalid));
    assert_eq!(h.call(26, &opened.to_le_bytes(), 0), Err(Error::Invalid));
    assert_eq!(attrs(&mut h, opened, 0x100), Err(Error::Invalid));
    assert_eq!(attrs(&mut h, opened, 0x01), Err(Error::Invalid));
    attrs(&mut h, opened, 0x24).unwrap();

    let initial = observed(&mut h, opened).unwrap();
    assert_eq!(number(initial[..8].to_vec()), 0);
    let epoch = 116_444_736_000_000_000u64;
    for time in initial[8..40].as_chunks::<8>().0 {
        assert!(number(time.to_vec()) > epoch);
    }
    assert_eq!(initial[40..], [0x24, 0, 0, 0, 0, 0, 0, 0]);
    let readonly = number(open(&mut h, saves, 0, "metadata.tmp").unwrap());
    assert_eq!(attrs(&mut h, readonly, 0), Err(Error::Denied));
    assert_eq!(
        host().call(25, &readonly.to_le_bytes(), 48),
        Err(Error::Denied)
    );
    let label = root.path().join("metadata.tmp");
    let disk = std::fs::File::options().write(true).open(&label).unwrap();
    disk.set_modified(std::time::UNIX_EPOCH - std::time::Duration::new(1, 500_000_000))
        .unwrap();
    let updated = observed(&mut h, readonly).unwrap();
    assert_eq!(number(updated[24..32].to_vec()), epoch - 15_000_000);
    #[cfg(target_os = "linux")]
    assert_eq!(initial[8..16], updated[8..16]);
    #[cfg(windows)]
    {
        // Windows reports the filesystem's creation time verbatim. Some
        // filesystems (and Wine) synthesize it from other file timestamps.
        use std::os::windows::fs::MetadataExt;
        assert_eq!(
            number(updated[8..16].to_vec()),
            disk.metadata().unwrap().creation_time()
        );
    }
    drop(disk);

    // Host writes (including timestamp changes) can affect native attributes.
    // Set and check the final staged attributes just before publishing.
    attrs(&mut h, opened, 0x24).unwrap();
    assert_eq!(observed(&mut h, readonly).unwrap()[40], 0x24);
    h.call(4, &readonly.to_le_bytes(), 0).unwrap();
    h.call(4, &opened.to_le_bytes(), 0).unwrap();
    let replacement = [
        saves.to_le_bytes().as_slice(),
        &0u32.to_le_bytes(),
        &12u32.to_le_bytes(),
        b"metadata.tmp",
        b"metadata.dat",
    ]
    .concat();
    h.call(14, &replacement, 0).unwrap();
    let reopened = number(open(&mut h, saves, 0, "metadata.dat").unwrap());
    assert_eq!(observed(&mut h, reopened).unwrap()[40], 0x24);
    assert_eq!(observed(&mut h, opened), Err(Error::Denied));
    assert_eq!(observed(&mut h, saves), Err(Error::Denied));
    let assets = grant(&mut h, root.path(), Access::Assets);
    let asset_file = number(open(&mut h, assets, 0, "metadata.dat").unwrap());
    assert_eq!(observed(&mut h, asset_file), Err(Error::Denied));
    h.revoke(saves).unwrap();
    assert_eq!(observed(&mut h, reopened), Err(Error::Denied));
}

#[test]
fn opened_file_metadata_has_canonical_times_and_strict_wire_lengths() {
    let root = tempfile::tempdir().unwrap();
    let name = root.path().join("save");
    std::fs::write(&name, b"complete save").unwrap();
    let disk = std::fs::File::options().write(true).open(&name).unwrap();
    let mut h = host();
    let saves = grant(&mut h, root.path(), Access::Save);
    let file = number(open(&mut h, saves, 1, "save").unwrap());
    for (time, seconds, nanos) in [
        (
            std::time::UNIX_EPOCH + std::time::Duration::new(1234, 567_000_000),
            1234i64,
            567_000_000u32,
        ),
        (
            std::time::UNIX_EPOCH - std::time::Duration::new(1, 250_000_000),
            -2,
            750_000_000,
        ),
    ] {
        disk.set_modified(time).unwrap();
        let reply = h.call(15, &file.to_le_bytes(), 24).unwrap();
        assert_eq!(
            reply,
            [
                13u64.to_le_bytes().as_slice(),
                &seconds.to_le_bytes(),
                &nanos.to_le_bytes(),
                &0u32.to_le_bytes()
            ]
            .concat()
        );
    }
    assert_eq!(h.call(15, &file.to_le_bytes(), 23), Err(Error::Invalid));
    for op in [15, 16] {
        assert_eq!(
            h.call(op, &file.to_le_bytes()[..7], 24),
            Err(Error::Invalid)
        );
        assert_eq!(
            h.call(op, &[file.to_le_bytes().as_slice(), &[0]].concat(), 24),
            Err(Error::Invalid)
        );
    }
}

#[test]
fn portable_paths_reject_traversal_device_names_ads_and_encoded_platform_paths() {
    let root = tempfile::tempdir().unwrap();
    let mut h = host();
    let cap = grant(&mut h, root.path(), Access::Save);
    for name in [
        "",
        "/etc/passwd",
        "../secret",
        "a/../../secret",
        "a//b",
        "a/./b",
        "a/",
        "C:/secret",
        "C:secret",
        "\\\\server\\share",
        "\\?\\C:\\secret",
        "x:stream",
        "a\\..\\b",
        "NUL",
        "con.txt",
        "COM1.bin",
        "LPT9",
        "conin$",
        "file.",
        "file ",
        "a\0b",
        "a\nb",
        "file?",
        "<file>",
    ] {
        assert_eq!(open(&mut h, cap, 1, name), Err(Error::Invalid), "{name:?}");
    }
    // URL escaping is not decoded into a second path interpretation.
    let literal = number(open(&mut h, cap, 1, "%2e%2e%2fsecret").unwrap());
    assert!(literal > 0);
    assert!(root.path().join("%2e%2e%2fsecret").exists());
}

#[cfg(unix)]
#[test]
fn symlink_junction_equivalents_hardlinks_and_special_files_cannot_escape_grants() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"untouched").unwrap();
    symlink(outside.path(), root.path().join("escape")).unwrap();
    symlink(outside.path().join("secret"), root.path().join("file")).unwrap();
    std::fs::hard_link(outside.path().join("secret"), root.path().join("hard")).unwrap();
    let fifo =
        std::ffi::CString::new(root.path().join("fifo").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let mut h = host();
    let cap = grant(&mut h, root.path(), Access::Save);
    for path in ["escape/secret", "file", "hard", "fifo"] {
        assert!(open(&mut h, cap, 1, path).is_err(), "{path}");
    }
    assert!(open(&mut h, cap, 0, "escape/secret").is_err());
    assert_eq!(
        std::fs::read(outside.path().join("secret")).unwrap(),
        b"untouched"
    );
}

#[cfg(windows)]
#[test]
fn windows_save_grant_rejects_hardlinks_to_files_outside_the_grant() {
    let root = tempfile::tempdir().unwrap();
    let saves = root.path().join("saves");
    let outside = root.path().join("outside");
    let link = saves.join("alias");
    std::fs::create_dir(&saves).unwrap();
    std::fs::write(&outside, b"untouched").unwrap();
    std::fs::hard_link(&outside, &link).unwrap();

    let mut h = host();
    let cap = grant(&mut h, &saves, Access::Save);
    for mode in [0, 1, 2] {
        assert_eq!(open(&mut h, cap, mode, "alias"), Err(Error::Denied));
    }
    let mut remove = cap.to_le_bytes().to_vec();
    remove.extend(b"alias");
    assert_eq!(h.call(8, &remove, 0), Err(Error::Denied));
    assert_eq!(std::fs::read(&outside).unwrap(), b"untouched");
    assert!(link.is_file());
}

#[test]
fn roms_are_read_in_place_with_offsets_beyond_four_gib_and_never_writable() {
    let mut f = tempfile::tempfile().unwrap();
    crate::test_support::mark_sparse(&f);
    let offset = (1u64 << 32) + 37;
    f.seek(SeekFrom::Start(offset)).unwrap();
    f.write_all(b"disc").unwrap();
    let mut h = host();
    let rom = h.grant_rom(f.try_clone().unwrap()).unwrap();
    assert_eq!(
        number(h.call(5, &rom.to_le_bytes(), 8).unwrap()),
        offset + 4
    );
    let req = [
        rom.to_le_bytes().as_slice(),
        &offset.to_le_bytes(),
        &4u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(h.call(2, &req, 4).unwrap(), b"disc");
    assert_eq!(
        h.call(
            3,
            &[rom.to_le_bytes().as_slice(), &offset.to_le_bytes(), b"oops"].concat(),
            0
        ),
        Err(Error::Denied)
    );
    f.seek(SeekFrom::Start(offset)).unwrap();
    let mut bytes = [0; 4];
    f.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"disc");
}

#[cfg(unix)]
#[test]
fn positional_rom_reads_do_not_move_a_shared_source_descriptor_cursor() {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(b"0123456789").unwrap();
    file.seek(SeekFrom::Start(7)).unwrap();
    let mut h = host();
    let id = h.grant_rom(file.try_clone().unwrap()).unwrap();
    let bytes = [
        id.to_le_bytes().as_slice(),
        &1u64.to_le_bytes(),
        &3u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(h.call(2, &bytes, 3).unwrap(), b"123");
    assert_eq!(file.stream_position().unwrap(), 7);
}

#[test]
fn media_validation_is_atomic_and_resource_handles_are_session_scoped() {
    let mut h = host();
    let texture = number(
        h.call(
            20,
            &[
                1u32.to_le_bytes().as_slice(),
                &1u32.to_le_bytes(),
                &[10, 20, 30, 255],
            ]
            .concat(),
            8,
        )
        .unwrap(),
    );
    let mut draw = texture.to_le_bytes().to_vec();
    for f in [0f32, 0.0, 100.0, 100.0] {
        draw.extend(f.to_le_bytes());
    }
    draw.extend(1u32.to_le_bytes());
    draw.extend(u32::MAX.to_le_bytes());
    let frame = [
        0x000000ffu32.to_le_bytes().as_slice(),
        &1u32.to_le_bytes(),
        &draw,
    ]
    .concat();
    h.call(22, &frame, 0).unwrap();
    assert_eq!(h.backend.frames.len(), 1);
    assert!(h.backend.frames[0].draws[0].linear);
    assert_eq!(host().call(22, &frame, 0), Err(Error::Denied));
    let mut bad = frame.clone();
    bad[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(h.call(22, &bad, 0), Err(Error::Invalid));
    let mut bad = frame.clone();
    bad[36..40].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(h.call(22, &bad, 0), Err(Error::Invalid));
    assert_eq!(h.backend.frames.len(), 1);
    h.backend.busy = true;
    assert_eq!(h.call(22, &frame, 0), Err(Error::Busy));
    h.call(21, &texture.to_le_bytes(), 0).unwrap();
    assert_eq!(h.call(22, &frame, 0), Err(Error::Denied));
    assert_eq!(
        h.backend.frames[0].draws[0].image.as_ref().unwrap().rgba,
        [10, 20, 30, 255]
    );
    for req in [vec![], vec![0; 8], vec![255; 8]] {
        assert!(h.call(20, &req, 8).is_err());
    }
}

#[test]
fn audio_window_and_network_permissions_precede_effects() {
    let mut p = permissions();
    p.audio = false;
    p.input = false;
    p.network = false;
    p.save = false;
    let mut h = Session::new(p, Recording::default());
    assert_eq!(
        h.call(
            30,
            &[
                2u32.to_le_bytes().as_slice(),
                &(-32768i16).to_le_bytes(),
                &32767i16.to_le_bytes()
            ]
            .concat(),
            0
        ),
        Err(Error::Denied)
    );
    assert_eq!(h.call(40, &[], 40), Err(Error::Denied));
    for op in 0u32..=9 {
        assert_eq!(h.call(50, &op.to_le_bytes(), 65536), Err(Error::Denied));
    }
    let root = tempfile::tempdir().unwrap();
    let dir =
        cap_std::fs::Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap();
    assert_eq!(h.grant_directory(dir, Access::Save), Err(Error::Denied));
    assert!(h.backend.audio.is_empty());
    assert_eq!(
        h.call(10, &[0u32.to_le_bytes(), 2u32.to_le_bytes()].concat(), 0),
        Err(Error::Invalid)
    );
    h.call(10, &[0u32.to_le_bytes(), 1u32.to_le_bytes()].concat(), 0)
        .unwrap();
    assert_eq!(h.backend.windows, [WindowCommand::Fullscreen(true)]);
    h.observe_window(WindowState {
        closed: true,
        ..WindowState::default()
    });
    assert_eq!(h.call(10, &4u32.to_le_bytes(), 0), Err(Error::Denied));
}

#[test]
fn keyboard_mouse_touch_text_controller_focus_and_rumble_share_one_service() {
    let mut h = host();
    h.observe_window(WindowState {
        focused: true,
        ..WindowState::default()
    });
    let k = h.input_backend().connect(1, 1, 123).unwrap();
    let m = h.input_backend().connect(2, 1, 124).unwrap();
    let t = h.input_backend().connect(4, 1, 125).unwrap();
    let p = h.input_backend().connect(3, 1, 126).unwrap();
    for event in [
        Event {
            device: k,
            kind: 1,
            code: 4,
            ..Event::default()
        },
        Event {
            device: k,
            kind: 17,
            code: 0x1f600,
            ..Event::default()
        },
        Event {
            device: m,
            kind: 3,
            x: 123.0,
            y: 456.0,
            ..Event::default()
        },
        Event {
            device: m,
            kind: 4,
            x: -4.5,
            y: 2.5,
            ..Event::default()
        },
        Event {
            device: m,
            kind: 5,
            code: 1,
            ..Event::default()
        },
        Event {
            device: m,
            kind: 7,
            x: 0.25,
            y: -1.5,
            ..Event::default()
        },
        Event {
            device: t,
            kind: 8,
            contact: u64::MAX,
            x: 0.5,
            y: 0.7,
            ..Event::default()
        },
    ] {
        assert!(h.input_backend().submit(event));
    }
    h.input_backend().update_gamepad(Gamepad {
        device: p,
        buttons: 1 << 12,
        left_x: i16::MIN,
        right_trigger: u16::MAX,
        ..Gamepad::default()
    });
    assert_eq!(
        h.call(
            42,
            &[k.to_le_bytes().as_slice(), &4u32.to_le_bytes()].concat(),
            4
        )
        .unwrap(),
        1u32.to_le_bytes()
    );
    let pointer = h.call(43, &m.to_le_bytes(), 32).unwrap();
    assert_eq!(&pointer[24..28], &1u32.to_le_bytes());
    let touch = h.call(44, &[], 32).unwrap();
    assert_eq!(&touch[16..24], &u64::MAX.to_le_bytes());
    let pad = h.call(41, &0u32.to_le_bytes(), 32).unwrap();
    assert_eq!(&pad[..8], &p.to_le_bytes());
    assert_eq!(&pad[20..22], &i16::MIN.to_le_bytes());
    let events = h.call(40, &[], 40 * 100).unwrap();
    assert!(events
        .as_chunks::<40>()
        .0
        .iter()
        .any(|e| u32::from_le_bytes(e[16..20].try_into().unwrap()) == 17
            && u32::from_le_bytes(e[20..24].try_into().unwrap()) == 0x1f600));
    h.call(
        45,
        &[
            0u32.to_le_bytes(),
            1u32.to_le_bytes(),
            2u32.to_le_bytes(),
            100u32.to_le_bytes(),
        ]
        .concat(),
        0,
    )
    .unwrap();
    assert_eq!(h.backend.rumble, [p]);
    h.observe_window(WindowState::default());
    assert_eq!(
        h.call(
            42,
            &[k.to_le_bytes().as_slice(), &4u32.to_le_bytes()].concat(),
            4
        )
        .unwrap(),
        0u32.to_le_bytes()
    );
    assert!(h.call(44, &[], 32).unwrap().is_empty());
    let pad = h.call(41, &0u32.to_le_bytes(), 32).unwrap();
    assert_eq!(&pad[16..32], &[0; 16]);
    assert_eq!(
        h.call(
            45,
            &[
                0u32.to_le_bytes(),
                1u32.to_le_bytes(),
                2u32.to_le_bytes(),
                100u32.to_le_bytes()
            ]
            .concat(),
            0
        ),
        Err(Error::Denied)
    );
}

#[test]
fn framed_transport_is_bounded_and_recovers_from_rejected_requests() {
    let mut input = Vec::new();
    for (op, body, cap) in [(u32::MAX, b"".as_slice(), 0u32), (0, b"".as_slice(), 4)] {
        input.extend(b"GPH1");
        input.extend(op.to_le_bytes());
        input.extend((body.len() as u32).to_le_bytes());
        input.extend(cap.to_le_bytes());
        input.extend(body);
    }
    let mut output = Vec::new();
    serve(&mut host(), input.as_slice(), &mut output).unwrap();
    assert_eq!(
        i32::from_le_bytes(output[..4].try_into().unwrap()),
        Error::Unsupported as i32
    );
    assert_eq!(&output[16..20], &1u32.to_le_bytes());
    for bytes in [
        b"GPH1".to_vec(),
        [
            b"GPH1".as_slice(),
            &0u32.to_le_bytes(),
            &u32::MAX.to_le_bytes(),
            &4u32.to_le_bytes(),
        ]
        .concat(),
    ] {
        assert!(serve(&mut host(), bytes.as_slice(), Vec::new()).is_err());
    }
    assert_eq!(host().call(0, &[], u32::MAX), Err(Error::Invalid));
}

#[test]
fn malformed_calls_never_panic_and_never_reach_a_backend_partially() {
    let mut h = host();
    let mut random = 0x123456789abcdefu64;
    for op in [
        0,
        1,
        2,
        3,
        4,
        5,
        6,
        7,
        8,
        9,
        10,
        11,
        12,
        20,
        21,
        22,
        30,
        40,
        41,
        42,
        43,
        44,
        45,
        46,
        50,
        60,
        61,
        62,
        u32::MAX,
    ] {
        for length in 0..128 {
            let mut bytes = Vec::new();
            for _ in 0..length {
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                bytes.push(random as u8);
            }
            let _ = h.call(op, &bytes, 256);
        }
    }
    assert!(h.backend.frames.is_empty());
    assert!(h.backend.audio.is_empty());
    assert!(h.backend.windows.is_empty());
}

#[test]
fn save_listing_creation_resize_flush_and_removal_are_capability_relative() {
    let root = tempfile::tempdir().unwrap();
    let mut h = host();
    let grant = grant(&mut h, root.path(), Access::Save);
    h.call(
        7,
        &[grant.to_le_bytes().as_slice(), b"profiles"].concat(),
        0,
    )
    .unwrap();
    let file = number(open(&mut h, grant, 1, "profiles/one.sav").unwrap());
    h.call(9, &[file.to_le_bytes(), 7u64.to_le_bytes()].concat(), 0)
        .unwrap();
    h.call(12, &file.to_le_bytes(), 0).unwrap();
    assert_eq!(number(h.call(5, &file.to_le_bytes(), 8).unwrap()), 7);
    let listing = h
        .call(
            6,
            &[grant.to_le_bytes().as_slice(), b"profiles"].concat(),
            100,
        )
        .unwrap();
    assert_eq!(&listing[8..], b"one.sav");
    assert_eq!(h.call(6, &grant.to_le_bytes(), 0), Err(Error::Busy));
    h.call(4, &file.to_le_bytes(), 0).unwrap();
    h.call(
        8,
        &[grant.to_le_bytes().as_slice(), b"profiles/one.sav"].concat(),
        0,
    )
    .unwrap();
    assert!(!root.path().join("profiles/one.sav").exists());
}

#[test]
fn enabled_multiplayer_reaches_local_peers_only_through_host_owned_handles() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = [0; 4];
        s.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ping");
        s.write_all(b"pong").unwrap();
    });
    let mut p = permissions();
    p.network = true;
    let mut h = Session::new(p, Recording::default());
    let until = |h: &mut Session<Recording>, request: &[u8], capacity: u32| {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match h.call(50, request, capacity) {
                Err(Error::Busy) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(2))
                }
                result => break result,
            }
        }
    };
    let open = [
        2u32.to_le_bytes().as_slice(),
        address.to_string().as_bytes(),
    ]
    .concat();
    let id = h.call(50, &open, 4).unwrap();
    assert_eq!(id.len(), 4);
    until(&mut h, &[1u32.to_le_bytes().as_slice(), &id].concat(), 0).unwrap();
    let ack = until(
        &mut h,
        &[4u32.to_le_bytes().as_slice(), &id, b"ping"].concat(),
        4,
    )
    .unwrap();
    assert_eq!(ack, 4u32.to_le_bytes());
    assert_eq!(
        until(&mut h, &[5u32.to_le_bytes().as_slice(), &id].concat(), 16).unwrap(),
        b"pong"
    );
    server.join().unwrap();
    h.call(50, &[6u32.to_le_bytes().as_slice(), &id].concat(), 0)
        .unwrap();
    assert_eq!(
        h.call(50, &[5u32.to_le_bytes().as_slice(), &id].concat(), 16),
        Err(Error::Denied)
    );
}

#[test]
fn microphone_requires_its_own_signed_permission_and_revocation_discards_pending_data() {
    struct Microphone {
        queue: capture::Capture,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl Backend for Microphone {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Ok(())
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Ok(())
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Ok(())
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn capture(&mut self, _: u32, _: &[u8], capacity: usize) -> Result<Vec<u8>> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.queue.read(1, capacity)
        }
        fn revoke_capture(&mut self) {
            self.queue.close();
        }
    }
    let mut p = permissions();
    p.audio = true;
    p.network = true;
    let queue = capture::Capture::new(1, 8, 4096).unwrap();
    queue.submit(&[123]).unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut host = Session::new(
        p.clone(),
        Microphone {
            queue: queue.clone(),
            calls: calls.clone(),
        },
    );
    assert_eq!(host.call(141, &[], 2), Err(Error::Denied));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    p.microphone = true;
    let mut host = Session::new(
        p,
        Microphone {
            queue: queue.clone(),
            calls: calls.clone(),
        },
    );
    assert_eq!(host.call(141, &[], 2).unwrap(), 123i16.to_le_bytes());
    queue.submit(&[456]).unwrap();
    host.revoke_microphone();
    assert_eq!(host.call(141, &[], 2), Err(Error::Denied));
    assert_eq!(queue.read(1, 2), Err(Error::Denied));
    assert_eq!(queue.submit(&[789]), Err(Error::Denied));
    // A separately signed request still cannot manufacture a provider.
    let mut p = permissions();
    p.microphone = true;
    let mut absent = Session::new(p, Recording::default());
    assert_eq!(absent.call(140, &[], 8), Err(Error::Unsupported));
}

#[test]
fn numeric_diagnostics_are_bounded_copied_ordered_session_private_and_revocable() {
    let mut p = permissions();
    p.input = false;
    p.audio = false;
    p.save = false;
    p.network = false;
    let mut host = Session::new(p.clone(), Recording::default());
    let mut other = Session::new(p, Recording::default());
    let packet = |level: u32, code: u32, values: &[u64]| {
        let mut bytes = [
            level.to_le_bytes(),
            code.to_le_bytes(),
            (values.len() as u32).to_le_bytes(),
        ]
        .concat();
        for value in values {
            bytes.extend(value.to_le_bytes());
        }
        bytes
    };
    let mut request = packet(3, u32::MAX, &[0, u64::MAX]);
    assert_eq!(host.call(63, &request, 8).unwrap(), 1u64.to_le_bytes());
    request.fill(0);
    let summary = host.diagnostic_summary();
    assert!(summary.contains("code=0xFFFFFFFF 0000000000000000 FFFFFFFFFFFFFFFF"));
    assert!(!summary.contains('\u{1b}'));
    assert_eq!(host.diagnostic_summary(), summary);
    assert!(other.diagnostic_summary().is_empty());
    let (events, lost) = host.take_diagnostics();
    assert_eq!(lost, 0);
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].level, events[0].code), (3, u32::MAX));
    assert_eq!(events[0].values, [0, u64::MAX]);
    assert!(other.take_diagnostics().0.is_empty());
    let valid = packet(0, 42, &[u64::MAX; 32]);
    for malformed in [
        vec![],
        valid[..valid.len() - 1].to_vec(),
        [valid.clone(), vec![0]].concat(),
        packet(4, 0, &[]),
        packet(0, 0, &[0; 33]),
    ] {
        assert_eq!(host.call(63, &malformed, 8), Err(Error::Invalid));
    }
    assert_eq!(host.call(63, &valid, 7), Err(Error::Invalid));
    assert!(host.take_diagnostics().0.is_empty());
    for sequence in 2u64..=131 {
        assert_eq!(host.call(63, &valid, 8).unwrap(), sequence.to_le_bytes());
    }
    let (events, lost) = host.take_diagnostics();
    assert_eq!(lost, 2);
    assert_eq!(events.len(), 128);
    assert_eq!(events.first().unwrap().sequence, 4);
    assert_eq!(events.last().unwrap().sequence, 131);
    assert!(host.diagnostic_summary().is_empty());
    assert!(events
        .windows(2)
        .all(|pair| pair[0].monotonic_ns <= pair[1].monotonic_ns));
    assert_eq!(host.take_diagnostics(), (vec![], 0));
    host.timing.close();
    assert_eq!(host.call(63, &valid, 8), Err(Error::Denied));
    assert!(host.take_diagnostics().0.is_empty());
    assert_eq!(
        other.call(63, &packet(1, 0, &[]), 8).unwrap(),
        1u64.to_le_bytes()
    );
}

#[test]
fn byte_diagnostics_preserve_raw_data_escape_summaries_and_validate_before_mutation() {
    let mut host = Session::new(permissions(), Recording::default());
    let mut other = Session::new(permissions(), Recording::default());
    let packet = |level: u32, code: u32, payload: &[u8]| {
        let mut request = [
            level.to_le_bytes(),
            code.to_le_bytes(),
            (payload.len() as u32).to_le_bytes(),
        ]
        .concat();
        request.extend(payload);
        request
    };
    let payload = b"x\0\x1b\xff\n";
    let mut request = packet(1, 77, payload);
    assert_eq!(host.call(64, &request, 8).unwrap(), 1u64.to_le_bytes());
    request.fill(0);
    let summary = host.diagnostic_summary();
    assert!(!summary.contains('\u{1b}'));
    assert!(!summary.contains('\0'));
    assert!(summary.contains("\\xff"));
    let (events, lost) = host.take_diagnostics();
    assert_eq!(lost, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].bytes.as_deref(), Some(payload.as_slice()));
    assert!(events[0].values.is_empty());
    assert!(other.take_diagnostics().0.is_empty());
    let valid = packet(0, 3, &[1; 128 * 1024]);
    for invalid in [
        vec![],
        valid[..valid.len() - 1].to_vec(),
        [valid.clone(), vec![0]].concat(),
        packet(4, 0, &[]),
        packet(0, 0, &[0; 128 * 1024 + 1]),
    ] {
        assert_eq!(host.call(64, &invalid, 8), Err(Error::Invalid));
    }
    assert_eq!(host.call(64, &valid, 7), Err(Error::Invalid));
    assert!(host.take_diagnostics().0.is_empty());
    assert_eq!(
        host.call(64, &packet(0, 3, &[]), 8).unwrap(),
        2u64.to_le_bytes()
    );
    let numeric = [1u32.to_le_bytes(), 4u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
    assert_eq!(host.call(63, &numeric, 8).unwrap(), 3u64.to_le_bytes());
    let (events, lost) = host.take_diagnostics();
    assert_eq!(lost, 0);
    assert_eq!(events[0].bytes, Some(vec![]));
    assert_eq!(events[1].bytes, None);
    assert!(events[0].monotonic_ns <= events[1].monotonic_ns);
    assert_eq!(host.call(64, &valid, 8).unwrap(), 4u64.to_le_bytes());
    let (events, lost) = host.take_diagnostics();
    assert_eq!(lost, 0);
    assert_eq!(
        events[0].bytes.as_deref(),
        Some([1u8; 128 * 1024].as_slice())
    );
    host.timing.close();
    assert_eq!(host.call(64, &valid, 8), Err(Error::Denied));
    assert!(host.take_diagnostics().0.is_empty());
}

#[test]
fn native_png_asset_requires_read_only_asset_grant_and_returns_a_releasable_image() {
    let dir = tempfile::tempdir().unwrap();
    let png = include_bytes!("../../examples/minimal/thumbnail.png");
    std::fs::write(dir.path().join("sample.png"), png).unwrap();
    std::fs::write(dir.path().join("invalid.png"), b"not a PNG").unwrap();
    let mut host = host();
    let asset = grant(&mut host, dir.path(), Access::Assets);
    let save = grant(&mut host, dir.path(), Access::Save);
    let request = |root: u64, name: &[u8]| [root.to_le_bytes().as_slice(), name].concat();
    let image = number(host.call(69, &request(asset, b"sample.png"), 8).unwrap());
    assert!(!host.images[&image].rgba.is_empty());
    assert_eq!(
        host.call(69, &request(save, b"sample.png"), 8),
        Err(Error::Denied)
    );
    assert!(host.call(69, &request(asset, b"../sample.png"), 8).is_err());
    assert_eq!(
        host.call(69, &request(asset, b"invalid.png"), 8),
        Err(Error::Invalid)
    );
    host.call(21, &image.to_le_bytes(), 0).unwrap();
    assert!(host.images.is_empty());
}

#[test]
fn tick_window_controls_require_real_gesture_and_commit_on_accepted_frame() {
    let mut host = host();
    let manifest =
        crate::manifest::Manifest::parse(include_bytes!("../../examples/minimal/manifest.toml"))
            .unwrap();
    host.tick = Some(tick_services::Services::new(&manifest).unwrap());
    host.observe_window(WindowState {
        width: 640,
        height: 480,
        focused: true,
        ..Default::default()
    });
    let toggle = [2u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
    let frame = [0u32.to_le_bytes(), 0u32.to_le_bytes()].concat();
    assert_eq!(host.call(71, &toggle, 4), Err(Error::Denied));
    host.call(40, &[], 40).unwrap();
    assert_eq!(host.call(71, &toggle, 4), Ok(vec![0; 4]));
    host.call(70, &frame, 0).unwrap();

    let keyboard = host.input_backend().connect(1, 1, 1).unwrap();
    host.input_backend().submit(Event {
        device: keyboard,
        kind: 1,
        code: 40,
        ..Default::default()
    });
    host.call(40, &[], 40 * 8).unwrap();
    assert_eq!(host.call(71, &toggle, 4), Ok(1u32.to_le_bytes().to_vec()));
    host.backend().busy = true;
    assert_eq!(host.call(70, &frame, 0), Err(Error::Busy));
    assert!(host.backend().windows.is_empty());
    host.backend().busy = false;
    host.call(70, &frame, 0).unwrap();
    assert_eq!(host.backend().windows, [WindowCommand::Fullscreen(true)]);
    assert_eq!(host.call(71, &toggle, 4), Err(Error::Denied));
}

#[test]
fn tick_compositor_validates_clip_and_primitive_records_atomically() {
    let mut host = host();
    let pixel = [1u32.to_le_bytes(), 1u32.to_le_bytes(), [255, 0, 0, 255]].concat();
    let image = number(host.call(20, &pixel, 8).unwrap());
    let frame = |flags: u32, width: f32| {
        let mut request = [0u32.to_le_bytes(), 1u32.to_le_bytes()].concat();
        request.extend(image.to_le_bytes());
        for v in [2.0f32, 3.0, 10.0, 10.0] {
            request.extend(v.to_le_bytes());
        }
        request.extend(0u32.to_le_bytes());
        request.extend(u32::MAX.to_le_bytes());
        request.extend(flags.to_le_bytes());
        for v in [1.0f32, 2.0, width, 5.0] {
            request.extend(v.to_le_bytes());
        }
        request
    };
    for invalid in [
        frame(16, 5.0),
        frame(7, 5.0),
        frame(3, -1.0),
        frame(3, f32::NAN),
    ] {
        assert_eq!(host.call(70, &invalid, 0), Err(Error::Invalid));
    }
    assert!(host.backend().frames.is_empty());
    host.call(70, &frame(3, 5.0), 0).unwrap();
    let draw = &host.backend().frames[0].draws[0];
    assert!(draw.circle);
    assert_eq!(draw.clip, Some([1.0, 2.0, 5.0, 5.0]));
    let mut solid = frame(9, 5.0);
    solid[8..16].copy_from_slice(&0x102030ffu64.to_le_bytes());
    host.call(70, &solid, 0).unwrap();
    let draw = &host.backend().frames[1].draws[0];
    assert!(draw.image.is_none());
    assert_eq!(draw.solid, Some(0x102030ff));
    solid[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(host.call(70, &solid, 0), Err(Error::Invalid));
}
