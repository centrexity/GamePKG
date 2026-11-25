//! Trusted child-local memory implementation; never compiled as game source.
//! The bootstrap fixes address windows before executing any game entry point.
#[cfg(any(target_os = "linux", windows))]
use std::ffi::c_void;
use std::io;

pub const PAGE: u64 = 4096;
pub struct Mapping {
    address: usize,
    bytes: usize,
}
// All access, protection and destruction are serialized by LocalRuntime.
unsafe impl Send for Mapping {}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn mmap(
        address: *mut c_void,
        bytes: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: isize,
    ) -> *mut c_void;
    fn mprotect(address: *mut c_void, bytes: usize, protection: i32) -> i32;
    fn munmap(address: *mut c_void, bytes: usize) -> i32;
}
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn VirtualAlloc(address: *mut c_void, bytes: usize, kind: u32, protection: u32) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, bytes: usize, protection: u32, old: *mut u32) -> i32;
    fn VirtualFree(address: *mut c_void, bytes: usize, kind: u32) -> i32;
}
impl Mapping {
    pub fn reserve(address: u64, bytes: u64) -> io::Result<Self> {
        let address = usize::try_from(address).map_err(|_| io::ErrorKind::InvalidInput)?;
        let bytes = usize::try_from(bytes).map_err(|_| io::ErrorKind::InvalidInput)?;
        if address == 0
            || bytes == 0
            || address % 65536 != 0
            || bytes % 65536 != 0
            || address.checked_add(bytes).is_none()
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        #[cfg(target_os = "linux")]
        let actual = unsafe {
            mmap(
                address as *mut c_void,
                bytes,
                0,
                0x02 | 0x20 | 0x4000 | 0x100000,
                -1,
                0,
            )
        };
        #[cfg(windows)]
        let actual = unsafe { VirtualAlloc(address as *mut c_void, bytes, 0x2000, 0x01) };
        #[cfg(not(any(target_os = "linux", windows)))]
        return Err(io::ErrorKind::Unsupported.into());
        #[cfg(any(target_os = "linux", windows))]
        {
            if actual.is_null() || actual as usize == usize::MAX {
                return Err(io::Error::last_os_error());
            }
            if actual as usize != address {
                #[cfg(target_os = "linux")]
                unsafe {
                    munmap(actual, bytes);
                }
                #[cfg(windows)]
                unsafe {
                    VirtualFree(actual, 0, 0x8000);
                }
                return Err(io::ErrorKind::AddrInUse.into());
            }
            Ok(Self { address, bytes })
        }
    }
    fn page_range(&self, offset: u64, bytes: u64) -> io::Result<(usize, usize)> {
        let offset = usize::try_from(offset).map_err(|_| io::ErrorKind::InvalidInput)?;
        let bytes = usize::try_from(bytes).map_err(|_| io::ErrorKind::InvalidInput)?;
        if bytes == 0
            || offset % PAGE as usize != 0
            || bytes % PAGE as usize != 0
            || offset.checked_add(bytes).is_none_or(|end| end > self.bytes)
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok((self.address + offset, bytes))
    }
    pub fn commit(&self, offset: u64, bytes: u64, mode: u32) -> io::Result<()> {
        #[cfg(any(target_os = "linux", windows))]
        let (address, bytes) = self.page_range(offset, bytes)?;
        #[cfg(not(any(target_os = "linux", windows)))]
        let (_address, _bytes) = self.page_range(offset, bytes)?;
        if !(1..=2).contains(&mode) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        #[cfg(target_os = "linux")]
        let ok =
            unsafe { mprotect(address as *mut c_void, bytes, if mode == 1 { 1 } else { 3 }) } == 0;
        #[cfg(windows)]
        let ok = unsafe {
            VirtualAlloc(
                address as *mut c_void,
                bytes,
                0x1000,
                if mode == 1 { 2 } else { 4 },
            )
        } as usize
            == address;
        #[cfg(not(any(target_os = "linux", windows)))]
        let ok = false;
        if ok {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn protect(&self, offset: u64, bytes: u64, mode: u32) -> io::Result<()> {
        #[cfg(any(target_os = "linux", windows))]
        let (address, bytes) = self.page_range(offset, bytes)?;
        #[cfg(not(any(target_os = "linux", windows)))]
        let (_address, _bytes) = self.page_range(offset, bytes)?;
        if mode > 2 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        #[cfg(target_os = "linux")]
        let ok = unsafe { mprotect(address as *mut c_void, bytes, [0, 1, 3][mode as usize]) } == 0;
        #[cfg(windows)]
        let ok = {
            let mut old = 0;
            (unsafe {
                VirtualProtect(
                    address as *mut c_void,
                    bytes,
                    [1, 2, 4][mode as usize],
                    &mut old,
                )
            }) != 0
        };
        #[cfg(not(any(target_os = "linux", windows)))]
        let ok = false;
        if ok {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    pub fn decommit(&self, offset: u64, bytes: u64) -> io::Result<()> {
        #[cfg(any(target_os = "linux", windows))]
        let (address, bytes) = self.page_range(offset, bytes)?;
        #[cfg(not(any(target_os = "linux", windows)))]
        let (_address, _bytes) = self.page_range(offset, bytes)?;
        // MAP_FIXED is only used inside this runtime's still-owned reservation,
        // under its mutex. The initial window reservation never replaces maps.
        #[cfg(target_os = "linux")]
        let ok = unsafe {
            mmap(
                address as *mut c_void,
                bytes,
                0,
                0x02 | 0x20 | 0x10 | 0x4000,
                -1,
                0,
            )
        } as usize
            == address;
        #[cfg(windows)]
        let ok = unsafe { VirtualFree(address as *mut c_void, bytes, 0x4000) } != 0;
        #[cfg(not(any(target_os = "linux", windows)))]
        let ok = false;
        if ok {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    // Caller validates page access while holding the same runtime mutex.
    pub unsafe fn read(&self, offset: usize, output: &mut [u8]) {
        unsafe {
            std::ptr::copy_nonoverlapping(
                (self.address + offset) as *const u8,
                output.as_mut_ptr(),
                output.len(),
            );
        }
    }
    /// Caller holds the runtime mutex, owns both mappings, validates every
    /// page/range and bounds the length. Overlap has memmove semantics.
    /// # Safety
    /// Both complete ranges must be accessible with source read and target write
    /// rights, and their mappings must remain live and serialized for the call.
    pub unsafe fn copy_to(&self, source: usize, target: &Self, destination: usize, bytes: usize) {
        if bytes != 0 {
            unsafe {
                std::ptr::copy(
                    (self.address + source) as *const u8,
                    (target.address + destination) as *mut u8,
                    bytes,
                );
            }
        }
    }
    /// # Safety
    /// Caller owns this live mapping, holds its runtime mutex and has checked
    /// the complete range for committed writable pages. No pointer escapes.
    pub unsafe fn fill(&self, offset: usize, bytes: usize, value: u8) {
        if bytes != 0 {
            unsafe {
                std::ptr::write_bytes((self.address + offset) as *mut u8, value, bytes);
            }
        }
    }
    pub unsafe fn write(&self, offset: usize, input: &[u8]) {
        unsafe {
            std::ptr::copy_nonoverlapping(
                input.as_ptr(),
                (self.address + offset) as *mut u8,
                input.len(),
            );
        }
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        unsafe {
            munmap(self.address as *mut c_void, self.bytes);
        }
        #[cfg(windows)]
        unsafe {
            VirtualFree(self.address as *mut c_void, 0, 0x8000);
        }
    }
}
