use crate::gamepkg;
use crate::physics::Game;
fn glyph(character: u8) -> u64 {
    match character {
        65 => 18842895918,
        66 => 32801506878,
        67 => 16660316687,
        68 => 32801080894,
        69 => 33840644639,
        70 => 17734517279,
        71 => 16695149071,
        72 => 18842895921,
        73 => 15170932878,
        75 => 18879369809,
        76 => 33840185872,
        77 => 18842572657,
        78 => 18842507057,
        79 => 15621211694,
        80 => 17734518334,
        82 => 18879563326,
        83 => 32247333391,
        84 => 4433514655,
        85 => 15621211697,
        86 => 4648912433,
        87 => 11464787505,
        89 => 4433521201,
        48 => 15629733422,
        49 => 15170933124,
        50 => 33558693422,
        51 => 32247317566,
        52 => 2247698626,
        53 => 32247857695,
        54 => 15621636622,
        55 => 8866891839,
        56 => 15621113390,
        57 => 15067498030,
        47 => 17725261857,
        _ => 0,
    }
}
fn letters(label: &[u8], x: f32, y: f32, scale: f32, color: u32) {
    let width = label.len() * 6;
    if width == 0 || width > 512 {
        return;
    }
    let mut pixels = Vec::new();
    pixels.resize(width * 7 * 4, 0u8);
    for (index, character) in label.iter().enumerate() {
        let bits = glyph(*character);
        for row in 0..7 {
            for col in 0..5 {
                if bits & (1u64 << (row * 5 + 4 - col)) != 0 {
                    let offset = (row * width + index * 6 + col) * 4;
                    pixels[offset] = (color >> 24) as u8;
                    pixels[offset + 1] = (color >> 16) as u8;
                    pixels[offset + 2] = (color >> 8) as u8;
                    pixels[offset + 3] = color as u8;
                }
            }
        }
    }
    gamepkg::draw_image(
        x,
        y,
        width as f32 * scale,
        7.0 * scale,
        width as u32,
        7,
        &pixels,
    );
}
pub fn text(label: &str, x: f32, y: f32, scale: f32, color: u32) {
    letters(label.as_bytes(), x, y, scale, color);
}
fn number(mut value: u32, x: f32, y: f32) {
    let mut digits = [48u8; 7];
    let mut count = 1;
    digits[6] = 48 + (value % 10) as u8;
    value /= 10;
    while value > 0 && count < 7 {
        digits[6 - count] = 48 + (value % 10) as u8;
        value /= 10;
        count += 1;
    }
    letters(&digits[7 - count..], x, y, 2.0, 0xe5f3ffff);
}

pub fn draw(game: &Game) {
    gamepkg::draw_clear(0x0b1020ff);
    for n in 0..12 {
        gamepkg::draw_rect(0.0, 60.0 + n as f32 * 32.0, 640.0, 1.0, 0x17233aff);
    }
    gamepkg::draw_rect(6.0, 56.0, 2.0, 392.0, 0x345476ff);
    gamepkg::draw_rect(632.0, 56.0, 2.0, 392.0, 0x345476ff);
    text("ARKANOO", 16.0, 10.0, 2.0, 0x68e5ffff);
    text("SCORE", 136.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.score, 136.0, 23.0);
    text("BEST", 240.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.best, 240.0, 23.0);
    text("LEVEL", 352.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.level, 352.0, 23.0);
    text("LIVES", 446.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.lives, 446.0, 23.0);
    text("AMMO", 546.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.ammo, 546.0, 23.0);
    let colors = [
        0xff627fff, 0xffa45bff, 0xffdb70ff, 0x65de9eff, 0x5db9ffff, 0xb28cffff,
    ];
    for index in 0..200 {
        if game.blocks[index] > 0 {
            let x = 20.0 + (index % 20) as f32 * 30.0;
            let y = 72.0 + (index / 20) as f32 * 18.0;
            gamepkg::draw_rect(x, y, 28.0, 16.0, colors[(index / 20) % 6]);
            gamepkg::draw_rect(x + 2.0, y + 2.0, 24.0, 2.0, 0xffffff65);
        }
    }
    let color = if game.ghost > 0 {
        0xb18cffff
    } else {
        0x63dcffff
    };
    gamepkg::draw_rect(game.paddle, 428.0, game.width, 10.0, color);
    gamepkg::draw_rect(game.paddle + 3.0, 429.0, game.width - 6.0, 2.0, 0xffffffff);
    for ball in &game.balls {
        gamepkg::draw_circle(ball.x, ball.y, 4.0, 0xffffffff);
    }
    for drop in &game.drops {
        let colors = [0x62de9eff, 0xffcd60ff, 0xb28cffff, 0xff647fff];
        gamepkg::draw_rect(
            drop.x - 8.0,
            drop.y - 8.0,
            16.0,
            16.0,
            colors[drop.kind as usize],
        );
        let label = match drop.kind {
            0 => "L",
            1 => "B",
            2 => "G",
            _ => "R",
        };
        text(label, drop.x - 5.0, drop.y - 7.0, 2.0, 0x101828ff);
    }
    for (x, y) in &game.rockets {
        gamepkg::draw_rect(*x - 2.0, *y, 4.0, 12.0, 0xffdd70ff);
    }
    for particle in &game.particles {
        gamepkg::draw_rect(particle.x, particle.y, 2.0, 2.0, 0xffcc70ff);
    }
    text(
        "LEFT/RIGHT MOVE   UP FIRE   DOWN PAUSE",
        42.0,
        461.0,
        1.0,
        0x8ba2c9ff,
    );
    if game.mode != 1 {
        gamepkg::draw_rect(64.0, 268.0, 512.0, 138.0, 0x101b35ff);
        let title = match game.mode {
            0 => "ARKANOO",
            2 => "PAUSED",
            3 => "LEVEL CLEARED",
            _ => "GAME OVER",
        };
        text(title, 100.0, 284.0, 3.0, 0x68e5ffff);
        let detail = match game.mode {
            0 => "NINE LEVELS THEN ENDLESS PLAY",
            2 => "DOWN TO RESUME",
            3 => "UP FOR NEXT LEVEL",
            _ => "UP TO PLAY AGAIN",
        };
        text(detail, 100.0, 323.0, 2.0, 0xe5f3ffff);
        if game.mode == 0 {
            text("UP TO START", 100.0, 356.0, 2.0, 0xffdb70ff);
        }
        text(
            "L LONG   B BALL   G GHOST   R ROCKET",
            100.0,
            386.0,
            1.0,
            0x8ba2c9ff,
        );
    }
}
pub fn sound(kind: u32, frame: u32) {
    let mut samples = [0i16; 800];
    let period = match kind {
        1 => 100,
        2 => 64,
        3 => 48,
        _ => 180,
    };
    for i in 0..800 {
        let sign = if (i + frame as usize) % period < period / 2 {
            1
        } else {
            -1
        };
        samples[i] = (sign * (800 - i) as i32 * 5) as i16;
    }
    gamepkg::audio_submit(&samples);
}
