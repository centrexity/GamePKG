//! Maintainer probe of the real native runtime through the Rust process owner.
//! Package validation and confinement belong to the app.
#[cfg(windows)]
fn main() -> std::io::Result<()> {
    use gamepkg_takedawn_windows_runner::{Event, Launch, Session};
    use std::{ffi::OsString, fs::File, path::PathBuf, time::Duration};
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 6 || (args[4] != "--headless" && args[4] != "--window") {
        return Err(std::io::Error::other(
            "usage: native_game.exe ABSOLUTE_GAME_EXE ABSOLUTE_ISO PRIVATE_SAVES LOG --headless|--window PROBE_SECONDS",
        ));
    }
    let seconds: u64 = args[5]
        .to_str()
        .and_then(|value| value.parse().ok())
        .filter(|value| (1..=300).contains(value))
        .ok_or_else(|| std::io::Error::other("probe seconds must be 1 to 300"))?;
    let began = std::time::Instant::now();
    let executable = PathBuf::from(&args[0]);
    let disc = File::open(&args[1])?;
    let saves = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&saves)?;
    let output = File::create(&args[3])?;
    let mut values: Vec<(OsString, OsString)> = Vec::new();
    // Display/Wine values are only for this private compatibility probe. The
    // native Windows app does not require Wine or an X11 display connection.
    for key in [
        "SystemRoot",
        "TEMP",
        "TMP",
        "DISPLAY",
        "XAUTHORITY",
        "WINEPREFIX",
        "WINEDEBUG",
        "WINEDLLOVERRIDES",
        "RECOMP_OVERLAY_TEST",
    ] {
        if let Some(value) = std::env::var_os(key) {
            values.push((key.into(), value));
        }
    }
    for (key, value) in [
        (
            "GAMEPKG_TAKEDAWN_DISC",
            OsString::from("Z:\\must-not-open.iso"),
        ),
        ("RECOMP_SAVE_DIR", saves.into_os_string()),
        ("RECOMP_PB_BACKEND", "vulkan".into()),
        ("RECOMP_FLIP_MARK", "1".into()),
        ("RECOMP_STUB_LOG", "1".into()),
        ("SDL_AUDIODRIVER", "dummy".into()),
    ] {
        values.push((key.into(), value));
    }
    let mut game_args = Vec::new();
    if args[4] == "--headless" {
        game_args.push("--headless".into());
    }
    game_args.push("/gamepkg-disc/default.xbe".into());
    let mut session = Session::start(Launch {
        executable,
        args: game_args,
        cwd: std::env::current_dir()?,
        environment: values,
        disc,
        output,
        memory_limit_bytes: Some(4 * 1024 * 1024 * 1024),
    })?;
    println!("Rust owner started the native game with the selected ISO handle.");
    loop {
        match session.poll()? {
            Some(Event::Restarted { count }) => {
                println!("Native game restarted after normal shutdown: {count}")
            }
            Some(Event::Exited(code)) => {
                println!("Native game exited: {code}");
                return if code == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::other(format!("native game exit {code}")))
                };
            }
            None => {}
        }
        if began.elapsed() >= Duration::from_secs(seconds) {
            session.stop()?;
            println!(
                "Intentional end of bounded native process-owner probe; child stopped: {:?}",
                session.poll()?
            );
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("This maintainer probe targets native Windows.");
}
