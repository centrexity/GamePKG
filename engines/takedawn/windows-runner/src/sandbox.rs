//! A Windows AppContainer with no network capabilities. Only host-selected
//! runtime files and the game's own save directory receive its SID in their ACL.
use super::{invalid, wide};
use std::{
    ffi::OsStr,
    fs, io,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::{
            GetSecurityInfo, SetEntriesInAclW, SetSecurityInfo, EXPLICIT_ACCESS_W, GRANT_ACCESS,
            SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
        },
        FreeSid,
        Isolation::{CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName},
        DACL_SECURITY_INFORMATION, PSID, SECURITY_CAPABILITIES,
    },
};

/// Persistent per-game identity. The profile is reused to retain compatibility
/// with its original save files. It grants no networking or broad file access.
pub struct Sandbox {
    sid: PSID,
}
impl Sandbox {
    pub fn open(name: &str) -> io::Result<Self> {
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.')
        {
            return Err(invalid("invalid private game container name"));
        }
        let name = wide(OsStr::new(name))?;
        let mut sid = null_mut();
        // SAFETY: terminated strings and a valid output pointer. No capabilities
        // are requested; the API owns profile creation for the current user.
        let mut status = unsafe {
            CreateAppContainerProfile(
                name.as_ptr(),
                name.as_ptr(),
                name.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if status as u32 == 0x800700b7 {
            status = unsafe { DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) };
        }
        if status < 0 {
            return Err(io::Error::other(format!(
                "Windows game container setup failed (HRESULT {status:#x})"
            )));
        }
        if sid.is_null() {
            return Err(io::Error::other(
                "Windows returned no game container identity",
            ));
        }
        Ok(Self { sid })
    }
    pub(crate) fn capabilities(&self) -> SECURITY_CAPABILITIES {
        SECURITY_CAPABILITIES {
            AppContainerSid: self.sid,
            Capabilities: null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        }
    }
    /// The host calls this only for its private verified executable directory.
    pub fn grant_runtime(&self, path: &Path) -> io::Result<()> {
        self.grant(path, 0x0012_00a9)
    }
    /// The host calls this only for this game's exclusive save directory.
    pub fn grant_saves(&self, path: &Path) -> io::Result<()> {
        self.grant(path, 0x0013_01bf)
    }
    fn grant(&self, path: &Path, rights: u32) -> io::Result<()> {
        // Hold the actual object across ACL read/update. Do not follow a final
        // junction/symlink and accidentally grant access to its target.
        let file = fs::OpenOptions::new()
            .access_mode(0x0006_0000)
            .custom_flags(0x0220_0000)
            .open(path)?;
        let metadata = file.metadata()?;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(invalid(
                "game access cannot be granted through a reparse point",
            ));
        }
        let mut old_acl = null_mut();
        let mut descriptor = null_mut();
        let status = unsafe {
            GetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_acl,
                null_mut(),
                &mut descriptor,
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        struct Allocation(*mut core::ffi::c_void);
        impl Drop for Allocation {
            fn drop(&mut self) {
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
        let _descriptor = Allocation(descriptor);
        let entry = EXPLICIT_ACCESS_W {
            grfAccessPermissions: rights,
            grfAccessMode: GRANT_ACCESS,
            grfInheritance: if metadata.is_dir() { 3 } else { 0 },
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: 0,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: self.sid.cast(),
            },
        };
        let mut acl = null_mut();
        let status = unsafe { SetEntriesInAclW(1, &entry, old_acl, &mut acl) };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        let _acl = Allocation(acl.cast());
        let status = unsafe {
            SetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null(),
            )
        };
        if status != 0 {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
        Ok(())
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        unsafe {
            FreeSid(self.sid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, Launch, Session};
    use std::{
        fs::File,
        io::{Read, Write},
        os::windows::io::FromRawHandle,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Security::{GetTokenInformation, TokenIsAppContainer, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken, SetEvent},
    };
    #[test]
    fn child_permissions() {
        let Some(outside) = std::env::var_os("GAMEPKG_CONTAINER_FIXTURE") else {
            return;
        };
        let mut token = null_mut();
        assert_ne!(
            unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) },
            0
        );
        let token = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(token) };
        let mut is_container = 0u32;
        let mut length = 0;
        assert_ne!(
            unsafe {
                GetTokenInformation(
                    token.as_raw_handle(),
                    TokenIsAppContainer,
                    (&mut is_container as *mut u32).cast(),
                    4,
                    &mut length,
                )
            },
            0
        );
        assert_eq!(is_container, 1);
        assert!(
            File::open(&outside).is_err(),
            "outside files must remain inaccessible"
        );
        assert!(fs::write(&outside, b"changed").is_err());
        assert!(fs::OpenOptions::new()
            .write(true)
            .open(std::env::current_exe().unwrap())
            .is_err());
        let address = std::env::var("GAMEPKG_CONTAINER_ADDRESS")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_err(),
            "networking must remain disabled"
        );
        let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_DISC_HANDLE")
            .unwrap()
            .parse()
            .unwrap();
        let mut disc = unsafe { File::from_raw_handle(raw as _) };
        let mut text = String::new();
        disc.read_to_string(&mut text).unwrap();
        assert_eq!(text, "fixture selected ISO");
        assert!(disc.write_all(b"changed").is_err());
        // The owner retains the same source handle across restart. Rewind the
        // fixture read so both launches observe identical selected-disc bytes.
        use std::io::{Seek, SeekFrom};
        disc.seek(SeekFrom::Start(0)).unwrap();
        fs::write("temporary-save", b"original profile").unwrap();
        assert_eq!(fs::read("temporary-save").unwrap(), b"original profile");
        fs::remove_file("temporary-save").unwrap();
        if !Path::new("restart-complete").exists() {
            fs::write("restart-complete", b"persisted").unwrap();
            let raw: usize = std::env::var("GAMEPKG_TAKEDAWN_RESTART_HANDLE")
                .unwrap()
                .parse()
                .unwrap();
            assert_ne!(unsafe { SetEvent(raw as _) }, 0);
        } else {
            assert_eq!(fs::read("restart-complete").unwrap(), b"persisted");
        }
        println!("CONFINED_GAME_FIXTURE_OK");
    }
    #[test]
    fn container_preserves_iso_saves_and_restrictions_across_restart() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = directory.path().join("runtime with spaces");
        let saves = directory.path().join("saves");
        fs::create_dir(&runtime).unwrap();
        fs::create_dir(&saves).unwrap();
        let executable = runtime.join("fixture.exe");
        fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let outside = directory.path().join("outside.iso");
        fs::write(&outside, "fixture selected ISO").unwrap();
        let name = format!(
            "GamePKG.Test.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let sandbox = Sandbox::open(&name).unwrap();
        sandbox.grant_runtime(&runtime).unwrap();
        sandbox.grant_saves(&saves).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let log = directory.path().join("output.log");
        let mut session = Session::start_confined(
            Launch {
                executable,
                args: vec![
                    "--exact".into(),
                    "sandbox::tests::child_permissions".into(),
                    "--nocapture".into(),
                ],
                cwd: saves.clone(),
                environment: vec![
                    ("SystemRoot".into(), std::env::var_os("SystemRoot").unwrap()),
                    ("TEMP".into(), saves.as_os_str().to_owned()),
                    ("TMP".into(), saves.as_os_str().to_owned()),
                    (
                        "GAMEPKG_CONTAINER_FIXTURE".into(),
                        outside.as_os_str().to_owned(),
                    ),
                    (
                        "GAMEPKG_CONTAINER_ADDRESS".into(),
                        listener.local_addr().unwrap().to_string().into(),
                    ),
                ],
                disc: File::open(&outside).unwrap(),
                output: File::create(&log).unwrap(),
            },
            sandbox,
        )
        .unwrap();
        let began = Instant::now();
        let mut restarts = 0;
        loop {
            match session.poll().unwrap() {
                Some(Event::Restarted { count }) => restarts = count,
                Some(Event::Exited(code)) => {
                    assert_eq!(code, 0, "{}", fs::read_to_string(&log).unwrap());
                    break;
                }
                None => {}
            }
            assert!(
                began.elapsed() < Duration::from_secs(30),
                "confined fixture did not finish"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(session);
        assert_eq!(restarts, 1);
        assert_eq!(
            fs::read_to_string(&log)
                .unwrap()
                .matches("CONFINED_GAME_FIXTURE_OK")
                .count(),
            2
        );
        assert_eq!(
            fs::read_to_string(&outside).unwrap(),
            "fixture selected ISO"
        );
        assert_eq!(
            fs::read(saves.join("restart-complete")).unwrap(),
            b"persisted"
        );
        let name = wide(OsStr::new(&name)).unwrap();
        assert_eq!(
            unsafe {
                windows_sys::Win32::Security::Isolation::DeleteAppContainerProfile(name.as_ptr())
            },
            0
        );
    }
}
