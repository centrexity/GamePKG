#![cfg(any(target_os = "linux", windows))]
#[allow(dead_code)]
#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::port_host::preparation::{LocalLayout, Plan, Target};
#[cfg(target_os = "linux")]
use gamepkg::port_host::{admission::CheckedSources, preparation};

#[cfg(target_os = "linux")]
fn assert_doom_object_receipts(receipt: &preparation::ExecutableReceipt) {
    assert!(receipt
        .arguments
        .iter()
        .any(|arg| arg == "codegen-units=64"));
    let audited = &receipt.game.objects;
    assert!((2..=64).contains(&audited.len()));
    assert_eq!(audited.len(), receipt.raw_objects.len());
    for (index, (object, raw)) in audited.iter().zip(&receipt.raw_objects).enumerate() {
        assert_eq!(object.name, format!("game-{index:04}.o"));
        assert_eq!(raw.name, object.name);
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a signed Doom migration package and the reviewed Windows cross-toolchains"]
fn signed_doom_native_objects_pass_windows_admission() {
    let package = std::env::var_os("GAMEPKG_TEST_DOOM_NATIVE_PACKAGE")
        .expect("select the signed native Doom test package");
    let mut package = gamepkg::archive::Package::open(std::path::Path::new(&package)).unwrap();
    let sources = CheckedSources::verify(&mut package).unwrap();
    let manifest = sources.manifest();
    assert_eq!(manifest.id, "com.gamepkg.doom");
    assert!(manifest.uses_service_runtime());
    let cache = tempfile::tempdir().unwrap();
    for target in [Target::Windows64, Target::Windows32] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true).unwrap();
        // This fixture exercises source admission, optimized COFF compilation,
        // object auditing and PE linking. The fixture transport does not
        // qualify the desktop compatibility bootstrap.
        let artifact = preparation::build(cache.path(), &sources, &plan, vec![], vec![])
            .unwrap_or_else(|error| panic!("{target:?}: {error:#}"));
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.source_kit, sources.identity());
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_doom_object_receipts(receipt);
        assert!(artifact.executable().unwrap().is_file());
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", feature = "gui"))]
#[test]
#[ignore = "requires a signed Doom migration package and the reviewed Windows cross-toolchains"]
fn signed_doom_links_the_windows_compatibility_runtime() {
    use gamepkg::port_host::preparation::Bootstrap;
    let package = std::env::var_os("GAMEPKG_TEST_DOOM_NATIVE_PACKAGE")
        .expect("select the signed native Doom test package");
    let mut package = gamepkg::archive::Package::open(std::path::Path::new(&package)).unwrap();
    let sources = CheckedSources::verify(&mut package).unwrap();
    assert_eq!(sources.manifest().id, "com.gamepkg.doom");
    let cache = tempfile::tempdir().unwrap();
    for (target, filename) in [
        (Target::Windows64, "doom-compatibility-x64.exe"),
        (Target::Windows32, "doom-compatibility-x86.exe"),
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true).unwrap();
        let artifact = preparation::build_in_process(cache.path(), &sources, &plan, None)
            .unwrap_or_else(|error| panic!("{target:?}: {error:#}"));
        let receipt = artifact.receipt();
        assert_eq!(receipt.bootstrap, Bootstrap::InProcess);
        assert_eq!(receipt.plan, plan);
        assert_eq!(receipt.game.source_kit, sources.identity());
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_doom_object_receipts(receipt);
        let executable = artifact.executable().unwrap();
        assert!(executable.is_file());
        if let Some(output) = std::env::var_os("GAMEPKG_TEST_DOOM_WINDOWS_OUTPUT") {
            std::fs::copy(executable, std::path::Path::new(&output).join(filename)).unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "four-target source/object admission and actual confined Linux clock workers"]
fn local_deadlines_execute_without_owner_dispatch_and_keep_every_object_audited() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Error, Frame, Result, WindowCommand,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_local_deadline.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec!["deadline_worker".into(), "event_worker".into()],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(8)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.game.source_kit, checked.identity());
        assert_eq!(receipt.game.objects.len(), receipt.raw_objects.len());
        assert!(!receipt.game.objects.is_empty());
        // rustc may omit empty partitions. Every emitted partition must still
        // be audited, including a genuinely split game compilation.
        assert!(receipt.game.objects.len() > 1);
        artifact.executable().unwrap();
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let queued = Arc::new(AtomicBool::new(false));
        let signal = queued.clone();
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            move || {
                signal.store(true, Ordering::Release);
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        // The first and only remote request is the final report. Do not service
        // it until all local deadlines, worker wake/join and negative calls ran.
        while !queued.load(Ordering::Acquire) {
            assert!(owner.poll().unwrap().is_none(), "{}", owner.diagnostics());
            assert!(
                Instant::now() < end,
                "local timing waited for owner dispatch"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 0x434c4bff);
        assert_eq!(events[0].values, [0]);
        owner.stop().unwrap();
    }
    // Infrastructure qualification must not admit an unmigrated engine.
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "four-target private join-evidence admission and actual confined Linux workers"]
fn local_join_evidence_is_unforgeable_and_every_object_remains_audited() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Error, Frame, Result, WindowCommand,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_local_join.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec!["joined_worker".into()],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(8)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.game.source_kit, checked.identity());
        assert_eq!(receipt.game.objects.len(), receipt.raw_objects.len());
        assert!(!receipt.game.objects.is_empty());
        // rustc may omit empty partitions. Every emitted partition must still
        // be audited, including a genuinely split game compilation.
        assert!(receipt.game.objects.len() > 1);
        artifact.executable().unwrap();
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let queued = Arc::new(AtomicBool::new(false));
        let signal = queued.clone();
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            move || {
                signal.store(true, Ordering::Release);
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        // The first and only remote request is the final report. Do not service
        // it until all local deadlines, worker wake/join and negative calls ran.
        while !queued.load(Ordering::Acquire) {
            assert!(owner.poll().unwrap().is_none(), "{}", owner.diagnostics());
            assert!(Instant::now() < end, "local join waited for owner dispatch");
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 0x4a4f49ff);
        assert_eq!(events[0].values, [0]);
        owner.stop().unwrap();
    }
    files.insert("source/src/lib.rs".into(),b"pub fn game_main(){let forged=gamepkg::LocalTaskJoin{task:1,value:37};let _=forged.task_id();}".to_vec());
    let forged = helper::sign(&files, root.path());
    let forged =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&forged).unwrap()).unwrap();
    let plan = Plan::new(
        Target::Linux64,
        "game_main",
        vec![],
        LocalLayout::None,
        true,
    )
    .unwrap();
    let denied = match preparation::build(root.path(), &forged, &plan, vec![], vec![]) {
        Ok(_) => panic!("game source forged host join evidence"),
        Err(error) => error.to_string(),
    };
    assert!(denied.contains("private"), "{denied}");
    // Infrastructure qualification must not admit an unmigrated engine.
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}

