//! Asset-free native Windows launch test. No game image, window, or audio.
#[cfg(not(windows))]
fn main() {
    panic!("Execute this fixture on native Windows");
}

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    test::run()
}

#[cfg(windows)]
mod test {
    use gamepkg_takedawn_windows_runner::{Process, Sandbox};
    use std::{
        ffi::{c_void, OsString},
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        os::windows::io::FromRawHandle,
        ptr::null,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Security::Isolation::DeleteAppContainerProfile,
        System::Memory::{
            VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
        },
    };

    struct Profile {
        sandbox: Option<Sandbox>,
        name: Vec<u16>,
        removed: bool,
    }
    impl Profile {
        fn cleanup(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            drop(self.sandbox.take());
            let status = unsafe { DeleteAppContainerProfile(self.name.as_ptr()) };
            if status < 0 {
                return Err(format!("Could not remove owned test profile: {status:#x}").into());
            }
            self.removed = true;
            Ok(())
        }
    }
    impl Drop for Profile {
        fn drop(&mut self) {
            if !self.removed {
                // Only this fixture's profile, never an app/game profile.
                let _ = self.cleanup();
            }
        }
    }
    struct Block(*mut c_void);
    impl Block {
        fn allocate(bytes: usize) -> Option<Self> {
            let pointer =
                unsafe { VirtualAlloc(null(), bytes, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE) };
            if pointer.is_null() {
                None
            } else {
                unsafe {
                    pointer.cast::<u8>().write_volatile(42);
                }
                Some(Self(pointer))
            }
        }
    }
    impl Drop for Block {
        fn drop(&mut self) {
            unsafe {
                VirtualFree(self.0, 0, MEM_RELEASE);
            }
        }
    }

    fn child() -> Result<(), Box<dyn std::error::Error>> {
        let raw: usize = std::env::var("GAMEPKG_DISC_HANDLE")?.parse()?;
        assert_eq!(
            std::env::var("GAMEPKG_TAKEDAWN_DISC_HANDLE")?,
            raw.to_string()
        );
        assert!(std::env::var_os("GAMEPKG_TAKEDAWN_RESTART_HANDLE").is_none());
        let mut selected = unsafe { File::from_raw_handle(raw as *mut c_void) };
        let mut contents = String::new();
        selected.read_to_string(&mut contents)?;
        assert_eq!(contents, "synthetic selected source");
        assert_eq!(
            selected.write(b"mutate").unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(fs::read(std::env::var_os("GP_TEST_SOURCE_PATH").unwrap()).is_err());
        assert!(fs::read(std::env::var_os("GP_TEST_CANARY").unwrap()).is_err());
        assert!(fs::write("ungranted.bin", b"must fail").is_err());
        fs::write("saves/checkpoint.txt", b"persistent fixture checkpoint")?;
        // The cache belongs to this process. Charging it a second time would
        // reject this 64 MiB allocation before the bounded engine workload.
        let cache = Block::allocate(64 * 1024 * 1024).expect("cache budget was counted twice");
        let mut game = Vec::new();
        let mut rejected = false;
        for _ in 0..24 {
            match Block::allocate(4 * 1024 * 1024) {
                Some(block) => game.push(block),
                None => {
                    rejected = true;
                    break;
                }
            }
        }
        let limited = std::env::var("GP_TEST_EXPECT_LIMIT")? == "1";
        assert_eq!(rejected, limited, "job-wide committed-memory policy");
        drop(game);
        drop(cache);
        println!("PASS confined engine: read-only inherited source, denied source path/canary/runtime writes, persistent saves, in-process cache, memory_limited={limited}");
        Ok(())
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        if std::env::args().nth(1).as_deref() == Some("--child") {
            return child();
        }
        for (budget, limited, double_counted) in [
            (128u64 * 1024 * 1024, true, false),
            (256u64 * 1024 * 1024, false, false),
            (64u64 * 1024 * 1024, true, true),
        ] {
            let folder = tempfile::tempdir()?;
            let runtime = folder.path().join("runtime");
            fs::create_dir(&runtime)?;
            let saves = runtime.join("saves");
            fs::create_dir(&saves)?;
            let executable = runtime.join("engine-fixture.exe");
            fs::copy(std::env::current_exe()?, &executable)?;
            let source_path = folder.path().join("selected.synthetic");
            fs::write(&source_path, b"synthetic selected source")?;
            // The launcher must reduce rights even if its parent can write.
            let source = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&source_path)?;
            let canary = folder.path().join("private-canary.txt");
            fs::write(&canary, b"must remain private")?;
            let output_path = folder.path().join("engine.log");
            let output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output_path)?;
            let name = format!(
                "GamePKG.CacheTest.{:x}",
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
            );
            let mut profile = Profile {
                sandbox: Some(Sandbox::open(&name)?),
                name: name.encode_utf16().chain(Some(0)).collect(),
                removed: false,
            };
            let sandbox = profile.sandbox.as_ref().unwrap();
            sandbox.grant_runtime(&runtime)?;
            sandbox.grant_saves(&saves)?;
            let mut values: Vec<(OsString, OsString)> = vec![
                (
                    "GP_TEST_SOURCE_PATH".into(),
                    source_path.clone().into_os_string(),
                ),
                ("GP_TEST_CANARY".into(), canary.clone().into_os_string()),
                (
                    "GP_TEST_EXPECT_LIMIT".into(),
                    if limited { "1" } else { "0" }.into(),
                ),
            ];
            for key in ["SystemRoot", "LOCALAPPDATA"] {
                values.push((
                    key.into(),
                    std::env::var_os(key).ok_or("Missing Windows profile environment")?,
                ));
            }
            let args = [OsString::from("--child")];
            assert!(
                matches!(Process::spawn_confined(&executable, &args, &runtime, &values, &source, &output, sandbox, 0), Err(error) if error.kind() == std::io::ErrorKind::InvalidInput)
            );
            assert_eq!(fs::metadata(&output_path)?.len(), 0);
            let process = Process::spawn_confined(
                &executable,
                &args,
                &runtime,
                &values,
                &source,
                &output,
                sandbox,
                budget,
            )?;
            let began = Instant::now();
            let code = loop {
                if let Some(code) = process.poll()? {
                    break code;
                }
                assert!(
                    began.elapsed() < Duration::from_secs(20),
                    "confined process deadline"
                );
                std::thread::sleep(Duration::from_millis(10));
            };
            drop(process);
            let log = fs::read_to_string(&output_path)?;
            if double_counted {
                assert_ne!(code, 0, "The double-counted budget must fail");
                assert!(log.contains("cache budget was counted twice"), "{log}");
                println!("PASS negative control: double-counted cache budget rejected at cache allocation");
            } else {
                assert_eq!(code, 0, "{log}");
                assert!(log.contains(&format!("memory_limited={limited}")), "{log}");
                print!("{log}");
            }
            assert_eq!(fs::read(&source_path)?, b"synthetic selected source");
            assert_eq!(fs::read(&canary)?, b"must remain private");
            assert_eq!(
                fs::read(saves.join("checkpoint.txt"))?,
                b"persistent fixture checkpoint"
            );
            profile.cleanup()?;
        }
        println!("PASS native external-process cache fixture; no game assets used");
        Ok(())
    }
}
