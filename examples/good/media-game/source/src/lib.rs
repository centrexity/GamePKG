const EXPECT_AUDIO: bool = true;
pub fn game_tick(_input: u32, state: u64) -> u64 {
    let Some(shader) = gamepkg_shader::Shader::from_slang("shaders/texture.slang", "blueMain") else { return 0; };
    let Some(plain) = gamepkg_shader::Shader::from_slang("shaders/main.slang", "fragmentMain") else { return 0; };
    if gamepkg_shader::Shader::from_slang("../outside.slang", "blueMain").is_some() { return 0; }
    if gamepkg_shader::Shader::from_slang("shaders/texture.slang", "missing").is_some() { return 0; }
    if shader.draw_rgba(513, 1, &[0; 4]) { return 0; }
    if shader.draw_rgba(1, 1, &[0; 3]) { return 0; }
    if shader.draw_texture("../manifest.toml") { return 0; }
    if shader.draw_texture("missing.png") { return 0; }
    if shader.draw_texture("hello.txt") { return 0; }
    if gamepkg_audio::submit(&[0; 1601]) { return 0; }
    if gamepkg_audio::submit_pcm16_le(&[0]) { return 0; }
    if gamepkg_audio::submit(&[100, -100]) != EXPECT_AUDIO { return 0; }
    if gamepkg_audio::submit_pcm16_le(&[0, 64, 0, 192]) != EXPECT_AUDIO { return 0; }
    if gamepkg_audio::submit(&[0; 1597]) { return 0; }
    if !plain.draw() { return 0; }
    if !shader.draw_texture("texture.png") { return 0; }
    if !shader.draw_rgba(1, 1, &[0, 0, 255, 255]) { return 0; }
    let _ = gamepkg_window::rect(48.0, 48.0, 4.0, 4.0, 0x00ff00ff);
    state + 1
}