#[test]
fn reviewed_entry_metadata_cannot_inject_code_or_select_an_incompatible_layout() {
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true).unwrap();
        let record = serde_json::to_vec(&plan).unwrap();
        assert_eq!(serde_json::from_slice::<Plan>(&record).unwrap(), plan);
        assert_ne!(
            plan.identity(),
            Plan::new(target, "game_main", vec![], LocalLayout::None, true)
                .unwrap()
                .identity()
        );
        let full = Plan::new(
            target,
            "game_main",
            vec![],
            LocalLayout::TakedawnMemory,
            true,
        )
        .unwrap();
        let scratch = Plan::new(
            target,
            "game_main",
            vec![],
            LocalLayout::TakedawnServices,
            true,
        )
        .unwrap();
        assert_ne!(scratch.identity(), full.identity());
        assert_eq!(
            serde_json::from_slice::<Plan>(&serde_json::to_vec(&scratch).unwrap()).unwrap(),
            scratch
        );
        assert_ne!(plan.identity(), full.identity());
        assert_eq!(
            serde_json::from_slice::<Plan>(&serde_json::to_vec(&full).unwrap()).unwrap(),
            full
        );
    }
    assert!(Plan::new(
        Target::Linux64,
        "game_main();unsafe{}",
        vec![],
        LocalLayout::None,
        true
    )
    .is_err());
    assert!(Plan::new(
        Target::Linux64,
        "game_main",
        vec![],
        LocalLayout::HaloI686,
        true
    )
    .is_err());
    assert!(Plan::new(
        Target::Windows32,
        "game_main",
        vec!["x::run".into()],
        LocalLayout::None,
        true
    )
    .is_err());
    let a = Plan::new(
        Target::Linux64,
        "game_main",
        vec![],
        LocalLayout::None,
        true,
    )
    .unwrap();
    let b = Plan::new(
        Target::Linux32,
        "game_main",
        vec![],
        LocalLayout::None,
        true,
    )
    .unwrap();
    assert_ne!(a.identity(), b.identity());
    assert_ne!(
        a.identity(),
        a.clone().with_codegen_units(4).unwrap().identity()
    );
    assert!(a.clone().with_codegen_units(0).is_err());
    assert!(a.clone().with_codegen_units(513).is_err());
    assert_ne!(
        a.identity(),
        a.clone()
            .with_compiler_memory_bytes(512 * 1024 * 1024)
            .unwrap()
            .identity()
    );
    assert!(a.clone().with_compiler_memory_bytes(0).is_err());
    assert!(a
        .clone()
        .with_compiler_memory_bytes(48 * 1024 * 1024 * 1024 + 1)
        .is_err());
    assert!(a.clone().with_codegen_units(512).is_ok());
    assert_eq!(
        a.clone()
            .with_compiler_memory_bytes(48 * 1024 * 1024 * 1024)
            .is_ok(),
        !cfg!(all(windows, target_pointer_width = "32"))
    );
    let record = serde_json::to_value(&a).unwrap();
    assert_eq!(record["codegen_units"], 64);
    let compiler_gib = if cfg!(all(windows, target_pointer_width = "32")) {
        2
    } else {
        24
    };
    assert_eq!(
        record["compiler_memory_bytes"],
        compiler_gib * 1024 * 1024 * 1024u64
    );
    for (field, value) in [
        ("entry", serde_json::json!("game_main();unsafe{bad()}")),
        ("tasks", serde_json::json!(["core::bad"])),
        ("layout", serde_json::json!("HaloI686")),
        ("tasks", serde_json::json!(vec!["task"; 257])),
        ("unreviewed_flag", serde_json::json!(true)),
        ("codegen_units", serde_json::json!(0)),
        ("codegen_units", serde_json::json!(513)),
        ("codegen_units", serde_json::json!(-1)),
        ("codegen_units", serde_json::json!(1.5)),
        ("compiler_memory_bytes", serde_json::json!(0)),
        (
            "compiler_memory_bytes",
            serde_json::json!(256 * 1024 * 1024 - 1u64),
        ),
        (
            "compiler_memory_bytes",
            serde_json::json!(48 * 1024 * 1024 * 1024 + 1u64),
        ),
        ("compiler_memory_bytes", serde_json::json!(-1)),
        ("compiler_memory_bytes", serde_json::json!(1.5)),
    ] {
        let mut changed = record.clone();
        changed[field] = value;
        assert!(serde_json::from_value::<Plan>(changed).is_err(), "{field}");
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "executes actual confined game-object compilation and trusted linking"]
fn separately_optimized_rust_objects_link_only_after_audit_and_bind_final_receipt() {
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    files.insert("source/src/lib.rs".into(),b"pub fn game_main(){let _=gamepkg::version();let mut samples=std::vec::Vec::new();samples.push(100i16);samples.push(200i16);let _=gamepkg::audio_stream_submit(1,&samples);}".to_vec());
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true)
            .unwrap()
            .with_codegen_units(1)
            .unwrap();
        let artifact =
            preparation::build(root.path(), &sources, &plan, vec![], vec!["ab".repeat(32)])
                .unwrap();
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.source_kit, sources.identity());
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.plan, plan);
        assert_eq!(receipt.raw_objects.len(), receipt.game.objects.len());
        assert_eq!(
            receipt.object_normalization,
            preparation::ObjectNormalization::None
        );
        assert_eq!(receipt.raw_objects, receipt.game.objects);

        assert_eq!(receipt.shader_preparation, vec!["ab".repeat(32)]);
        assert_eq!(receipt.game.objects.len(), 1);
        assert!(receipt.arguments.iter().any(|x| x == "lto=off"));
        assert!(receipt.arguments.iter().any(|x| x == "panic=abort"));
        let bytes = std::fs::read(artifact.executable().unwrap()).unwrap();
        assert_eq!(receipt.binary, gamepkg::verify::digest(&bytes));
        assert_eq!(
            serde_json::from_slice::<preparation::ExecutableReceipt>(
                &serde_json::to_vec(receipt).unwrap()
            )
            .unwrap(),
            *receipt
        );
        let cache = root.path().join(format!("cache-{target:?}"));
        let installed = artifact.publish(&cache).unwrap();
        assert_eq!(std::fs::read(installed).unwrap(), bytes);
        let executable = artifact.executable().unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(0o600);
        std::fs::set_permissions(&executable, permissions).unwrap();
        std::fs::write(executable, b"tampered").unwrap();
        assert!(artifact.executable().is_err());
    }
    files.insert(
        "source/src/lib.rs".into(),
        b"pub fn game_main(){let _=gamepkg::connect_local(0);}".to_vec(),
    );
    let denied = tempfile::tempdir().unwrap();
    let package = helper::sign(&files, denied.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    let plan = Plan::new(
        Target::Linux64,
        "game_main",
        vec![],
        LocalLayout::None,
        true,
    )
    .unwrap();
    let error = preparation::build(root.path(), &sources, &plan, vec![], vec![])
        .err()
        .expect("game source must not install a trusted dispatcher");
    assert!(format!("{error:#}").contains("cannot find function `connect_local`"));
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "compiles and executes real confined x64/i686 native children; no physical audio"]
fn owner_executes_audited_children_with_private_pipes_rom_and_atomic_saves() {
    use gamepkg::port_host::owner::{Grants, Owner, SaveGrant};
    use gamepkg::port_host::{Backend, Frame, Result, WindowCommand};
    use std::sync::{Arc, Mutex};
    struct Output(Arc<Mutex<Vec<i16>>>);
    impl Backend for Output {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Ok(())
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Ok(())
        }
        fn audio(&mut self, data: &[i16], _: u32) -> Result<()> {
            self.0.lock().unwrap().extend(data);
            Ok(())
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.permissions.save = true;
    manifest.permissions.audio = true;
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert("source/src/lib.rs".into(), br#"
pub fn service_task(argument:u64)->u64 {
    match gamepkg::local_task_status(){Ok((id,false))=>{if id==0{return 0;}},_=>return 0}
    let mut message=[0u8;8];
    if gamepkg::local_mailbox_receive(argument,1000,&mut message)!=Ok(8){return 0;}
    let result=u64::from_le_bytes(message)+1;
    if gamepkg::local_mailbox_send(argument,&u64::to_le_bytes(result))!=Ok(()){return 0;}
    result
}
pub fn game_main() {
    let grants = match gamepkg::grants() { Ok(v) => v, Err(_) => return };
    let mut read_game_data = false;
    for grant in grants {
        if grant.kind == 6 {
            if gamepkg::file_exists(grant.id,"data/fixture") != Ok(true) { return; }
            if gamepkg::file_open(grant.id,true,"data/fixture") != Err(-4) { return; }
            let file = match gamepkg::file_open(grant.id,false,"data/fixture") {
                Ok(file) => file, Err(_) => return,
            };
            let mut data = [0u8;8];
            if gamepkg::file_read(file,0,&mut data) != Ok(8) || data != *b"selected" { return; }
            if gamepkg::file_write(file,0,b"changed") != Err(-4) { return; }
            if gamepkg::file_close(file) != Ok(()) { return; }
            read_game_data = true;
        }
        if grant.kind == 3 {
            if gamepkg::save_volume(grant.id) != Err(-4) { return; }
            if gamepkg::save_file_info(grant.id).err() != Some(-4) { return; }
            if gamepkg::file_exists(grant.id,"save") != Err(-4) { return; }
            let size = match gamepkg::file_size(grant.id) { Ok(v) => v, Err(_) => return };
            if size != 4294967297u64 { return; }
            let mut byte = [0u8;1];
            if gamepkg::file_read(grant.id,4294967296u64,&mut byte) != Ok(1) { return; }
            if byte[0] != 73 { return; }
            if gamepkg::file_write(grant.id,0,&byte) != Err(-4) { return; }
        }
        if grant.kind == 4 {
            if gamepkg::file_write(grant.id,1048575,b"x") != Ok(()) { return; }
            let mut byte=[0u8;1];
            if gamepkg::file_read(grant.id,1048575,&mut byte) != Ok(1) || byte[0] != 120 { return; }
            if gamepkg::file_write(grant.id,1048576,b"!") != Err(-1) { return; }
            if gamepkg::file_close(grant.id) != Ok(()) { return; }
            if gamepkg::file_size(grant.id) != Err(-4) { return; }
        }
        if grant.kind == 2 {
            if gamepkg::file_exists(grant.id,"missing-file") != Ok(false) { return; }
            if gamepkg::save_directory(grant.id,"",false) != Ok(1) { return; }
            if gamepkg::save_directory(grant.id,"scratch",false) != Ok(0) { return; }
            if gamepkg::save_directory(grant.id,"scratch",true) != Ok(2) { return; }
            if gamepkg::save_directory(grant.id,"scratch",true) != Ok(1) { return; }
            if gamepkg::directory_list(grant.id,"scratch").is_err() { return; }
            if gamepkg::directory_remove(grant.id,"scratch") != Ok(()) { return; }
            let file = match gamepkg::file_open(grant.id,true,"save.tmp") { Ok(v) => v, Err(_) => return };
            if gamepkg::file_exists(grant.id,"save.tmp") != Ok(true) { return; }
            if gamepkg::file_write(file,0,b"new-save") != Ok(()) { return; }
            if gamepkg::save_file_set_attributes(file,0x24) != Ok(()) { return; }
            let detail = match gamepkg::save_file_info(file) { Ok(v) => v, Err(_) => return };
            if detail.attributes != 0x24 || detail.length != 8 || detail.times[0] == 0 { return; }
            let retained = match gamepkg::file_duplicate(file) { Ok(v) => v, Err(_) => return };
            if gamepkg::file_close(file) != Ok(()) { return; }
            let metadata = match gamepkg::file_metadata(retained) { Ok(v) => v, Err(_) => return };
            if metadata.length != 8 || metadata.modified_nanoseconds >= 1000000000 { return; }
            // Flush the retained opened identity after closing the title handle.
            if gamepkg::file_sync(retained) != Ok(()) { return; }
            if gamepkg::file_replace(grant.id,"save.tmp","save.dat",true) != Ok(()) { return; }
            if gamepkg::file_exists(grant.id,"save.tmp") != Ok(false) { return; }
            if gamepkg::file_exists(grant.id,"save.dat") != Ok(true) { return; }
            let mut contents=[0u8;8];
            if gamepkg::file_read(retained,0,&mut contents) != Ok(8) || contents != *b"new-save" { return; }
            let committed_detail = match gamepkg::save_file_info(retained) { Ok(v) => v, Err(_) => return };
            if committed_detail.attributes != 0x24 || committed_detail.times[0] != detail.times[0] { return; }
            if gamepkg::file_close(retained) != Ok(()) { return; }
            if gamepkg::file_duplicate(retained) != Err(-4) { return; }
            if gamepkg::save_volume(grant.id) != Ok([1024,1016]) { return; }
        }
    }
    if !read_game_data { return; }
    let mailbox=match gamepkg::local_mailbox_create(8,1){Ok(id)=>id,Err(_)=>return};
    if gamepkg::local_mailbox_send(mailbox,&u64::to_le_bytes(41))!=Ok(()){return;}
    let request=[u32::to_le_bytes(0).as_slice(),&u64::to_le_bytes(mailbox),&u32::to_le_bytes(0)].concat();
    let mut task=[0u8;8];
    if gamepkg::local_call(10,&request,&mut task)!=Ok(8){return;}
    let mut result=[0u8;8];
    loop {
        match gamepkg::local_call(11,&task,&mut result) {
            Ok(8)=>break,
            Err(-2)=>{},
            _=>return,
        }
    }
    if u64::from_le_bytes(result)!=42 {return;}
    if gamepkg::local_mailbox_receive(mailbox,0,&mut result)!=Ok(8){return;}
    if u64::from_le_bytes(result)!=42 {return;}
    if gamepkg::local_mailbox_release(mailbox)!=Ok(()){return;}
    let _ = gamepkg::audio(&[123i16,456i16],2);
}
"#.to_vec());
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    let media = root.path().join("selected-media");
    std::fs::create_dir_all(media.join("data")).unwrap();
    std::fs::write(media.join("data/fixture"), b"selected").unwrap();
    let rom_path = root.path().join("disc.iso");
    let mut rom = std::fs::File::create(&rom_path).unwrap();
    use std::io::{Seek, SeekFrom, Write};
    rom.seek(SeekFrom::Start(4294967296)).unwrap();
    rom.write_all(&[73]).unwrap();
    drop(rom);
    for target in [Target::Linux64, Target::Linux32] {
        let plan = Plan::new(
            target,
            "game_main",
            vec!["service_task".into()],
            LocalLayout::None,
            true,
        )
        .unwrap();
        let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![]).unwrap();
        let save = SaveGrant::open(root.path(), sources.manifest(), 1024).unwrap();
        assert!(SaveGrant::open(root.path(), sources.manifest(), 1024).is_err());
        let pcm = Arc::new(Mutex::new(Vec::new()));
        let mut owner = Owner::start(
            &artifact,
            Output(pcm.clone()),
            Grants {
                volatile_cache_bytes: Some(1024 * 1024),
                roms: vec![std::fs::File::open(&rom_path).unwrap()],
                game_data: vec![cap_std::fs::Dir::open_ambient_dir(
                    &media,
                    cap_std::ambient_authority(),
                )
                .unwrap()],
                saves: Some(save),
                ..Grants::default()
            },
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            owner.dispatch(16);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(std::time::Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(*pcm.lock().unwrap(), [123, 456]);
        let before = std::time::Instant::now();
        owner.stop().unwrap();
        assert!(before.elapsed() < std::time::Duration::from_millis(100));
        drop(owner);
    }
    assert_eq!(std::fs::metadata(rom_path).unwrap().len(), 4294967297);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "executes four-target multi-object audit/link and actual confined Linux children"]
fn multiple_codegen_objects_are_all_audited_linked_and_bound_in_receipts() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Frame, Result, WindowCommand,
    };
    use std::sync::{Arc, Mutex};
    struct Silent(Arc<Mutex<Vec<i16>>>);
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn audio(&mut self, data: &[i16], _: u32) -> Result<()> {
            self.0.lock().unwrap().extend(data);
            Ok(())
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.permissions.audio = true;
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    let mut entry = String::new();
    for part in 0..8 {
        entry.push_str(&format!("mod part_{part};\n"));
        let mut body = String::from("pub fn calculate(input:u64)->u64{let mut value=input;\n");
        for step in 0..48 {
            body.push_str(&format!(
                "value=value.wrapping_add({}u64).rotate_left({});\n",
                part * 100 + step,
                step % 63 + 1
            ));
        }
        body.push_str("value}\n");
        files.insert(format!("source/src/part_{part}.rs"), body.into_bytes());
    }
    entry.push_str("pub fn game_main(){let seed=match gamepkg::version(){Ok(value)=>u64::from(value),Err(_)=>0};let mut total=0u64;\n");
    let mut expected = 0u64;
    for part in 0..8 {
        entry.push_str(&format!("total^=part_{part}::calculate(seed);\n"));
        let mut value = 1u64;
        for step in 0..48 {
            value = value
                .wrapping_add(part * 100 + step)
                .rotate_left((step % 63 + 1) as u32);
        }
        expected ^= value;
    }
    entry.push_str("let _=gamepkg::audio(&[total as i16],1);}\n");
    files.insert("source/src/lib.rs".into(), entry.into_bytes());
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true).unwrap();
        let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![]).unwrap();
        let receipt = artifact.receipt();
        assert!(
            (2..=16).contains(&receipt.game.objects.len()),
            "{target:?}: no independently emitted units"
        );
        assert!(receipt
            .arguments
            .iter()
            .any(|arg| arg == "codegen-units=64"));
        assert_eq!(receipt.game.source_kit, sources.identity());
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.plan, plan);
        assert_eq!(receipt.raw_objects.len(), receipt.game.objects.len());
        if matches!(target, Target::Windows64 | Target::Windows32) {
            assert_eq!(
                receipt.object_normalization,
                preparation::ObjectNormalization::RustMingwSymbolExclusionsV1
            );
            assert_ne!(receipt.raw_objects, receipt.game.objects);
        } else {
            assert_eq!(
                receipt.object_normalization,
                preparation::ObjectNormalization::None
            );
            assert_eq!(receipt.raw_objects, receipt.game.objects);
        }

        for (index, object) in receipt.game.objects.iter().enumerate() {
            assert_eq!(object.name, format!("game-{index:04}.o"));
            assert_eq!(object.sha256.len(), 64);
        }
        assert_eq!(
            receipt.binary,
            gamepkg::verify::digest(&std::fs::read(artifact.executable().unwrap()).unwrap())
        );
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut owner = Owner::start(
            &artifact,
            Silent(output.clone()),
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            owner.dispatch(32);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(std::time::Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            *output.lock().unwrap(),
            [expected as i16],
            "{}",
            owner.diagnostics()
        );
        owner.stop().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "builds four audited targets and runs actual confined Linux32/64 children"]
