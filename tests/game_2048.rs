mod adapted {
    pub mod gamepkg {
        thread_local! {
            static WORDS: std::cell::RefCell<std::collections::VecDeque<u64>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
            static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
        }
        pub fn samples(values: &[u64]) {
            WORDS.with(|words| *words.borrow_mut() = values.iter().copied().collect());
        }
        pub fn random_u64() -> u64 {
            WORDS
                .with(|words| words.borrow_mut().pop_front())
                .unwrap_or_else(|| {
                    NEXT.with(|next| {
                        let value = next.get().wrapping_mul(6364136223846793005).wrapping_add(1);
                        next.set(value);
                        value
                    })
                })
        }
    }
    pub mod model {
        include!("../ports/2048/source/src/model.rs");
        impl Clone for Position {
            fn clone(&self) -> Self {
                self.copy()
            }
        }
        impl PartialEq for Position {
            fn eq(&self, other: &Self) -> bool {
                self.cells == other.cells
                    && self.score == other.score
                    && self.continued == other.continued
            }
        }
        impl std::fmt::Debug for Position {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple("Position")
                    .field(&self.cells)
                    .field(&self.score)
                    .field(&self.continued)
                    .finish()
            }
        }
        impl Clone for Game {
            fn clone(&self) -> Self {
                Self {
                    position: self.position.copy(),
                    history: self.history.clone(),
                    best: self.best,
                    revision: self.revision,
                    dirty: self.dirty,
                }
            }
        }
        impl PartialEq for Game {
            fn eq(&self, other: &Self) -> bool {
                self.position == other.position
                    && self.history == other.history
                    && self.best == other.best
                    && self.revision == other.revision
                    && self.dirty == other.dirty
            }
        }
        impl std::fmt::Debug for Game {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple("Game")
                    .field(&self.position)
                    .field(&self.history)
                    .field(&self.best)
                    .field(&self.revision)
                    .field(&self.dirty)
                    .finish()
            }
        }
    }
}
#[path = "../ports/2048/source/src/rows.rs"]
mod original_rows;
use adapted::model::{slide, spawn, Game, Position};
fn cells(value: u64) -> [u8; 16] {
    let mut cells = [0u8; 16];
    for (index, cell) in cells.iter_mut().enumerate() {
        *cell = ((value >> (4 * index)) & 15) as u8;
    }
    cells
}
fn game(board: [u8; 16]) -> Game {
    Game {
        position: Position {
            cells: board,
            score: 0,
            continued: false,
        },
        best: 0,
        history: Vec::new(),
        revision: 1,
        dirty: false,
    }
}
#[test]
fn every_original_row_move_matches_until_the_old_representation_limit() {
    for row in 0..=u16::MAX {
        let board = cells(u64::from(row));
        if !board.contains(&15) {
            let moved = slide(&board, 0).0;
            // Original rows use the most significant nibble as their left edge.
            let reverse = |r: u16| {
                ((r & 15) << 12) | ((r & 240) << 4) | ((r & 3840) >> 4) | ((r & 61440) >> 12)
            };
            let expected = reverse(original_rows::get_left_move(reverse(row)));
            assert_eq!(
                &moved[..4],
                &cells(u64::from(expected))[..4],
                "row {row:04x}"
            );
            let right = reverse(original_rows::get_right_move(reverse(row)));
            assert_eq!(&slide(&board, 1).0[..4], &cells(u64::from(right))[..4]);
        }
    }
}
#[test]
fn merges_score_once_and_preserve_all_directions_and_large_tiles() {
    let (left, score) = slide(&cells(0x1111), 0);
    assert_eq!(&left[..4], &[2, 2, 0, 0]);
    assert_eq!(score, 8);
    assert_eq!(&slide(&cells(0x1111), 1).0[..4], &[0, 0, 2, 2]);
    let (up, score) = slide(&cells(0x1000100010001), 2);
    assert_eq!(up[0], 2);
    assert_eq!(up[4], 2);
    assert_eq!(score, 8);
    let down = slide(&cells(0x1000100010001), 3).0;
    assert_eq!(down[8], 2);
    assert_eq!(down[12], 2);
    let (big, score) = slide(&cells(0xff), 0);
    assert_eq!(big[0], 16);
    assert_eq!(score, 65536);
    let (triple, score) = slide(&cells(0x111), 0);
    assert_eq!(&triple[..4], &[2, 1, 0, 0]);
    assert_eq!(score, 4);
    let (chain, score) = slide(&cells(0x211), 0);
    assert_eq!(&chain[..4], &[2, 2, 0, 0]);
    assert_eq!(score, 4);
}
#[test]
fn spawning_is_uniform_and_rejection_sampled_with_original_four_probability() {
    for rank in 0..16 {
        for value in 10..20 {
            adapted::gamepkg::samples(&[value, rank]);
            let mut board = [0; 16];
            spawn(&mut board);
            let mut expected = [0; 16];
            expected[rank as usize] = if value % 10 == 0 { 2 } else { 1 };
            assert_eq!(board, expected);
        }
    }
    let mut full = [1u8; 16];
    spawn(&mut full);
    assert_eq!(full, [1; 16]);
    adapted::gamepkg::samples(&[0, 5, 10, 16]);
    let mut board = [0; 16];
    spawn(&mut board);
    assert_eq!(board[0], 2);
}
#[test]
fn undo_restart_win_continue_game_over_and_noop_inputs_are_consistent() {
    let mut run = game(cells(0x11));
    run.update(1);
    assert_eq!(run.position.score, 4);
    assert_eq!(run.best, 4);
    assert_eq!(run.history.len(), 1);
    run.update(16);
    assert_eq!(run.position.cells, cells(0x11));
    assert_eq!(run.position.score, 0);
    assert_eq!(run.best, 4);
    run.update(64);
    assert_eq!(run.best, 4);
    assert!(run.history.is_empty());
    assert_eq!(run.position.score, 0);
    let before = run.clone();
    run.update(1 << 8);
    assert_eq!(run, before, "held input must not repeat moves");
    let mut win = game(cells(0xaa));
    win.update(1);
    assert!(win.won());
    assert_eq!(win.position.score, 2048);
    let frozen = win.clone();
    win.update(2);
    assert_eq!(win, frozen);
    win.update(65536);
    assert!(win.position.continued);
    win.update(16);
    assert!(!win.won());
    assert!(!win.position.continued);
    let lost = game([1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1]);
    assert!(lost.over());
    let mut edge = game(cells(1));
    let original = edge.clone();
    edge.update(1);
    assert_eq!(
        edge, original,
        "invalid moves may not spawn or consume undo"
    );
}
#[test]
fn saves_restore_board_score_best_undo_and_continue_with_strict_validation() {
    let mut run = game(cells(0x11));
    run.update(1);
    let bytes = run.encode();
    let loaded = Game::decode(&bytes).unwrap();
    assert_eq!(loaded.position, run.position);
    assert_eq!(loaded.history, run.history);
    assert_eq!(loaded.best, 4);
    assert!(!loaded.dirty);
    let mut resumed = loaded;
    resumed.update(16);
    assert_eq!(resumed.position.cells, cells(0x11));
    assert_eq!(resumed.best, 4);
    for length in 0..bytes.len() {
        assert!(Game::decode(&bytes[..length]).is_none());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(Game::decode(&extra).is_none());
    for offset in [0, 4, 16, 17, 18, 19, 48, 49, 50, 51] {
        let mut bad = bytes.clone();
        bad[offset] = 255;
        assert!(Game::decode(&bad).is_none(), "offset {offset}");
    }
    let mut bad = bytes.clone();
    bad[24] = 64;
    assert!(Game::decode(&bad).is_none());
    let mut bad = bytes;
    bad[40] = 3;
    assert!(Game::decode(&bad).is_none());
    let mut win = game(cells(0xaa));
    win.update(1);
    win.update(65536);
    assert!(Game::decode(&win.encode()).unwrap().position.continued);
}

#[test]
fn maximum_history_and_mutated_saves_remain_bounded_and_canonical() {
    let mut run = game(cells(0x11));
    for _ in 0..128 {
        run.history.push(run.position.copy());
    }
    run.update(1);
    assert_eq!(run.history.len(), 128);
    let bytes = run.encode();
    assert_eq!(bytes.len(), 3636);
    assert_eq!(Game::decode(&bytes).unwrap().encode(), bytes);
    for index in 0..bytes.len() {
        for value in [0, 255] {
            let mut mutated = bytes.clone();
            mutated[index] = value;
            if let Some(decoded) = Game::decode(&mutated) {
                assert_eq!(decoded.encode(), mutated);
            }
        }
    }
    let mut too_many = bytes;
    too_many[16] = 129;
    assert!(Game::decode(&too_many).is_none());
}
