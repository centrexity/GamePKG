use gamepkg::settings::Settings;
#[test]
fn settings_persist_and_failed_saves_preserve_previous_values() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config/settings.toml");
    let mut settings = Settings::read(&path).unwrap();
    settings.dxc_directory = temp.path().join("dxc").to_string_lossy().into_owned();
    settings.library_directory = temp.path().join("library").to_string_lossy().into_owned();
    settings.tor_proxy = "127.0.0.1:9150".into();
    settings.memory_ceiling_mib = 64;
    settings.fuel_ceiling = 10000;
    settings.tick_deadline_ms = 50;
    settings.approved_crates.clear();
    settings.optimize_builds = false;
    settings.dark_mode = false;
    settings.mute_audio = true;
    settings.audio_volume = 20;
    settings.automatic_update_checks = false;
    settings.update_interval_hours = 12;
    settings.save(&path).unwrap();
    assert_eq!(Settings::read(&path).unwrap(), settings);
    let before = std::fs::read(&path).unwrap();
    settings.tor_proxy = "8.8.8.8:9050".into();
    assert!(settings.save(&path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn settings_validate_bounds_and_reject_unknown_or_large_files() {
    let defaults = Settings::default();
    for hours in [0, 169] {
        let s = Settings {
            update_interval_hours: hours,
            ..Default::default()
        };
        assert!(s.validate().is_err());
    }
    defaults.validate().unwrap();
    for approved in [
        vec!["unknown".into()],
        vec!["byteorder-lite".into(), "byteorder-lite".into()],
    ] {
        let mut s = defaults.clone();
        s.approved_crates = approved;
        assert!(s.validate().is_err());
    }
    for memory in [0, 513] {
        let mut s = defaults.clone();
        s.memory_ceiling_mib = memory;
        assert!(s.validate().is_err());
    }
    for fuel in [0, 100000001] {
        let mut s = defaults.clone();
        s.fuel_ceiling = fuel;
        assert!(s.validate().is_err());
    }
    for deadline in [0, 15, 110] {
        let mut s = defaults.clone();
        s.tick_deadline_ms = deadline;
        assert!(s.validate().is_err());
    }
    let mut s = defaults.clone();
    s.library_directory = "relative".into();
    assert!(s.validate().is_err());
    for proxy in ["example.com:9050", "127.0.0.1:0", "192.168.1.2:9050"] {
        assert!(gamepkg::settings::validate_proxy(proxy).is_err());
    }
    for directory in ["relative", "/tmp/dxc\ninvalid"] {
        let mut s = defaults.clone();
        s.dxc_directory = directory.into();
        assert!(s.validate().is_err());
    }
    let mut s = defaults.clone();
    s.audio_volume = 101;
    assert!(s.validate().is_err());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    std::fs::write(&path, "unknown=true").unwrap();
    assert!(Settings::read(&path).is_err());
    std::fs::write(&path, vec![b'x'; 65537]).unwrap();
    assert!(Settings::read(&path).is_err());
}
#[test]
fn saved_library_and_environment_override_are_used_by_cli() {
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("settings.toml");
    let library = temp.path().join("library");
    let settings = Settings {
        library_directory: library.to_string_lossy().into_owned(),
        ..Default::default()
    };
    settings.save(&config).unwrap();
    let cli = env!("CARGO_BIN_EXE_gamepkg");
    let list = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/lists/starter.gamepkglist"
    );
    let output = Command::new(cli)
        .env_remove("GAMEPKG_HOME")
        .env("GAMEPKG_CONFIG", &config)
        .args(["import-list", list])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(library.join("catalog.gamepkglist").exists());
    let other = temp.path().join("override");
    let output = Command::new(cli)
        .env("GAMEPKG_HOME", &other)
        .env("GAMEPKG_CONFIG", &config)
        .args(["import-list", list])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(other.join("catalog.gamepkglist").exists());
}
