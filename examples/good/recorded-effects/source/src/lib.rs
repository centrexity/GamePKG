pub fn game_tick(input:u32,state:u64)->u64 {
    gamepkg::effect_volume(64);
    let pressed=input&gamepkg::HELD_START!=0;
    if pressed && state==0 {gamepkg::effect_play("chime.wav");}
    gamepkg::draw_clear(0x102638ff);
    gamepkg::draw_circle(320.0,240.0,if pressed {80.0}else{60.0},0x62ddadff);
    u64::from(pressed)
}
