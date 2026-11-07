//! OS-reported allocation for regular files. Never substitute file length for
//! unavailable allocation data, and never follow Windows reparse points.
use std::{fs::Metadata, path::Path};

#[cfg(unix)]
pub(crate) fn file(_path: &Path, metadata: &Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(metadata.blocks().saturating_mul(512))
}

#[cfg(windows)]
pub(crate) fn file(path: &Path, _metadata: &Metadata) -> Option<u64> {
    use std::{
        ffi::c_void,
        mem::MaybeUninit,
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
    };
    #[repr(C)]
    struct CompressionInfo {
        allocated: i64,
        format: u16,
        unit_shift: u8,
        chunk_shift: u8,
        cluster_shift: u8,
        reserved: [u8; 3],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandleEx(
            handle: *mut c_void,
            class: i32,
            info: *mut c_void,
            size: u32,
        ) -> i32;
    }
    // OPEN_REPARSE_POINT pins the entry itself if it changes after inventory.
    let file = std::fs::OpenOptions::new()
        // Allocation is metadata: do not request file-data access. This also
        // works while a writer denies data reads.
        .access_mode(0x0080) // FILE_READ_ATTRIBUTES
        .custom_flags(0x0020_0000)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.file_attributes() & 0x400 != 0 {
        return None;
    }
    let mut info = MaybeUninit::<CompressionInfo>::zeroed();
    // FileCompressionInfo measures sparse/compressed storage through this open
    // handle. Unsupported filesystems return None, not a misleading file size.
    let success = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            8,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<CompressionInfo>() as u32,
        )
    };
    if success == 0 {
        return None;
    }
    u64::try_from(unsafe { info.assume_init() }.allocated).ok()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn file(_path: &Path, _metadata: &Metadata) -> Option<u64> {
    None
}
