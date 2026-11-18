//! Trusted Linux child lockdown, installed after bootstrap and before game
//! entry. All subsequently created runtime threads inherit the same filter.
//! No file opens, sockets, process execution, fork, or executable mappings.
/// Detach the local worker's descriptor table before it enters the filter.
/// Platform providers in other threads keep their descriptors.
#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
#[allow(dead_code)]
pub fn install_local_worker() -> std::io::Result<()> {
    use std::ffi::{c_long, c_uint};
    unsafe extern "C" {
        fn syscall(number: c_long, ...) -> c_long;
    }
    const CLOSE_RANGE: c_long = 436;
    const UNSHARE: c_uint = 1 << 1;
    if unsafe { syscall(CLOSE_RANGE, 0 as c_uint, c_uint::MAX, UNSHARE) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    install()
}

#[cfg(target_os = "linux")]
pub fn install() -> std::io::Result<()> {
    use std::ffi::{c_int, c_ulong};
    #[repr(C)]
    struct Filter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }
    #[repr(C)]
    struct Program {
        len: u16,
        filter: *const Filter,
    }
    unsafe extern "C" {
        fn prctl(option: c_int, ...) -> c_int;
    }
    let f = |code, jt, jf, k| Filter { code, jt, jf, k };
    #[cfg(target_arch = "x86_64")]
    let (arch, map, protect, clone, prctl_nr, allowed): (u32, u32, u32, u32, u32, &[u32]) = (
        0xc000003e,
        9,
        10,
        56,
        157,
        &[
            0, 1, 3, 5, 8, 11, 12, 13, 14, 15, 19, 20, 24, 25, 28, 35, 39, 60, 96, 98, 131, 186,
            202, 218, 228, 230, 231, 234, 273, 302, 318, 334, 72,
        ],
    );
    #[cfg(target_arch = "x86")]
    let (arch, map, protect, clone, prctl_nr, allowed): (u32, u32, u32, u32, u32, &[u32]) = (
        0x40000003,
        192,
        125,
        120,
        172,
        &[
            1, 3, 4, 6, 19, 20, 45, 77, 78, 91, 108, 119, 140, 145, 146, 162, 163, 173, 174, 175,
            186, 197, 221, 224, 240, 243, 252, 258, 265, 267, 270, 311, 340, 355, 386, 403, 407,
            422, 55,
        ],
    );
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    return Err(std::io::ErrorKind::Unsupported.into());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        const ALLOW: u32 = 0x7fff0000;
        const DENY: u32 = 0x00050001;
        let mut filters = vec![
            f(0x20, 0, 0, 4),
            f(0x15, 1, 0, arch),
            f(0x06, 0, 0, 0x80000000),
            f(0x20, 0, 0, 0),
        ];
        for syscall in [map, protect] {
            filters.extend([
                f(0x15, 0, 4, syscall),
                f(0x20, 0, 0, 32),
                f(0x45, 0, 1, 4),
                f(0x06, 0, 0, DENY),
                f(0x06, 0, 0, ALLOW),
            ]);
        }
        // clone3 cannot be inspected through its user pointer. ENOSYS asks the
        // trusted pthread implementation to use the inspectable clone ABI.
        filters.extend([f(0x15, 0, 1, 435), f(0x06, 0, 0, 0x00050026)]);
        filters.extend([
            f(0x15, 0, 7, clone),
            f(0x20, 0, 0, 16),
            f(0x45, 0, 1, !0x003d0f00),
            f(0x06, 0, 0, DENY),
            f(0x54, 0, 0, 0x00010900),
            f(0x15, 0, 1, 0x00010900),
            f(0x06, 0, 0, ALLOW),
            f(0x06, 0, 0, DENY),
        ]);
        filters.extend([
            f(0x15, 0, 4, prctl_nr),
            f(0x20, 0, 0, 16),
            f(0x15, 0, 1, 15),
            f(0x06, 0, 0, ALLOW),
            f(0x06, 0, 0, DENY),
        ]);
        for &syscall in allowed {
            filters.extend([f(0x15, 0, 1, syscall), f(0x06, 0, 0, ALLOW)]);
        }
        filters.push(f(0x06, 0, 0, DENY));
        let program = Program {
            len: u16::try_from(filters.len()).map_err(|_| std::io::ErrorKind::InvalidInput)?,
            filter: filters.as_ptr(),
        };
        if unsafe { prctl(38, 1 as c_ulong, 0 as c_ulong, 0 as c_ulong, 0 as c_ulong) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe {
            prctl(
                22,
                2 as c_ulong,
                &program as *const Program as c_ulong,
                0 as c_ulong,
                0 as c_ulong,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}
#[cfg(not(target_os = "linux"))]
pub fn install() -> std::io::Result<()> {
    Ok(())
} // Windows confinement is the mandatory AppContainer/job owner.
