// Synthetic engine only: the integration test injects the actual trusted worker
// framing and snapshot-control functions, so malformed packets need no ROM.
#![allow(dead_code)]
mod gamepkg {
    use std::io::{Read, Write};
    fn u32s(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
    // ACTUAL_PACKET
    // ACTUAL_NEXT_TICK
    mod engine {
        const SIZE: usize = 275497;
        static mut BUFFER: [u8; SIZE + 2] = [0; SIZE + 2];
        unsafe fn snapshot_size() -> u32 { SIZE as u32 }
        unsafe fn snapshot_data() -> *mut u8 {
            std::ptr::addr_of_mut!(BUFFER).cast::<u8>().add(1)
        }
        unsafe fn snapshot_capture() -> i32 {
            let base = std::ptr::addr_of_mut!(BUFFER).cast::<u8>();
            *base = 0xa5; *base.add(SIZE+1) = 0x5a;
            for index in 0..SIZE { *base.add(index+1) = (index % 251) as u8; }
            0
        }
        unsafe fn snapshot_restore(length: u32) -> i32 {
            let base = std::ptr::addr_of_mut!(BUFFER).cast::<u8>();
            assert_eq!(*base, 0xa5); assert_eq!(*base.add(SIZE+1), 0x5a);
            if length as usize != SIZE || *base.add(1) == 0xee { -1 } else { 0 }
        }
        unsafe fn engine_status() -> u32 { 0x11223344 }
        unsafe fn engine_position() -> u32 { 0x55667788 }
        unsafe fn engine_controller() -> u32 { 0x99aabbcc }
        // ACTUAL_SNAPSHOT_CONTROL
    }
}
fn main() {
    gamepkg::__gamepkg_packet(&[255]);
    while let Some((_input, state)) = gamepkg::__gamepkg_next_tick() {
        let mut reply = vec![0]; reply.extend_from_slice(&state.wrapping_add(1).to_le_bytes());
        gamepkg::__gamepkg_packet(&reply);
    }
}