fn segmented_memory_bindings_cross_aliases_without_partial_denied_writes() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Frame, Result, WindowCommand,
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert("source/src/lib.rs".into(), br#"
fn check_memory() -> Result<(),i32> {
    let mut windows=[0u8;16];
    if gamepkg::local_call(1,&[],&mut windows)?!=16{return Err(-3);}
    let window=u64::from_le_bytes([windows[0],windows[1],windows[2],windows[3],windows[4],windows[5],windows[6],windows[7]]);
    let mut commit=std::vec::Vec::new();
    commit.extend(u64::to_le_bytes(window));commit.extend(u64::to_le_bytes(0));
    commit.extend(u64::to_le_bytes(8192));commit.extend(u32::to_le_bytes(2));
    gamepkg::local_call(2,&commit,&mut [])?;
    let spans=[[window,8190,2],[window,0,2]];
    gamepkg::local_memory_write_segments(&spans,&[1,2,3,4])?;
    let mut output=[0u8;4];
    gamepkg::local_memory_read_segments(&spans,&mut output)?;
    if output!=[1,2,3,4]{return Err(-3);}
    let mut protect=std::vec::Vec::new();
    protect.extend(u64::to_le_bytes(window));protect.extend(u64::to_le_bytes(4096));
    protect.extend(u64::to_le_bytes(4096));protect.extend(u32::to_le_bytes(1));
    gamepkg::local_call(3,&protect,&mut [])?;
    let denied=[[window,0,2],[window,8190,2]];
    if gamepkg::local_memory_write_segments(&denied,&[9,9,9,9])!=Err(-4){return Err(-3);}
    gamepkg::local_memory_read_segments(&spans,&mut output)?;
    if output!=[1,2,3,4]{return Err(-3);}
    let invalid=[[window,0,2],[0u64,0,2]];
    if gamepkg::local_memory_write_segments(&invalid,&[9,9,9,9])!=Err(-4){return Err(-3);}
    output=[0x5a;4];
    if gamepkg::local_memory_read_segments(&invalid,&mut output)!=Err(-4){return Err(-3);}
    if output!=[0x5a;4]{return Err(-3);}
    if gamepkg::local_memory_read_segments(&[],&mut [])!=Err(-1){return Err(-3);}
    if gamepkg::local_memory_write_segments(&spans,&[1,2,3])!=Err(-1){return Err(-3);}
    gamepkg::local_memory_read_segments(&spans,&mut output)?;
    if output!=[1,2,3,4]{return Err(-3);}
    Ok(())
}
pub fn game_main(){let status=match check_memory(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x5345474d,&[status]);}
"#.to_vec());
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(2)
            .unwrap();
        let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert!(!artifact.receipt().game.objects.is_empty());
        if matches!(target, Target::Windows32 | Target::Windows64) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            owner.dispatch(32);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(std::time::Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (events, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        assert_eq!(events.len(), 1, "{}", owner.diagnostics());
        assert_eq!(events[0].code, 0x5345474d);
        assert_eq!(events[0].values, vec![0]);
        owner.stop().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "builds four audited targets and exercises three opaque memory banks in confined Linux children"]
fn takedawn_memory_banks_are_distinct_bounded_and_nonexecutable() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Frame, Result, WindowCommand,
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_takedawn_memory.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![],
            LocalLayout::TakedawnMemory,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().plan, plan);
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        println!("PASS {target:?}: separate-bank fixture source checked before bindings; every optimized game object audited before trusted linking");
        if matches!(target, Target::Windows32 | Target::Windows64) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut events = Vec::new();
        loop {
            owner.dispatch(32);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(std::time::Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1, "{}", owner.diagnostics());
        assert_eq!(events[0].code, 0x544b0700);
        assert_eq!(events[0].values, vec![0]);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined main/contiguous/flash bank isolation, protection, copy and late-failure atomicity; silent/synthetic");
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "executes four-target clock-client admission and confined Linux children"]
fn host_clock_samples_preserve_raw_units_and_anchored_utc_in_confined_games() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Frame, Result, WindowCommand,
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(gamepkg::port_host::Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert("source/src/lib.rs".into(),br#"
fn check()->Result<(),i32>{
    gamepkg::diagnostic(1,0x434c4f00,&[])?;
    if gamepkg::call(65,&[1u8],&mut [0u8;32])!=Err(-1) || gamepkg::call(65,&[],&mut [0u8;31])!=Err(-1) || gamepkg::call(66,&[1u8],&mut [0u8;24])!=Err(-1) || gamepkg::call(66,&[],&mut [0u8;23])!=Err(-1){return Err(-3);}
    let mut previous=0u64;
    for _ in 0..16u32 {
        let raw=gamepkg::performance_sample()?;if raw.counter<previous{return Err(-3);}previous=raw.counter;
        gamepkg::diagnostic(1,0x434c4f02,&[raw.counter,raw.frequency,raw.uptime_ms])?;
        let sample=gamepkg::clock_sample()?;if sample.counter<previous || sample.frequency!=raw.frequency{return Err(-3);}previous=sample.counter;
        gamepkg::diagnostic(1,0x434c4f01,&[sample.counter,sample.frequency,sample.uptime_ms,sample.utc_100ns])?;
    }
    Ok(())
}
pub fn game_main(){let status=match check(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x434c4fff,&[status]);}
"#.to_vec());
    let package = helper::sign(&files, root.path());
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true)
            .unwrap()
            .with_codegen_units(2)
            .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        // Initial game diagnostic blocks on owner dispatch, so this establishes
        // the exact calendar origin before any child's worker-only clock read.
        let before = owner.host.timing().sample().unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                owner.diagnostics()
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 34, "{}", owner.diagnostics());
        assert_eq!(events[0].code, 0x434c4f00);
        assert_eq!(events[33].code, 0x434c4fff);
        assert_eq!(events[33].values, vec![0]);
        let after = owner.host.timing().sample().unwrap();
        let mut previous = before.counter;
        for (index, event) in events[1..33].iter().enumerate() {
            if index % 2 == 0 {
                assert_eq!(event.code, 0x434c4f02);
                let v = &event.values;
                assert_eq!(v.len(), 3);
                assert!(v[0] >= previous && v[0] <= after.counter);
                previous = v[0];
                assert_eq!(v[1], before.frequency);
                assert!(v[2] >= before.uptime_ms && v[2] <= after.uptime_ms);
                continue;
            }
            assert_eq!(event.code, 0x434c4f01);
            let v = &event.values;
            assert_eq!(v.len(), 4);
            assert!(v[0] >= previous && v[0] <= after.counter);
            previous = v[0];
            assert_eq!(v[1], before.frequency);
            assert!(v[2] >= before.uptime_ms && v[2] <= after.uptime_ms);
            let expected = before.utc_100ns
                + u64::try_from(
                    u128::from(v[0] - before.counter) * 10_000_000 / u128::from(before.frequency),
                )
                .unwrap();
            assert_eq!(v[3], expected);
        }
        owner.stop().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "four-target scratch/CAS source/object checks and confined Linux execution"]
