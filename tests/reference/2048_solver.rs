// Maintainer oracle for pinned Adrien Ball 2048-rs (MIT).
// Copy to examples/gamepkg_golden.rs in that checkout and run:
// cargo +stable run --locked --example gamepkg_golden
// Use tests/fixtures/2048-solver.json source hashes and parameters.
use play_2048::{
    board::{Board, Direction},
    evaluators::*,
    solver::SolverBuilder,
};
fn evaluator() -> PrecomputedBoardEvaluator {
    PrecomputedBoardEvaluator::new(
        CombinedBoardEvaluator::default()
            .combine(
                MonotonicityEvaluator {
                    gameover_penalty: -200000.0,
                    monotonicity_power: 4,
                },
                1.0,
            )
            .combine(
                EmptyTileEvaluator {
                    gameover_penalty: 0.0,
                    power: 1,
                },
                200.0,
            )
            .combine(
                AlignmentEvaluator {
                    gameover_penalty: 0.0,
                    power: 1,
                },
                500.0,
            ),
    )
}
fn main() {
    let fixtures: [[u8; 16]; 6] = [
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [2, 2, 0, 2, 4, 0, 0, 1, 0, 3, 0, 4, 0, 3, 0, 4],
        [1, 2, 3, 4, 2, 3, 4, 5, 3, 4, 5, 6, 4, 5, 6, 6],
        [1, 2, 1, 2, 2, 1, 2, 1, 1, 2, 1, 2, 2, 1, 2, 1],
        [8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0, 0, 1, 1, 2, 2],
        [0, 0, 1, 1, 2, 2, 3, 3, 0, 4, 0, 4, 5, 0, 6, 0],
    ];
    let mut solver = SolverBuilder::default()
        .board_evaluator(evaluator())
        .proba_4(0.1)
        .base_max_search_depth(3)
        .min_branch_proba(0.001)
        .build();
    let evaluation = evaluator();
    for cells in fixtures {
        let board = Board::from(
            cells
                .iter()
                .map(|value| if *value == 0 { 0u16 } else { 1u16 << value })
                .collect::<Vec<_>>(),
        );
        let direction = match solver.next_best_move(board) {
            Some(Direction::Left) => "0",
            Some(Direction::Right) => "1",
            Some(Direction::Up) => "2",
            Some(Direction::Down) => "3",
            None => "null",
        };
        println!(
            "{{\"cells\":{:?},\"direction\":{},\"evaluation\":{}}}",
            cells,
            direction,
            evaluation.evaluate(board)
        );
    }
}
