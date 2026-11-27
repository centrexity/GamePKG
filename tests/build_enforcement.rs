#[allow(dead_code)]
#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::{build_policy, port_host::audit};

#[test]
fn signatures_platform_or_profile_names_cannot_opt_out_of_exclusive_services() {
    let mut m =
        gamepkg::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
            .unwrap();
    assert!(build_policy::require(None).is_ok());
    for profile in [
        "port-owned-engine",
        "native-port",
        "unmigrated-engine",
        "unknown",
        "",
        "gamepkg-native-services",
        "GAMEPKG-SAFE",
    ] {
        assert!(build_policy::require(Some(profile))
            .unwrap_err()
            .to_string()
            .contains("requires migration"));
        m.builtin_engine = Some(profile.into());
        assert!(!gamepkg::native::build_supported(&m));
        assert!(!gamepkg::native::play_supported(&m));
    }
}

#[test]
fn game_definitions_cannot_launder_system_calls_or_replace_host_runtime_symbols() {
    for symbol in [
        "SDL_PollEvent",
        "_SDL_CreateWindow",
        "vkCreateInstance",
        "glDrawElements",
        "socket",
        "fopen",
        "CreateFileW",
        "gamepkg_port_call",
        "gamepkg_local_call",
        "clock_gettime",
        "_gamepkg_port_call",
        "gamepkg_input_connect",
        "malloc",
        "_memcpy",
        "__imp_CreateFileW",
        "_socket@12",
        "@SDL_PollEvent@4",
        "socket@GLIBC_2.2.5",
    ] {
        assert!(
            audit::check_symbols([], [symbol]).is_err(),
            "defined {symbol}"
        );
        assert!(
            audit::check_symbols([symbol], [symbol]).is_err(),
            "laundered {symbol}"
        );
    }
    audit::check_symbols(
        ["game_local", "gamepkg_port_call", "memcpy"],
        ["game_local"],
    )
    .unwrap();
    for symbol in [
        "clock_gettime",
        "QueryPerformanceCounter",
        "QueryPerformanceFrequency",
        "GetTickCount64",
        "__imp_QueryPerformanceCounter",
        "_QueryPerformanceCounter@4",
        "__imp_socket",
        "_socket@12",
        "@SDL_PollEvent@4",
        "_ZN3std2fs4read",
        "dlsym",
        "GetProcAddress",
        "dlopen",
    ] {
        assert!(audit::check_symbols([symbol], []).is_err());
    }
}

