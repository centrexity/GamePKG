fn policy_probe(input: u32) -> u64 {
    gamepkg::draw_clear(0x102030ff);
    if input & 15 == 1 {
        let mut pixels=Vec::new();pixels.resize(1048576,255u8);
        let mut accepted=0;
        gamepkg::draw_clip(0.0,0.0,0.0,0.0);
        for _ in 0..8 {if gamepkg::draw_image(0.0,0.0,1.0,1.0,512,512,&pixels){accepted+=1;}gamepkg::draw_clear(0x102030ff);}
        if !gamepkg::draw_image(0.0,0.0,1.0,1.0,512,512,&pixels){accepted+=100;}
        return accepted;
    }
    if input & 15 == 2 {
        let mut accepted=0;
        for _ in 0..1024 {if gamepkg::draw_clip_reset(){accepted+=1;}}
        if !gamepkg::draw_clip(0.0,0.0,1.0,1.0){accepted+=2048;}
        if !gamepkg::draw_rect(0.0,0.0,1.0,1.0,0xffffffff){accepted+=4096;}
        return accepted;
    }
    if input & 15 == 3 {
        gamepkg::draw_clip(1.0,2.0,3.0,4.0);
        gamepkg::draw_rect(0.0,0.0,640.0,480.0,0xff0000ff);
        let data=[0u8];return u64::from(data[(input&1) as usize]);
    }
    if input & 15 == 4 {
        let mut accepted=0;
        if !gamepkg::draw_image_linear(0.0,0.0,1.0,1.0,0,1,&[]){accepted+=1;}
        if !gamepkg::draw_image_linear(0.0,0.0,1.0,1.0,513,1,&[]){accepted+=2;}
        if !gamepkg::draw_image_linear(0.0,0.0,1.0,1.0,1,1,&[0,0,0]){accepted+=4;}
        if !gamepkg::draw_image_linear(f32::NAN,0.0,1.0,1.0,1,1,&[0,0,0,0]){accepted+=8;}
        if gamepkg::draw_image_linear(10.0,20.0,30.0,40.0,1,1,&[255,192,0,255]){accepted+=16;}
        return accepted;
    }
    let mut accepted=0;
    if gamepkg::draw_clip(10.0,20.0,30.0,40.0){accepted+=1;}
    if !gamepkg::draw_clip(f32::NAN,0.0,1.0,1.0){accepted+=2;}
    if !gamepkg::draw_clip(-1.0,0.0,1.0,1.0){accepted+=4;}
    if !gamepkg::draw_clip(0.0,0.0,641.0,480.0){accepted+=8;}
    if gamepkg::draw_clip(0.0,0.0,0.0,0.0){accepted+=16;}
    if gamepkg::draw_clip_reset(){accepted+=32;}
    gamepkg::draw_clear(0x102030ff);
    if gamepkg::draw_rect(500.0,350.0,30.0,30.0,0x00ff00ff){accepted+=64;}
    accepted
}
pub fn game_tick(input: u32, _: u64) -> u64 {
    if input & 2147483648 != 0 {return policy_probe(input);}
    if input & gamepkg::HELD_LEFT != 0 {
        gamepkg::draw_rect(500.0,350.0,30.0,30.0,0x00ff00ff);
        gamepkg::draw_rect(20.0,400.0,20.0,20.0,0xffffffff);return 4;
    }
    gamepkg::draw_clear(0x102030ff);
    if input & gamepkg::HELD_RIGHT != 0 {
        gamepkg::draw_clip(160.0,120.0,160.0,120.0);
        gamepkg::draw_shader(0);
        gamepkg::draw_shader_image(0,1,1,&[0,0,0,0]);
        gamepkg::draw_rect(20.0,400.0,20.0,20.0,0xffffffff);return 1;
    }
    if input & gamepkg::HELD_UP != 0 {
        gamepkg::draw_clip(0.0,0.0,0.0,0.0);
        gamepkg::draw_shader(0);
        gamepkg::draw_shader_image(0,1,1,&[0,0,0,0]);
        gamepkg::draw_circle(300.0,210.0,40.0,0x00ff00ff);
        gamepkg::draw_asset_image_linear("gold.png",0.0,0.0,640.0,480.0);
        gamepkg::draw_image(0.0,0.0,640.0,480.0,1,1,&[255,0,0,255]);
        gamepkg::draw_clip_reset();
        gamepkg::draw_rect(20.0,400.0,20.0,20.0,0xffffffff);return 2;
    }
    if input & gamepkg::HELD_DOWN != 0 {
        gamepkg::draw_clip(10.0,10.0,40.0,40.0);
        gamepkg::draw_rect(0.0,0.0,640.0,480.0,0xff0000ff);
        gamepkg::draw_clear(0x102030ff);
        gamepkg::draw_rect(500.0,350.0,30.0,30.0,0x00ff00ff);
        gamepkg::draw_rect(20.0,400.0,20.0,20.0,0xffffffff);return 3;
    }
    gamepkg::draw_clip(80.0,60.0,240.0,180.0);
    gamepkg::draw_rect(0.0,0.0,640.0,480.0,0xff0000ff);
    gamepkg::draw_circle(300.0,210.0,40.0,0x00ff00ff);
    gamepkg::draw_image_linear(260.0,70.0,100.0,120.0,1,1,&[255,192,0,255]);
    gamepkg::draw_image(60.0,160.0,100.0,100.0,1,1,&[0,0,255,255]);
    gamepkg::draw_clip(360.0,100.0,200.0,200.0);
    gamepkg::draw_rect(0.0,0.0,640.0,480.0,0x9900ffff);
    gamepkg::draw_clip_reset();
    gamepkg::draw_rect(20.0,400.0,20.0,20.0,0xffffffff);0
}
