#![cfg(target_os = "linux")]
#[path = "../engines/native-services/lockdown.rs"]
mod lockdown;
#[test]
fn filtered_child_entry() {
    if std::env::var_os("GAMEPKG_LOCKDOWN_CHILD").is_none() {
        return;
    }
    lockdown::install().unwrap();
    assert!(std::fs::read("/etc/hostname").is_err());
    assert!(std::net::TcpStream::connect("127.0.0.1:9").is_err());
    assert!(std::process::Command::new("/bin/true").status().is_err());
    assert_eq!(
        std::thread::spawn(|| {
            assert!(std::fs::read("/etc/hostname").is_err());
            42
        })
        .join()
        .unwrap(),
        42
    );
    unsafe extern "C" {
        fn mmap(
            address: *mut std::ffi::c_void,
            bytes: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: isize,
        ) -> *mut std::ffi::c_void;
    }
    assert_eq!(
        unsafe { mmap(std::ptr::null_mut(), 4096, 7, 0x22, -1, 0) } as usize,
        usize::MAX
    );
}
#[test]
fn local_runtime_keeps_threads_but_denies_file_network_exec_and_rwx() {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "filtered_child_entry", "--nocapture"])
        .env("GAMEPKG_LOCKDOWN_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
