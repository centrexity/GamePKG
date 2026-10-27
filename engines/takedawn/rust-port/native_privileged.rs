//! Native host bindings for the reviewed cache/interrupt instruction sites.
//! Cached/uncached guest RAM views share coherent host pages. Device uploads
//! retain the renderer's Vulkan visibility operations; this publication fence
//! does not attempt to flush privileged physical caches on the player's OS.
use std::sync::atomic::{fence, Ordering};

extern "C" {
    fn xbox_NativeInterruptDisable();
    fn xbox_NativeInterruptEnableAfterShadow();
}

#[no_mangle]
pub extern "C" fn gamepkg_takedawn_cli() {
    unsafe { xbox_NativeInterruptDisable() }
}

/// The generated site calls this after the instruction following STI.
#[no_mangle]
pub extern "C" fn gamepkg_takedawn_sti_after_shadow() {
    unsafe { xbox_NativeInterruptEnableAfterShadow() }
}

#[no_mangle]
pub extern "C" fn gamepkg_takedawn_wbinvd() {
    fence(Ordering::SeqCst);
}
