pub fn game_main() {
    let _ = show();
}

fn show() -> Result<(), i32> {
    let mut saves = 0;
    for grant in gamepkg::grants()? {
        if grant.kind == 2 {
            saves = grant.id;
        }
    }
    if saves == 0 {
        return Err(-4);
    }
    let restarted = gamepkg::file_exists(saves, "window-test.bin")?;
    if restarted {
        let file = gamepkg::file_open(saves, false, "window-test.bin")?;
        let mut bytes = [0; 4];
        let count = gamepkg::file_read(file, 0, &mut bytes)?;
        gamepkg::file_close(file)?;
        if count != 4 || &bytes != b"RSTR" {
            return Err(-3);
        }
    } else {
        let file = gamepkg::file_create_new(saves, "window-test.bin")?;
        gamepkg::file_write(file, 0, b"RSTR")?;
        gamepkg::file_sync(file)?;
        gamepkg::file_close(file)?;
    }

    let initial = gamepkg::image_create(
        1,
        1,
        if restarted {
            &[255, 0, 0, 255]
        } else {
            &[0, 0, 255, 255]
        },
    )?;
    let active = gamepkg::image_create(1, 1, &[0, 255, 0, 255])?;
    let mut right = false;
    loop {
        let window = gamepkg::window_state()?;
        if window.closed {
            return Ok(());
        }
        for event in gamepkg::input_events()? {
            if event.code == 79 && (event.kind == 1 || event.kind == 2) {
                right = event.kind == 1;
            }
        }
        let w = window.width as f32;
        let h = window.height as f32;
        if w > 0.0 && h > 0.0 {
            let draw = gamepkg::ImageDraw {
                image: if right && window.focused {
                    active
                } else {
                    initial
                },
                rect: [w / 4.0, h / 4.0, w / 2.0, h / 2.0],
                linear: false,
                shader: None,
            };
            gamepkg::present(0x000000ff, &[draw])?;
        }
        let (now, generation) = gamepkg::monotonic()?;
        gamepkg::wait_until(now.saturating_add(16_000_000), generation)?;
    }
}
