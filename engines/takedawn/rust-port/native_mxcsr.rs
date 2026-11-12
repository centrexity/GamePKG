//! Native floating-point control for the reviewed x86 Takedawn runtime.
//! This uses the host's real SSE state, shared with the original native engine.
//! It contains no instruction decoder or software arithmetic implementation.
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
compile_error!("Takedawn native MXCSR requires an x86 host; other hosts need qualification");

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[no_mangle]
pub extern "C" fn gamepkg_takedawn_stmxcsr() -> u32 {
    let mut value = 0u32;
    // The writable four-byte local remains alive for the whole instruction.
    unsafe {
        core::arch::asm!(
            "stmxcsr [{address}]",
            address = in(reg) &mut value,
            options(nostack, preserves_flags)
        );
    }
    value
}

/// Restore the original native SSE control/status word on this OS thread.
///
/// # Safety
/// Reserved MXCSR bits must be zero, as required by the original instruction.
/// The reviewed game sets FTZ in a word just returned by STMXCSR.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[no_mangle]
pub unsafe extern "C" fn gamepkg_takedawn_ldmxcsr(value: u32) {
    // Memory is readable for four bytes. This instruction changes real native
    // floating-point state; it must never be marked pure or replaced by a copy.
    core::arch::asm!(
        "ldmxcsr [{address}]",
        address = in(reg) &value,
        options(nostack, preserves_flags, readonly)
    );
}
