//! Native Windows tests import the exact production notification code.
#![cfg(windows)]
#![allow(dead_code)]

#[path = "../../../../vendor/winit/src/platform_impl/windows/display_restore.rs"]
mod display_restore;
#[path = "../../../../vendor/winit/src/platform_impl/windows/display_restore_state.rs"]
mod display_restore_state;
