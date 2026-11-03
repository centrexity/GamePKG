use gamepkg::settings::Settings;
#[test]
fn settings_persist_and_failed_saves_preserve_previous_values() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("config/settings.toml");
    let mut settings = Settings::read(&path).unwrap();
    settings.wasi_sysroot_directory = temp.path().join("wasi").to_string_lossy().into_owned();
    settings.dxc_directory = temp.path().join("dxc").to_string_lossy().into_owned();
    settings.library_directory = temp.path().join("library").to_string_lossy().into_owned();
    settings.tor_proxy = "127.0.0.1:9150".into();
    settings.memory_ceiling_mib = 64;
    settings.tick_deadline_ms = 50;
    settings.legacy_approved_crates.clear();
    settings.optimize_builds = false;
    settings.dark_mode = false;
    settings.ui_scale_percent = 140;
    settings.mute_audio = true;
    settings.audio_volume = 20;
    settings.automatic_update_checks = false;
    settings.check_app_updates_on_startup = false;
    settings.update_interval_hours = 12;
    settings.save(&path).unwrap();
    assert_eq!(Settings::read(&path).unwrap(), settings);
    let before = std::fs::read(&path).unwrap();
    settings.tor_proxy = "8.8.8.8:9050".into();
    assert!(settings.save(&path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}
#[test]
fn app_startup_checks_default_on_for_existing_configs_and_are_independent_of_game_checks() {
    let settings: Settings = toml::from_str("automatic_update_checks = false\n").unwrap();
    assert!(settings.check_app_updates_on_startup);
    assert!(!settings.automatic_update_checks);
    let settings: Settings = toml::from_str("check_app_updates_on_startup = false\n").unwrap();
    assert!(!settings.check_app_updates_on_startup);
    assert!(settings.automatic_update_checks);
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
    for scale in [0, 79, 161, u16::MAX] {
        let settings = Settings {
            ui_scale_percent: scale,
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }
    defaults.validate().unwrap();
    assert_eq!(defaults.shader_backend, "wgsl");
    assert_eq!(defaults.renderer, "vulkan");
    for memory in [0, 4097, u64::MAX] {
        let mut s = defaults.clone();
        s.memory_ceiling_mib = memory;
        assert!(s.validate().is_err());
    }
    assert_eq!(defaults.memory_ceiling_mib, 512);
    for memory in [512, 1024, 2048, 4096] {
        let mut settings = defaults.clone();
        settings.memory_ceiling_mib = memory;
        settings.validate().unwrap();
    }
    assert_eq!(defaults.tick_deadline_ms, 100);
    for deadline in [10, 100, 500, 1000] {
        let mut settings = defaults.clone();
        settings.tick_deadline_ms = deadline;
        settings.validate().unwrap();
    }
    for deadline in [0, 15, 1010] {
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
    for directory in ["relative", "/tmp/slang\ninvalid", "/tmp/slang\0invalid"] {
        let mut s = defaults.clone();
        s.slang_directory = directory.into();
        assert!(s.validate().is_err());
    }
    for directory in ["relative", "/tmp/dxc\ninvalid"] {
        let mut invalid = defaults.clone();
        invalid.wasi_sysroot_directory = directory.into();
        assert!(invalid.validate().is_err());
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

#[test]
fn windows_compiler_configuration_is_explicit_and_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    let mut settings = Settings {
        windows_compiler_distribution: "GamePKG-Ubuntu-24.04".into(),
        ..Default::default()
    };
    assert!(settings.validate().is_err());
    settings.windows_compiler_executable = "/opt/gamepkg/bin/gamepkg".into();
    settings.save(&path).unwrap();
    assert_eq!(Settings::read(&path).unwrap(), settings);
    for invalid in ["--exec", "Ubuntu\n", "Ubuntu;other", "Ubuntu/other"] {
        let mut changed = settings.clone();
        changed.windows_compiler_distribution = invalid.into();
        assert!(changed.save(&path).is_err());
        assert_eq!(Settings::read(&path).unwrap(), settings);
    }
    for invalid in ["relative/gamepkg", "C:\\gamepkg.exe", "/bin/gamepkg\n"] {
        let mut changed = settings.clone();
        changed.windows_compiler_executable = invalid.into();
        assert!(changed.save(&path).is_err());
        assert_eq!(Settings::read(&path).unwrap(), settings);
    }
}

#[test]
fn earlier_instruction_ceiling_is_read_but_never_written_or_enforced() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    std::fs::write(&path, "fuel_ceiling = 0\n").unwrap();
    let settings = Settings::read(&path).unwrap();
    assert_eq!(settings.legacy_execution_ceiling, Some(0));
    settings.validate().unwrap();
    settings.save(&path).unwrap();
    assert!(!std::fs::read_to_string(&path).unwrap().contains("fuel"));
    assert_eq!(
        Settings::read(&path).unwrap().legacy_execution_ceiling,
        None
    );
}

#[test]
fn legacy_crate_approvals_are_ignored_and_removed_on_save() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.toml");
    std::fs::write(
        &path,
        "approved_crates=['gamepkg-input','serde','gamepkg-evil']\n",
    )
    .unwrap();
    let settings = Settings::read(&path).unwrap();
    settings.validate().unwrap();
    settings.save(&path).unwrap();
    assert!(!std::fs::read_to_string(path)
        .unwrap()
        .contains("approved_crates"));
}

#[test]
fn render_resolution_limit_is_opt_in_and_persists() {
    let default: gamepkg::settings::Settings = toml::from_str("tick_deadline_ms=500").unwrap();
    assert!(!default.render_limit_enabled);
    let mut settings = default;
    settings.render_limit_enabled = true;
    settings.render_max_width = 1280;
    settings.render_max_height = 720;
    settings.validate().unwrap();
    let saved = toml::to_string(&settings).unwrap();
    assert_eq!(
        toml::from_str::<gamepkg::settings::Settings>(&saved).unwrap(),
        settings
    );
    settings.render_max_width = 0;
    assert!(settings.validate().is_err());
}

#[test]
fn native_watchdog_is_opt_in_persists_and_rejects_invalid_delays() {
    let old: Settings = toml::from_str("format_version=1").unwrap();
    assert_eq!(old.native_watchdog_seconds, 0);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.toml");
    for seconds in [0, 5, 30, 600] {
        let settings = Settings {
            native_watchdog_seconds: seconds,
            ..old.clone()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::read(&path).unwrap(), settings);
    }
    for seconds in [1, 4, 601, u16::MAX] {
        let settings = Settings {
            native_watchdog_seconds: seconds,
            ..old.clone()
        };
        assert!(settings.validate().is_err());
    }
}
