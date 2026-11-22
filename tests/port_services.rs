//! Real C caller -> dependency-free Rust client -> private pipes -> host services.
#![cfg(unix)]
use gamepkg::port_host::{serve, Backend, Result, Session, WindowCommand, WindowState};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    process::{Command, Stdio},
};
#[path = "../engines/native-services/client.rs"]
#[allow(dead_code)]
mod local_client;

#[derive(Default)]
struct Output {
    pcm: Vec<i16>,
    window: Vec<WindowCommand>,
    frames: Vec<gamepkg::port_host::Frame>,
}
impl Backend for Output {
    fn window(&mut self, command: WindowCommand) -> Result<()> {
        self.window.push(command);
        Ok(())
    }
    fn present(&mut self, frame: gamepkg::port_host::Frame) -> Result<()> {
        self.frames.push(frame);
        Ok(())
    }
    fn audio(&mut self, s: &[i16], _: u32) -> Result<()> {
        self.pcm.extend(s);
        Ok(())
    }
    fn audio_streams(&self) -> Option<gamepkg::port_host::audio_streams::Mixer> {
        use gamepkg::port_host::audio_streams::{Layout, Mixer};
        Some(Mixer::new(Layout::Stereo, 65536))
    }
    fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
        Ok(())
    }
}

#[test]
fn compatibility_layer_dispatches_inside_the_game_process() {
    let _exclusive = local_client::TEST_BINDING_LOCK.lock().unwrap();
    let mut permissions =
        gamepkg::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
            .unwrap()
            .permissions;
    permissions.audio = true;
    permissions.input = true;
    permissions.network = false;
    let mut session = Session::new(permissions, Output::default());
    session.observe_window(WindowState {
        focused: true,
        ..WindowState::default()
    });
    let keyboard = session.input_backend().connect(1, 1, 123).unwrap();
    assert!(session.input_backend().submit(gamepkg_native_input::Event {
        device: keyboard,
        kind: 1,
        code: 4,
        ..Default::default()
    }));
    let mut rom = tempfile::tempfile().unwrap();
    rom.write_all(b"disc").unwrap();
    let rom_id = session.grant_rom(rom).unwrap();
    let (connection, pending) =
        gamepkg::port_host::broker::channel_with_services(session.worker_services(), || {});
    local_client::connect_local(move |op, request, capacity| {
        connection
            .call(op, request, capacity)
            .map_err(|error| error as i32)
    })
    .unwrap();
    assert!(local_client::connect_local(|_, _, _| unreachable!()).is_err());

    let worker = std::thread::spawn(move || {
        let mut bytes = [0; 4];
        assert_eq!(local_client::file_read(rom_id, 0, &mut bytes), Ok(4));
        assert_eq!(&bytes, b"disc");
        assert_eq!(local_client::file_write(rom_id, 0, b"x"), Err(-4));
        assert_eq!(local_client::fullscreen(true), Ok(()));
        assert_eq!(local_client::present(0x102030ff, &[]), Ok(()));
        assert_eq!(local_client::audio(&[17, -17], 2), Ok(()));
        assert_eq!(
            local_client::audio_stream_create_with_rate(96_000, 1, 2),
            Err(-5)
        );
        let stream = local_client::audio_stream_create_with_rate(32000, 1, 2).unwrap();
        local_client::audio_stream_submit(stream, &[17, -17]).unwrap();
        assert_eq!(local_client::audio_stream_status(stream), Ok([1, 0, 0, 1]));
        local_client::audio_stream_release(stream).unwrap();
        let legacy = local_client::audio_stream_create(1, 2).unwrap();
        local_client::audio_stream_release(legacy).unwrap();
        assert_eq!(local_client::key_held(keyboard, 4), Ok(true));
        assert_eq!(local_client::network(0, &[], &mut bytes), Err(-4));
        let (before, _) = local_client::monotonic().unwrap();
        let (after, _) = local_client::monotonic().unwrap();
        assert!(after >= before);
        assert_eq!(local_client::call(0, &[], &mut [0; 3]), Err(-1));
        assert_eq!(local_client::call(0, &[], &mut bytes), Ok(4));
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !worker.is_finished() {
        pending.dispatch(&mut session, 8);
        if std::time::Instant::now() >= deadline {
            drop(pending);
            let _ = worker.join();
            local_client::disconnect();
            panic!("in-process request did not complete");
        }
        std::thread::yield_now();
    }
    worker.join().unwrap();
    let output = session.backend();
    assert!(matches!(
        output.window.as_slice(),
        [WindowCommand::Fullscreen(true)]
    ));
    assert_eq!(output.frames.len(), 1);
    assert_eq!(output.pcm, [17, -17]);
    drop(session);
    assert_eq!(local_client::file_read(rom_id, 0, &mut [0; 4]), Err(-4));
    local_client::disconnect();

    local_client::connect_local(|_, _, _| Ok(vec![0; 5])).unwrap();
    let mut response = [0xa5; 4];
    assert_eq!(local_client::call(0, &[], &mut response), Err(-3));
    assert_eq!(response, [0xa5; 4]);
    assert_eq!(local_client::call(0, &[], &mut response), Err(-4));
}

#[test]
fn c_port_uses_real_shared_services_over_private_pipes_without_game_dependencies() {
    run_probe(false);
}
#[test]
#[ignore = "requires installed i686 Rust std and native 32-bit C development libraries"]
fn i686_c_and_rust_port_use_the_64_bit_hosts_wire_contract() {
    run_probe(true);
}
fn run_probe(i686: bool) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let object = temp.path().join("probe.o");
    let binary = temp.path().join("probe");
    let source = temp.path().join("bootstrap.rs");
    let mut cc = Command::new("cc");
    if i686 {
        cc.arg("-m32");
    }
    let c = cc
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Werror")
        .arg("-I")
        .arg(root.join("include"))
        .arg("-c")
        .arg(root.join("engines/native-services/probe.c"))
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(c.status.success(), "{}", String::from_utf8_lossy(&c.stderr));
    let nm = std::path::Path::new("/usr/bin/llvm-nm-18");
    gamepkg::port_host::audit::objects(nm, std::slice::from_ref(&object)).unwrap();
    let bad = temp.path().join("bad.c");
    let bad_object = temp.path().join("bad.o");
    fs::write(
        &bad,
        "#include <stdio.h>\nint forbidden(void) {return puts(\"direct platform call\");}\n",
    )
    .unwrap();
    assert!(Command::new("cc")
        .arg("-c")
        .arg(&bad)
        .arg("-o")
        .arg(&bad_object)
        .status()
        .unwrap()
        .success());
    assert!(gamepkg::port_host::audit::objects(nm, &[bad_object]).is_err());
    fs::write(&source,format!(r#"
#[path={:?}] #[allow(dead_code)] mod client;
use std::os::fd::FromRawFd;
extern "C" {{fn port_client_probe(rom:u64)->i32;}}
fn main() {{
    assert_eq!(client::call(0,&[],&mut [0;4]),Err(-4));
    client::connect(unsafe {{std::fs::File::from_raw_fd(0)}},unsafe {{std::fs::File::from_raw_fd(1)}}).unwrap();
    let mut args = std::env::args().skip(1);
    let rom = args.next().unwrap().parse().unwrap();
    let save = args.next().unwrap().parse().unwrap();
    let assets = args.next().unwrap().parse().unwrap();
    let result = unsafe {{ port_client_probe(rom) }};
    if result == 0 {{
        let start = (1u64 << 32) + 38;
        assert_eq!(client::file_rom_view(rom, u64::MAX, 1), Err(-1));
        assert_eq!(client::file_rom_view(rom, start, 4), Err(-1));
        assert_eq!(client::file_rom_view(save, 0, 0), Err(-4));
        let view = client::file_rom_view(rom, start, 2).unwrap();
        assert_eq!(client::file_size(view), Ok(2));
        assert_eq!(client::file_rom_view(view, 0, 1), Err(-4));
        let mut bytes = [0xa5; 4];
        assert_eq!(client::file_read(view, 0, &mut bytes), Ok(2));
        assert_eq!(bytes, [b'i', b's', 0xa5, 0xa5]);
        assert_eq!(client::file_read(view, u64::MAX, &mut bytes), Ok(0));
        assert_eq!(client::file_write(view, 0, b"x"), Err(-4));
        assert_eq!(client::file_resize(view, 0), Err(-4));
        let alias = client::file_duplicate(view).unwrap();
        client::file_close(view).unwrap();
        assert_eq!(client::file_read(view, 0, &mut []), Err(-4));
        assert_eq!(client::file_read(alias, 1, &mut bytes), Ok(1));
        assert_eq!(bytes[0], b's');
        assert_eq!(client::file_close(rom), Err(-4));
        client::file_close(alias).unwrap();
        assert_eq!(client::file_open_existing(save, "missing"), Err(-3));
        assert!(!client::file_exists(save, "missing").unwrap());
        assert_eq!(client::file_open_existing(assets, "known"), Err(-4));
        assert_eq!(client::file_create_new(assets, "missing"), Err(-4));
        assert_eq!(client::file_create_new(rom, "missing"), Err(-4));
        assert_eq!(client::file_create_new(save, "../outside"), Err(-1));
        let file = client::file_create_new(save, "new").unwrap();
        client::file_write(file, 0, b"data").unwrap();
        assert_eq!(client::file_create_new(save, "new"), Err(-3));
        client::file_close(file).unwrap();
        let reopened = client::file_open_existing(save, "new").unwrap();
        let mut bytes = [0; 4];
        assert_eq!(client::file_read(reopened, 0, &mut bytes), Ok(4));
        assert_eq!(&bytes, b"data");
        assert_eq!(client::file_write(reopened, 4, b"b"), Err(-2));
        client::file_write(reopened, 0, b"D").unwrap();
        client::file_close(reopened).unwrap();
    }}
    client::disconnect();
    if result!=0 {{eprintln!("C fixture failed at line {{result}}");std::process::exit(1);}}
}}
"#,root.join("engines/native-services/client.rs"))).unwrap();
    let mut rustc = Command::new("rustc");
    let mut stack_runtime = None;
    if i686 {
        rustc.args(["--target", "i686-unknown-linux-gnu"]);
        let linker = std::env::var_os("GAMEPKG_TEST_I686_LINKER").unwrap_or_else(|| "cc".into());
        {
            let mut argument = std::ffi::OsString::from("linker=");
            argument.push(&linker);
            rustc.arg("-C").arg(argument);
        }
        let runtime = Command::new(linker)
            .args(["-m32", "-print-file-name=libssp_nonshared.a"])
            .output()
            .unwrap();
        assert!(runtime.status.success());
        let path = std::path::PathBuf::from(std::str::from_utf8(&runtime.stdout).unwrap().trim());
        assert!(path.is_file(),"32-bit stack protector runtime missing; select a complete multilib compiler with GAMEPKG_TEST_I686_LINKER");
        stack_runtime = Some(path);
    }
    rustc
        .arg("--edition=2021")
        .arg(&source)
        .arg("-C")
        .arg(format!("link-arg={}", object.display()));
    if let Some(runtime) = stack_runtime {
        rustc
            .arg("-C")
            .arg(format!("link-arg={}", runtime.display()));
    }
    let rust = rustc.arg("-o").arg(&binary).output().unwrap();
    assert!(
        rust.status.success(),
        "{}",
        String::from_utf8_lossy(&rust.stderr)
    );
    let mut p = toml::from_str::<gamepkg::manifest::Manifest>(include_str!(
        "../examples/minimal/manifest.toml"
    ))
    .unwrap()
    .permissions;
    p.audio = true;
    p.network = false;
    p.save = true;
    let mut host = Session::new(p, Output::default());
    host.observe_window(WindowState {
        width: 1000,
        height: 500,
        focused: true,
        ..Default::default()
    });
    let mut rom = tempfile::tempfile().unwrap();
    rom.seek(SeekFrom::Start((1u64 << 32) + 37)).unwrap();
    rom.write_all(b"disc").unwrap();
    let id = host.grant_rom(rom).unwrap();
    let save_dir = temp.path().join("saves");
    let asset_dir = temp.path().join("assets");
    fs::create_dir(&save_dir).unwrap();
    fs::create_dir(&asset_dir).unwrap();
    fs::write(asset_dir.join("known"), b"asset").unwrap();
    let open_dir =
        |path| cap_std::fs::Dir::open_ambient_dir(path, cap_std::ambient_authority()).unwrap();
    let save = host.grant_save_quota(open_dir(&save_dir), 4).unwrap();
    let assets = host
        .grant_directory(open_dir(&asset_dir), gamepkg::port_host::Access::Assets)
        .unwrap();
    let mut child = Command::new(&binary)
        .arg(id.to_string())
        .arg(save.to_string())
        .arg(assets.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    serve(
        &mut host,
        child.stdout.take().unwrap(),
        child.stdin.take().unwrap(),
    )
    .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "native client {:?}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(host.backend().pcm, [i16::MIN, i16::MAX]);
    assert_eq!(fs::read(save_dir.join("new")).unwrap(), b"Data");
    assert!(!asset_dir.join("missing").exists());
}