fn service_scratch_memory_is_private_bounded_and_atomic_without_larger_records() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Error, Frame, Result, WindowCommand,
    };
    use std::time::{Duration, Instant};
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_scratch_memory.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let sources =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![],
            LocalLayout::TakedawnServices,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &sources, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().plan, plan);
        assert_eq!(artifact.receipt().raw_objects.len(), 2);
        assert_eq!(artifact.receipt().game.objects.len(), 2);
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert_eq!(artifact.receipt().game.source_kit, sources.identity());
        if let Some(path) = std::env::var_os("GAMEPKG_SCRATCH_EVIDENCE") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(
                path.join(format!("receipt-{target:?}.json")),
                serde_json::to_vec_pretty(artifact.receipt()).unwrap(),
            )
            .unwrap();
        }
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        let mut events = vec![];
        loop {
            owner.dispatch(16);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 2, "{events:?}");
        assert_eq!(events[0].code, 0x53435241);
        assert_eq!(events[0].values, [4, 1048576, 12312, 65536]);
        assert_eq!(events[1].code, 0x534352ff);
        assert_eq!(events[1].values, [0]);
        owner.stop().unwrap();
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "four-target source/object admission and actual confined Linux semaphore workers"]
fn local_semaphores_execute_without_owner_dispatch_and_keep_every_object_audited() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Error, Frame, Result, WindowCommand,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_local_semaphore.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    // A valid signature cannot package the trusted implementation or a test
    // provider, even when its source is unused by the checked game entry.
    for forbidden in [
        include_bytes!("../engines/native-services/local_runtime.rs").as_slice(),
        include_bytes!("../engines/native-services/local_bindings.rs").as_slice(),
        include_bytes!("local_semaphore_client.rs").as_slice(),
    ] {
        files.insert("source/src/unused.rs".into(), forbidden.to_vec());
        let denied = helper::sign(&files, root.path());
        assert!(
            CheckedSources::verify(&mut gamepkg::archive::Package::open(&denied).unwrap()).is_err()
        );
        files.remove("source/src/unused.rs");
    }
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![
                "semaphore_release_worker".into(),
                "semaphore_take_worker".into(),
                "semaphore_cancel_worker".into(),
            ],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(8)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.game.source_kit, checked.identity());
        assert_eq!(receipt.game.objects.len(), receipt.raw_objects.len());
        assert!(!receipt.game.objects.is_empty());
        // rustc may omit empty partitions. Every emitted partition must still
        // be audited, including a genuinely split game compilation.
        assert!(receipt.game.objects.len() > 1);
        artifact.executable().unwrap();
        if let Some(path) = std::env::var_os("GAMEPKG_SERVICE_SEMAPHORE_EVIDENCE") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(
                path.join(format!("receipt-{target:?}.json")),
                serde_json::to_vec_pretty(receipt).unwrap(),
            )
            .unwrap();
        }

        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let queued = Arc::new(AtomicBool::new(false));
        let signal = queued.clone();
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            move || {
                signal.store(true, Ordering::Release);
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        // The first and only remote request is the final report. Do not service
        // it until all local semaphore operations, worker wake/join and negative calls ran.
        while !queued.load(Ordering::Acquire) {
            assert!(owner.poll().unwrap().is_none(), "{}", owner.diagnostics());
            assert!(
                Instant::now() < end,
                "local semaphore waited for owner dispatch"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 0x53454dff);
        assert_eq!(events[0].values, [0]);
        owner.stop().unwrap();
    }
    // Infrastructure qualification must not admit an unmigrated engine.
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "four-target source/object admission and actual confined Linux mixed-object wait workers"]
fn local_wait_sets_are_atomic_and_keep_every_object_audited() {
    use gamepkg::port_host::{
        owner::{Grants, Owner},
        Backend, Error, Frame, Result, WindowCommand,
    };
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };
    struct Silent;
    impl Backend for Silent {
        fn window(&mut self, _: WindowCommand) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn present(&mut self, _: Frame) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
        fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
            Err(Error::Unsupported)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut files = helper::fixture("examples/minimal");
    let mut manifest = gamepkg::manifest::Manifest::parse(&files["manifest.toml"]).unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    files.insert(
        "manifest.toml".into(),
        toml::to_string(&manifest).unwrap().into_bytes(),
    );
    files.insert(
        "source/src/lib.rs".into(),
        include_bytes!("fixtures/service_local_waitset.rs").to_vec(),
    );
    let package = helper::sign(&files, root.path());
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    // A valid signature cannot package the trusted implementation or a test
    // provider, even when its source is unused by the checked game entry.
    for forbidden in [
        include_bytes!("../engines/native-services/local_runtime.rs").as_slice(),
        include_bytes!("../engines/native-services/local_bindings.rs").as_slice(),
        include_bytes!("local_waitset_client.rs").as_slice(),
        include_bytes!("local_mutex_client.rs").as_slice(),
        include_bytes!("local_mutex_state.rs").as_slice(),
        include_bytes!("fixtures/local_mutex_ffi.rs").as_slice(),
    ] {
        files.insert("source/src/unused.rs".into(), forbidden.to_vec());
        let denied = helper::sign(&files, root.path());
        assert!(
            CheckedSources::verify(&mut gamepkg::archive::Package::open(&denied).unwrap()).is_err()
        );
        files.remove("source/src/unused.rs");
    }
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![
                "waitset_release_worker".into(),
                "waitset_abandon_worker".into(),
                "waitset_cancel_worker".into(),
            ],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(8)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        let receipt = artifact.receipt();
        assert_eq!(receipt.game.sdk, gamepkg::port_host::build_identity());
        assert_eq!(receipt.game.source_kit, checked.identity());
        assert_eq!(receipt.game.objects.len(), receipt.raw_objects.len());
        assert!(!receipt.game.objects.is_empty());
        // rustc may omit empty partitions. Every emitted partition must still
        // be audited, including a genuinely split game compilation.
        assert!(receipt.game.objects.len() > 1);
        artifact.executable().unwrap();
        if let Some(path) = std::env::var_os("GAMEPKG_SERVICE_WAITSET_EVIDENCE") {
            let path = std::path::Path::new(&path);
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(
                path.join(format!("receipt-{target:?}.json")),
                serde_json::to_vec_pretty(receipt).unwrap(),
            )
            .unwrap();
        }

        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let queued = Arc::new(AtomicBool::new(false));
        let signal = queued.clone();
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            move || {
                signal.store(true, Ordering::Release);
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        // The first and only remote request is the final report. Do not service
        // it until all local mixed-object wait operations, worker wake/join and negative calls ran.
        while !queued.load(Ordering::Acquire) {
            assert!(owner.poll().unwrap().is_none(), "{}", owner.diagnostics());
            assert!(
                Instant::now() < end,
                "local wait set waited for owner dispatch"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 0x575354ff);
        assert_eq!(events[0].values, [0]);
        owner.stop().unwrap();
    }
    // Infrastructure qualification must not admit an unmigrated engine.
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
