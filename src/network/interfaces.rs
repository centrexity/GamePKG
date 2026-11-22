//! Bounded IPv4 interface discovery. No names, MACs or OS pointers cross the ABI.
use anyhow::{ensure, Result};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Interface {
    pub index: u32,
    pub flags: u32,
    pub ip: std::net::Ipv4Addr,
}
#[cfg(unix)]
type GetIfAddrs = unsafe extern "C" fn(*mut *mut libc::ifaddrs) -> libc::c_int;
#[cfg(unix)]
type FreeIfAddrs = unsafe extern "C" fn(*mut libc::ifaddrs);

#[cfg(all(unix, not(target_os = "android")))]
fn unix_provider() -> Result<(GetIfAddrs, FreeIfAddrs)> {
    Ok((libc::getifaddrs, libc::freeifaddrs))
}

#[cfg(target_os = "android")]
fn unix_provider() -> Result<(GetIfAddrs, FreeIfAddrs)> {
    // Android first exports these Bionic functions in API 24. Keep API 23
    // linkable and resolve only these fixed symbols from the loaded libc.
    unsafe {
        let get = libc::dlsym(libc::RTLD_DEFAULT, c"getifaddrs".as_ptr());
        let free = libc::dlsym(libc::RTLD_DEFAULT, c"freeifaddrs".as_ptr());
        ensure!(
            !get.is_null() && !free.is_null(),
            "interface discovery requires Android API 24 or later"
        );
        Ok((
            std::mem::transmute::<*mut libc::c_void, GetIfAddrs>(get),
            std::mem::transmute::<*mut libc::c_void, FreeIfAddrs>(free),
        ))
    }
}

#[cfg(all(test, target_os = "android"))]
pub(super) fn available() -> bool {
    unix_provider().is_ok()
}

#[cfg(unix)]
pub(super) fn list() -> Result<Vec<Interface>> {
    let (get, free) = unix_provider()?;
    let mut head = std::ptr::null_mut();
    ensure!(unsafe { get(&mut head) } == 0, "interface discovery failed");
    struct Guard(*mut libc::ifaddrs, FreeIfAddrs);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                (self.1)(self.0);
            }
        }
    }
    let _guard = Guard(head, free);
    let mut out = Vec::new();
    let mut current = head;
    let mut entries = 0;
    while !current.is_null() {
        entries += 1;
        ensure!(entries <= 4096, "too many host interfaces");
        let row = unsafe { &*current };
        current = row.ifa_next;
        if row.ifa_addr.is_null() || unsafe { (*row.ifa_addr).sa_family } as i32 != libc::AF_INET {
            continue;
        }
        let addr = unsafe { &*row.ifa_addr.cast::<libc::sockaddr_in>() };
        let ip = std::net::Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
        let mut flags = 0u32;
        for (native, wire) in [
            (libc::IFF_UP, 1),
            (libc::IFF_RUNNING, 2),
            (libc::IFF_LOOPBACK, 4),
            (libc::IFF_BROADCAST, 8),
            (libc::IFF_MULTICAST, 16),
        ] {
            if row.ifa_flags & native as u32 != 0 {
                flags |= wire;
            }
        }
        let index = if row.ifa_name.is_null() {
            0
        } else {
            unsafe { libc::if_nametoindex(row.ifa_name) }
        };
        out.push(Interface { index, flags, ip });
        ensure!(out.len() <= 256, "too many IPv4 interface addresses");
    }
    out.sort();
    out.dedup();
    Ok(out)
}
#[cfg(windows)]
pub(super) fn list() -> Result<Vec<Interface>> {
    use windows_sys::Win32::{
        Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA},
        NetworkManagement::IpHelper::{
            GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
            GAA_FLAG_SKIP_MULTICAST, IP_ADAPTER_ADDRESSES_LH,
        },
        Networking::WinSock::{AF_INET, SOCKADDR_IN},
    };
    // Native alignment, at most 1 MiB; provider owns all returned linked data.
    let mut storage = vec![0u64; 128 * 1024];
    let mut bytes = (storage.len() * 8) as u32;
    let head = storage.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
    let code = unsafe {
        GetAdaptersAddresses(
            AF_INET as u32,
            GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER,
            std::ptr::null(),
            head,
            &mut bytes,
        )
    };
    if code == ERROR_NO_DATA {
        return Ok(Vec::new());
    }
    ensure!(code != ERROR_BUFFER_OVERFLOW, "too many host interfaces");
    ensure!(code == 0, "interface discovery failed: {code}");
    let mut out = Vec::new();
    let mut current = head;
    let mut entries = 0;
    while !current.is_null() {
        entries += 1;
        ensure!(entries <= 4096, "too many host interfaces");
        let row = unsafe { &*current };
        current = row.Next;
        let up = row.OperStatus == 1;
        let loopback = row.IfType == 24;
        let mut flags = if up { 3 } else { 0 };
        if loopback {
            flags |= 4;
        }
        if matches!(row.IfType, 6 | 71) {
            flags |= 8;
        }
        if unsafe { row.Anonymous2.Flags } & 16 == 0 {
            flags |= 16;
        }
        let index = unsafe { row.Anonymous1.Anonymous.IfIndex };
        let mut address = row.FirstUnicastAddress;
        while !address.is_null() {
            entries += 1;
            ensure!(entries <= 4096, "too many interface addresses");
            let unicast = unsafe { &*address };
            address = unicast.Next;
            if unicast.Address.lpSockaddr.is_null()
                || unicast.Address.iSockaddrLength < std::mem::size_of::<SOCKADDR_IN>() as i32
            {
                continue;
            }
            let sa = unsafe { &*unicast.Address.lpSockaddr.cast::<SOCKADDR_IN>() };
            if sa.sin_family != AF_INET {
                continue;
            }
            let ip = std::net::Ipv4Addr::from(unsafe { sa.sin_addr.S_un.S_addr }.to_ne_bytes());
            out.push(Interface { index, flags, ip });
            ensure!(out.len() <= 256, "too many IPv4 interface addresses");
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}
#[cfg(not(any(unix, windows)))]
pub(super) fn list() -> Result<Vec<Interface>> {
    anyhow::bail!("interface discovery unsupported")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_ipv4_snapshot_is_bounded_and_contains_no_unspecified_addresses() {
        #[cfg(target_os = "android")]
        if !available() {
            assert!(list().is_err());
            return;
        }
        let rows = list().unwrap();
        assert!(rows.len() <= 256);
        assert!(rows.windows(2).all(|r| r[0] < r[1]));
        assert!(rows
            .iter()
            .all(|r| !r.ip.is_unspecified() && r.flags & !31 == 0));
    }
}
