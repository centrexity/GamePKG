// Adapted from c0m4r/arkanoo; see CREDITS.md for revision and changes.
mod physics;
mod render;
use std::sync::Mutex;
static GAME: Mutex<Option<physics::Game>> = Mutex::new(None);

pub fn game_tick(input: u32, state: u64) -> u64 {
    if let Ok(mut slot) = GAME.lock() {
        if slot.is_none() {
            let mut game = physics::Game::new();
            let mut bytes = [0u8; 16];
            if gamepkg::save_read(0, &mut bytes) == Some(16) && bytes[0..4] == *b"ARK1" {
                game.best =
                    u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]).min(9999999);
                game.unlocked =
                    u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]).clamp(1, 999);
            }
            *slot = Some(game);
        }
        if let Some(game) = slot.as_mut() {
            game.update(input);
            render::draw(game);
            if game.sound > 0 {
                render::sound(game.sound, game.frame);
                game.sound = 0;
            }
            if game.dirty {
                let best = game.best.to_le_bytes();
                let level = game.unlocked.to_le_bytes();
                let bytes = [
                    65, 82, 75, 49, best[0], best[1], best[2], best[3], level[0], level[1],
                    level[2], level[3], 0, 0, 0, 0,
                ];
                if gamepkg::save_write(&bytes) {
                    game.dirty = false;
                }
            }
            if game.score >= 10 {
                gamepkg::achievement_unlock(0);
            }
            if game.unlocked >= 2 {
                gamepkg::achievement_unlock(1);
            }
            if game.unlocked >= 10 {
                gamepkg::achievement_unlock(2);
            }
        }
    }
    state.wrapping_add(1)
}
