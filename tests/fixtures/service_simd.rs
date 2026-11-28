fn run() -> Result<(), i32> {
    let a = [1u32, 2, 3, 4, 5, 6, 7, 8];
    let b = [10u32, 20, 30, 40, 50, 60, 70, 80];
    let wide = gamepkg::simd_u32x8_wrapping_sum(gamepkg::simd_u32x8_add(a, b));
    if wide != 396 {
        return Err(-3);
    }
    let weighted = gamepkg::simd_f32x4_sum(gamepkg::simd_f32x4_mul(
        [1.0, 2.0, 3.0, 4.0],
        [10.0, 20.0, 30.0, 40.0],
    ));
    if weighted != 300.0 {
        return Err(-3);
    }
    let float_wide = gamepkg::simd_f32x8_sum(gamepkg::simd_f32x8_add(
        [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
        [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0],
    ));
    if float_wide != 396.0 {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x53494d44, &[u64::from(wide), 300, 396])?;
    Ok(())
}
pub fn game_main() {
    let code = match run() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x53494dff, &[code]);
}