#[cfg(target_os = "linux")]
#[test]
fn signed_legacy_package_and_valid_cached_executable_cannot_reach_build_play_or_export() {
    let temp = tempfile::tempdir().unwrap();
    let library = temp.path().join("library");
    let mut legacy = helper::fixture("examples/minimal");
    let manifest = format!(
        "builtin_engine=\"port-owned-engine\"\n{}",
        String::from_utf8(legacy["manifest.toml"].clone()).unwrap()
    );
    legacy.insert("manifest.toml".into(), manifest.into_bytes());
    let signed = helper::sign(&legacy, temp.path());
    let manifest = gamepkg::registry::install(&library, &signed, true).unwrap();
    let path = gamepkg::native::cache_path(&library, &manifest.id).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let exe = std::fs::read("/bin/true").unwrap();
    std::fs::write(&path, &exe).unwrap();
    gamepkg::integrity::record_build(&path, &exe).unwrap();
    assert!(!gamepkg::native::is_built(&library, &manifest.id).unwrap());
    assert!(!gamepkg::build::is_built(&library, &manifest.id).unwrap());
    let error = gamepkg::native::compile(&library, &manifest.id).unwrap_err();
    assert!(
        error.to_string().contains("requires migration"),
        "{error:#}"
    );
    let error = gamepkg::build::compile(&library, &manifest.id).unwrap_err();
    assert!(error.to_string().contains("requires migration"));
    assert!(gamepkg::runtime::Game::load(&library, &manifest.id)
        .err()
        .unwrap()
        .to_string()
        .contains("requires migration"));
    assert!(
        gamepkg::web_export::export(&library, &manifest.id, &temp.path().join("web"))
            .unwrap_err()
            .to_string()
            .contains("requires migration")
    );
    assert!(!temp.path().join("web").exists());
    let request = temp.path().join("request.gamepkgstore");
    assert!(
        gamepkg::build_worker::prepare(&library, &manifest.id, &request)
            .unwrap_err()
            .to_string()
            .contains("requires migration")
    );
    assert!(!request.exists());
    assert!(gamepkg::build_worker::accept(
        &library,
        &manifest.id,
        &temp.path().join("old-exchange")
    )
    .unwrap_err()
    .to_string()
    .contains("requires migration"));
    let store = temp.path().join("source.gamepkgstore");
    gamepkg::store::export(&library, &[signed], &store).unwrap();
    for transport in [false, true] {
        let out = temp.path().join(if transport {
            "remote-build"
        } else {
            "worker-build"
        });
        let result = if transport {
            gamepkg::build_worker::compile_transport(&store, &out, "wgsl", true)
        } else {
            gamepkg::build_worker::compile(&store, &out, "wgsl", true)
        };
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires migration"));
        assert!(!out.exists());
    }
    assert_eq!(
        std::fs::read(path).unwrap(),
        exe,
        "rejected build must not overwrite a cache"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn audit_rejects_nonobjects_reserved_definitions_and_incomplete_tool_output() {
    use std::{
        os::unix::fs::{symlink, PermissionsExt},
        process::Command,
    };
    let temp = tempfile::tempdir().unwrap();
    let nm = std::path::Path::new("/usr/bin/llvm-nm-18");
    for bytes in [
        b"!<arch>\n".as_slice(),
        b"BC\xc0\xde",
        b"MZ",
        b"\x7fELF\x02\x01",
    ] {
        let file = temp.path().join("fake.o");
        std::fs::write(&file, bytes).unwrap();
        assert!(audit::objects(nm, &[file]).is_err());
    }
    let c = temp.path().join("game.c");
    let object = temp.path().join("game.o");
    for text in [
        "int SDL_PollEvent(void){return 0;}",
        "int gamepkg_port_call(void){return 0;}",
        "int socket(void){return 0;}",
        "extern int clock_gettime(int, void*); int game_local(void){char out[32];return clock_gettime(1,out);}",
    ] {
        std::fs::write(&c, text).unwrap();
        assert!(Command::new("cc")
            .arg("-c")
            .arg(&c)
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap()
            .success());
        assert!(audit::objects(nm, std::slice::from_ref(&object)).is_err());
    }
    std::fs::write(&c, "int game_local(void){return 0;}").unwrap();
    assert!(Command::new("cc")
        .arg("-c")
        .arg(&c)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    audit::objects(nm, std::slice::from_ref(&object)).unwrap();
    std::fs::write(
        &c,
        "__attribute__((constructor)) static void startup(void){} int game_local(void){return 0;}",
    )
    .unwrap();
    assert!(Command::new("cc")
        .arg("-c")
        .arg(&c)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    assert!(audit::objects(nm, std::slice::from_ref(&object))
        .unwrap_err()
        .to_string()
        .contains("startup"));
    std::fs::write(&c, "int game_local(void){return 0;}").unwrap();
    assert!(Command::new("cc")
        .arg("-flto")
        .arg("-c")
        .arg(&c)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    assert!(audit::objects(nm, std::slice::from_ref(&object)).is_err());
    assert!(Command::new("cc")
        .arg("-c")
        .arg(&c)
        .arg("-o")
        .arg(&object)
        .status()
        .unwrap()
        .success());
    let alias = temp.path().join("alias.o");
    symlink(&object, &alias).unwrap();
    assert!(audit::objects(nm, &[alias]).is_err());
    let fake = temp.path().join("fake-nm");
    std::fs::write(&fake, "#!/bin/sh\nyes game_local | head -c 1100000\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(audit::objects(&fake, &[object])
        .unwrap_err()
        .to_string()
        .contains("incomplete"));
}

#[cfg(target_os = "linux")]
#[test]
fn cross_windows_objects_cannot_smuggle_default_libraries_or_startup_sections() {
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("game.c");
    let object = root.path().join("game.obj");
    let nm = std::path::Path::new("/usr/bin/llvm-nm-18");
    for target in [
        "i686-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ] {
        std::fs::write(&source, "int game_local(void){return 0;}").unwrap();
        assert!(Command::new("clang-18")
            .arg(format!("--target={target}"))
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap()
            .success());
        audit::objects(nm, std::slice::from_ref(&object)).unwrap();
        for text in ["__declspec(dllimport) int __stdcall QueryPerformanceCounter(long long*); int game_local(void){long long c;return QueryPerformanceCounter(&c);}","#pragma comment(lib,\"SDL2\")\nint game_local(void){return 0;}","#pragma section(\".CRT$XCU\",read)\nvoid start(void){} __declspec(allocate(\".CRT$XCU\")) void (*init)(void)=start;"] {
            std::fs::write(&source,text).unwrap();assert!(Command::new("clang-18").arg(format!("--target={target}")).arg("-c").arg(&source).arg("-o").arg(&object).status().unwrap().success());assert!(audit::objects(nm,std::slice::from_ref(&object)).is_err(),"{target}: {text}");
        }
    }
}
