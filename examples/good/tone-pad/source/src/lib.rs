// A quiet triangle wave: hold Up for a higher pitch, Down for a lower pitch.
pub fn game_tick(input: u32, state: u64) -> u64 {
    let period = if input & (4 | gamepkg::HELD_UP) != 0 { 120 } else if input & (8 | gamepkg::HELD_DOWN) != 0 { 480 } else { 240 };
    let mut samples = [0i16; 768];
    let mut phase = state as u32 % period;
    for sample in &mut samples {
        let value = if phase < period/2 { (phase*4000/period) as i32-1000 } else { 3000-(phase*4000/period) as i32 };
        *sample = value as i16;
        phase = (phase+1)%period;
    }
    let _ = (gamepkg::audio_submit(&samples));
    let _ = (gamepkg::draw_clear(0x102336ff));
    for index in 0..3 { let _ = (gamepkg::draw_rect((index*180+60) as f32,160.0,160.0,160.0,0x17324aff)); }
    let x = if period == 480 { 140.0 } else if period == 120 { 500.0 } else { 320.0 };
    let _ = (gamepkg::draw_circle(x,240.0,45.0,0x64dcb4ff));
    phase as u64
}
