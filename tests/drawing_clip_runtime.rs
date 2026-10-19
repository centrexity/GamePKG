use gamepkg::{drawing::Command, runtime::Game};
fn library() -> tempfile::TempDir {
    fn copy(source: &std::path::Path, target: &std::path::Path) {
        std::fs::create_dir_all(target).unwrap();
        for item in std::fs::read_dir(source).unwrap() {
            let item = item.unwrap();
            let dest = target.join(item.file_name());
            if item.file_type().unwrap().is_dir() {
                copy(&item.path(), &dest);
            } else {
                std::fs::copy(item.path(), dest).unwrap();
            }
        }
    }
    let root = tempfile::tempdir().unwrap();
    copy(
        std::path::Path::new(&std::env::var("GAMEPKG_CLIP_TEST_ROOT").unwrap()),
        root.path(),
    );
    root
}
#[test]
#[ignore = "requires fixtures prepared by tests/clipping_game.py"]
fn clip_bindings_validate_replace_reset_charge_and_rollback_across_profiles() {
    for id in ["com.gamepkg.clip-core", "com.gamepkg.clip-component"] {
        let root = library();
        let mut game = Game::load(root.path(), id).unwrap();
        assert_eq!(game.step(0x80000000).unwrap(), 127);
        assert!(matches!(
            game.frame().commands.as_slice(),
            [Command::Rect {
                x: 500.0,
                y: 350.0,
                ..
            }]
        ));
        assert_eq!(game.step(0x80000004).unwrap(), 31);
        assert!(
            matches!(game.frame().commands.as_slice(),[Command::LinearImage{x:10.0,y:20.0,width:30.0,height:40.0,image}] if image.size()==[1,1] && image.pixels()==[255,192,0,255])
        );
        assert_eq!(game.step(0).unwrap(), 0);
        let clips: Vec<_> = game
            .frame()
            .commands
            .iter()
            .filter_map(|c| {
                if let Command::Clip { rect } = c {
                    Some(*rect)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            clips,
            [
                Some([80.0, 60.0, 240.0, 180.0]),
                Some([360.0, 100.0, 200.0, 200.0]),
                None
            ]
        );
        assert_eq!(
            game.step(0x80000001).unwrap(),
            108,
            "Clearing or empty clipping refunded transferred image bytes"
        );
        assert!(game.frame().commands.is_empty());
        assert_eq!(game.step(0x80000002).unwrap(), 7168);
        assert_eq!(game.frame().commands.len(), 1024);
        assert_eq!(game.step(1 << 8).unwrap(), 4);
        assert!(
            !game
                .frame()
                .commands
                .iter()
                .any(|c| matches!(c, Command::Clip { .. })),
            "Clip persisted into next frame"
        );
        assert!(game.step(0x80000003).is_err());
        assert!(game.frame().commands.is_empty());
        assert!(game.frame().background.is_none());
    }
}
