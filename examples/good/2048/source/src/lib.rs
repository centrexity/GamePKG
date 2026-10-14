// 2048-rs adaptation. Upstream MIT notice: assets/LICENSE-2048.txt.
mod rows;
fn tile(board: u64, index: u32) -> u32 { ((board >> (4 * index)) & 15) as u32 }
fn spawn(board: u64) -> u64 {
    let start = ((board ^ (board >> 23)).wrapping_mul(6364136223846793005) >> 32) as u32 % 16;
    for offset in 0..16 {
        let index = (start + offset) % 16;
        if tile(board, index) == 0 { return board | (1u64 << (4 * index)); }
    }
    board
}
pub fn slide(board: u64, direction: u32) -> u64 {
    let mut output = 0u64;
    for line in 0..4 {
        let mut row = 0u16;
        for position in 0..4 {
            let index = if direction < 2 { line * 4 + position } else { position * 4 + line };
            row |= (tile(board, index) as u16) << (4 * (3 - position));
        }
        let moved = if direction == 0 || direction == 2 { rows::get_left_move(row) } else { rows::get_right_move(row) };
        for position in 0..4 {
            let index = if direction < 2 { line * 4 + position } else { position * 4 + line };
            output |= (((moved >> (4 * (3 - position))) & 15) as u64) << (4 * index);
        }
    }
    output
}
fn digit(value: u32, x: f32, y: f32) {
    let patterns = [63u32,6,91,79,102,109,125,7,127,111];
    let pattern = patterns[value as usize];
    for segment in 0..7 {
        if pattern & (1 << segment) != 0 {
            let xs = [0.0,12.0,12.0,0.0,0.0,0.0,0.0];
            let ys = [0.0,0.0,12.0,24.0,12.0,0.0,12.0];
            let horizontal = segment == 0 || segment == 3 || segment == 6;
            let _ = gamepkg::draw_rect(x+xs[segment],y+ys[segment],if horizontal {14.0} else {2.0},if horizontal {2.0} else {14.0},0x202020ff);
        }
    }
}
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut board = if state == 0 { spawn(spawn(0)) } else { state };
    let direction = if input & 1 != 0 { 0 } else if input & 2 != 0 { 1 } else if input & 4 != 0 { 2 } else { 3 };
    if input & 15 != 0 {
        let moved = slide(board, direction);
        if moved != board { board = spawn(moved); }
    }
    let _ = gamepkg::draw_clear(0x302b27ff);
    for index in 0..16 {
        let exponent = tile(board,index);
        let x = 115.0 + (index % 4) as f32 * 104.0;
        let y = 25.0 + (index / 4) as f32 * 104.0;
        let colors = [0x57504affu32,0xeee4daff,0xede0c8ff,0xf2b179ff,0xf59563ff,0xf67c5fff,0xf65e3bff,0xedcf72ff];
        let color = colors[exponent.min(7) as usize];
        let _ = gamepkg::draw_rect(x,y,96.0,96.0,color);
        if exponent != 0 {
            let mut number = 1u32 << exponent;
            let mut place = 4;
            loop {
                digit(number % 10,x + 5.0 + place as f32 * 17.0,y + 34.0);
                number /= 10;
                if number == 0 { break; }
                place -= 1;
            }
        }
    }
    board
}
