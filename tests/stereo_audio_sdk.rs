use gamepkg::runtime::Game;
#[test]
#[ignore = "requires signed fixtures from tests/stereo_audio_game.py"]
fn generated_stereo_bindings_preserve_channels_and_atomic_rejections() {
    let root = std::path::PathBuf::from(std::env::var("GAMEPKG_STEREO_ROOT").unwrap());
    for suffix in ["core", "component"] {
        for enabled in [true, false] {
            let id = format!(
                "com.gamepkg.stereo-{suffix}-{}",
                if enabled { "on" } else { "off" }
            );
            let mut game = Game::load(&root, &id).unwrap();
            assert_eq!(game.step(0).unwrap(), if enabled { 31 } else { 28 });
            assert_eq!(game.audio_channels(), if enabled { 2 } else { 1 });
            assert_eq!(
                game.audio_frame(),
                if enabled {
                    &[32767, -32768, 16384, -16384][..]
                } else {
                    &[]
                }
            );
            assert_eq!(game.step(0).unwrap(), if enabled { 31 } else { 28 });
            assert_eq!(game.audio_frame().len(), if enabled { 4 } else { 0 });
        }
    }
}
