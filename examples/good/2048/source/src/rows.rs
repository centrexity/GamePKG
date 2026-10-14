// Adapted from Adrien Ball, 2048-rs (MIT). See assets/LICENSE-2048.txt.
pub fn get_left_move(row: u16) -> u16 {
    let mut result = row;
    let mut prev_value = 255;
    let mut new_value_idx = 0;
    // whether or not tiles have been moved in this row
    let mut moved = false;
    for i in 0..4 {
        let value: u8 = ((row & (0xF << (4 * (3 - i)))) >> (4 * (3 - i))) as u8;
        if value == 0 {
            moved = true;
        } else if value == prev_value && value < 15 {
            result = set_value_in_row(result, new_value_idx - 1, value + 1);
            result = set_value_in_row(result, i as u8, 0);
            prev_value = 255;
            moved = true;
        } else {
            if moved {
                result = set_value_in_row(result, new_value_idx, value);
                result = set_value_in_row(result, i as u8, 0);
            }
            prev_value = value;
            new_value_idx += 1;
        }
    }
    result
}

pub fn get_right_move(row: u16) -> u16 {
    invert_row(get_left_move(invert_row(row)))
}

fn invert_row(row: u16) -> u16 {
    let mut inverted_row: u16 = 0;
    for i in 0..4 {
        let value = (row >> (4 * i)) & 0xF;
        inverted_row = set_value_in_row(inverted_row, i as u8, value as u8);
    }
    inverted_row
}

fn set_value_in_row(row: u16, idx: u8, value: u8) -> u16 {
    // bitmask with 0000 at the corresponding tile_idx and 1s everywhere else
    let clear_mask: u16 = !(0xF << (4 * (3 - idx) as u16));
    let update_mask: u16 = (value as u16) << (4 * (3 - idx) as u16);
    (row & clear_mask) | update_mask
}
