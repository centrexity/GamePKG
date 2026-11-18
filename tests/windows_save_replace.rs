//! Host Windows adapter checks; also runnable with standalone `rustc --test`.
//! Wine is useful for adapter regressions, not NTFS/power-loss qualification.
#![cfg(windows)]

#[path = "../src/port_host/filesystem/windows_replace.rs"]
mod windows_replace;
