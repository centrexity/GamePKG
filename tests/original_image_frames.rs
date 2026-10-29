//! Real signed full-source-resolution frames, prepared privately by the maintainer runner.
use gamepkg::{drawing::Command, runtime::Game};
use sha2::{Digest, Sha256};
#[test]
#[ignore = "requires private signed original-image recipes from check_original_image_frames.py"]
fn complete_original_background_frames_preserve_all_tile_pixels() {
    let root =
        std::path::PathBuf::from(std::env::var_os("GAMEPKG_ORIGINAL_IMAGE_LIBRARY").unwrap());
    let expected: Vec<Vec<String>> =
        serde_json::from_slice(&std::fs::read(root.join("expected-images.json")).unwrap()).unwrap();
    for profile in ["core-v1", "component-v1"] {
        for native in if profile == "core-v1" {
            vec![false, true]
        } else {
            vec![false]
        } {
            let id = format!("com.gamepkg.full-images-{profile}");
            let mut game = Game::load_target(&root, &id, &[], Some(native)).unwrap();
            for (scene, hashes) in expected.iter().enumerate() {
                assert_eq!(
                    game.step(if scene == 0 { 0 } else { 2 }).unwrap(),
                    ((scene as u64) << 32) | 24,
                    "{profile} native={native} scene={scene}"
                );
                let images: Vec<_> = game
                    .frame()
                    .commands
                    .iter()
                    .filter_map(|c| {
                        if let Command::LinearImage { image, .. } = c {
                            Some(image)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(images.len(), 24);
                let total: usize = images.iter().map(|p| p.pixels().len()).sum();
                assert_eq!(total, 17430768);
                assert_eq!(
                    images
                        .iter()
                        .map(|p| hex::encode(Sha256::digest(p.pixels())))
                        .collect::<Vec<_>>(),
                    *hashes
                );
                println!("{profile} native={native} scene={scene}: all24 original tile hashes, {total} bytes and {} commands preserved",game.frame().commands.len());
            }
            let config = std::path::PathBuf::from(std::env::var_os("GAMEPKG_CONFIG").unwrap());
            assert_eq!(
                config.parent(),
                root.parent(),
                "Only a private fixture config may be changed"
            );
            let original = std::fs::read(&config).unwrap();
            let mut settings = gamepkg::settings::Settings::read(&config).unwrap();
            settings.memory_ceiling_mib = 128;
            settings.save(&config).unwrap();
            let denied = Game::load_target(&root, &id, &[], Some(native))
                .err()
                .map(|e| e.to_string());
            std::fs::write(&config, original).unwrap();
            assert!(
                denied
                    .as_deref()
                    .is_some_and(|e| e.contains("memory ceiling")),
                "{profile} native={native} must retain the lower user memory ceiling: {denied:?}"
            );
        }
    }
}
