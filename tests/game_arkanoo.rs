#[path = "../ports/arkanoo/source/src/physics.rs"]
mod physics;
#[test]
fn campaign_and_endless_are_nonempty() {
    for level in 1..30 {
        let mut count = 0;
        for row in 0..10 {
            for col in 0..20 {
                count += physics::pattern(level, row, col) as usize;
            }
        }
        assert!(count >= 20, "level {level}: {count}");
    }
}
#[test]
fn controls_pause_and_paddle_bounds() {
    let mut game = physics::Game::new();
    game.update(4);
    assert_eq!(game.mode, 1);
    for _ in 0..200 {
        game.update(256);
    }
    assert_eq!(game.paddle, 8.0);
    game.update(8);
    let x = game.balls[0].x;
    game.update(512);
    assert_eq!(game.balls[0].x, x);
    game.update(8);
    assert_eq!(game.mode, 1);
}
#[test]
fn bonuses_are_bounded_and_expire() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.collect(0);
    assert_eq!(game.width, 100.0);
    for _ in 0..10 {
        game.collect(1);
        game.collect(3);
    }
    assert_eq!(game.balls.len(), 4);
    assert_eq!(game.ammo, 9);
    game.collect(2);
    assert_eq!(game.ghost, 600);
    game.update(4);
    assert_eq!(game.ammo, 8);
    assert_eq!(game.rockets.len(), 1);
}
#[test]
fn collisions_score_and_checkpoint_level_completion() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.blocks = [0; 200];
    game.blocks[0] = 1;
    game.balls[0].attached = false;
    game.balls[0].x = 34.0;
    game.balls[0].y = 94.0;
    game.balls[0].vx = 0.0;
    game.balls[0].vy = -4.0;
    game.update(0);
    assert_eq!(game.blocks[0], 0);
    assert_eq!(game.score, 10);
    assert!(game.balls[0].vy > 0.0);
    assert_eq!(game.mode, 3);
    assert_eq!(game.unlocked, 2);
    assert!(game.dirty);
    game.update(4);
    assert_eq!(game.level, 2);
    assert_eq!(game.mode, 1);
}
#[test]
fn loss_and_restart_do_not_underflow_or_keep_stale_bonuses() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.lives = 1;
    game.balls[0].attached = false;
    game.balls[0].y = 500.0;
    game.update(0);
    assert_eq!(game.mode, 4);
    assert_eq!(game.lives, 0);
    assert_eq!(game.score, 0);
    game.update(4);
    assert_eq!(game.lives, 3);
    assert_eq!(game.level, 1);
    assert_eq!(game.balls.len(), 1);
}

#[test]
fn ghost_passes_through_and_rockets_clear_neighbours() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.ghost = 600;
    game.balls[0].attached = false;
    game.balls[0].x = 34.0;
    game.balls[0].y = 94.0;
    game.balls[0].vx = 0.0;
    game.balls[0].vy = -4.0;
    game.update(0);
    assert_eq!(game.blocks[20], 0);
    assert!(game.balls[0].vy < 0.0);
    game.rockets.push((334.0, 94.0));
    let before = game.score;
    game.update(0);
    assert!(game.score >= before + 30);
    assert!(game.rockets.is_empty());
}

#[test]
fn campaign_can_advance_into_endless_and_resume_checkpoint() {
    let mut game = physics::Game::new();
    game.unlocked = 9;
    game.update(4);
    assert_eq!(game.level, 9);
    game.blocks = [0; 200];
    game.update(0);
    assert_eq!(game.mode, 3);
    assert_eq!(game.unlocked, 10);
    game.update(4);
    assert_eq!(game.level, 10);
    assert!(game.blocks.contains(&1));
}

#[test]
fn temporary_bonuses_expire_without_unbounded_entities() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.launch = u32::MAX;
    game.collect(0);
    game.collect(2);
    for _ in 0..600 {
        game.update(0);
    }
    assert_eq!(game.width, 70.0);
    assert_eq!(game.long, 0);
    assert_eq!(game.ghost, 0);
    for i in 0..200 {
        game.hit(i);
    }
    assert!(game.particles.len() <= 64);
    assert!(game.drops.len() <= 8);
}

#[test]
fn original_gravity_and_spin_equations_use_one_sixtieth_second_steps() {
    let mut ball = physics::Ball {
        x: 0.0,
        y: 0.0,
        vx: 2.0,
        vy: 1.0,
        spin: 3.0,
        attached: false,
    };
    ball.apply_forces(false);
    assert!((ball.vx - 2.15).abs() < 0.00001);
    assert!((ball.spin - 2.94).abs() < 0.00001);
    ball.apply_forces(true);
    assert_eq!(ball.spin, 0.0);
    assert!((ball.vy - 1.15).abs() < 0.00001);
    for _ in 0..100 {
        ball.apply_forces(true);
    }
    assert_eq!(ball.vy, 4.5);
    ball.vy = -3.0;
    ball.apply_forces(true);
    assert_eq!(ball.vy, -3.0);
    ball.vy = 6.0;
    ball.apply_forces(true);
    assert_eq!(
        ball.vy, 6.0,
        "original gravity does not slow an already faster ball"
    );
}
#[test]
fn speed_portal_activates_once_orbits_and_does_not_lose_lives() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.blocks = [0; 200];
    game.blocks[0] = 1;
    game.balls[0].attached = false;
    game.balls[0].vx = 30.0;
    game.balls[0].vy = 0.0;
    game.update(0);
    assert!(game.portal);
    assert_eq!(game.score, 5000);
    let score = game.score;
    game.update(0);
    assert_eq!(game.score, score);
    let angle = game.frame as f32 * 0.1;
    assert!((game.balls[0].x - (320.0 + angle.cos() * 75.0)).abs() < 0.001);
    let score = game.score;
    game.update(0);
    assert_eq!(game.score, score);
    game.balls.clear();
    let lives = game.lives;
    game.update(0);
    assert_eq!(game.lives, lives);
}
#[test]
fn portal_pull_preserves_integer_world_motion_and_delays_completion() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.portal = true;
    game.blocks = [0; 200];
    game.blocks[0] = 1;
    game.block_x[0] = 266.0;
    game.block_y[0] = 232.0;
    game.pull_blocks();
    assert_eq!(game.block_x[0], 270.0);
    assert_eq!(game.block_y[0], 232.0);
    game.block_x[0] = 306.0;
    game.pull_blocks();
    assert_eq!(game.blocks[0], 0);
    assert_eq!(game.portal_timer, 0);
    for _ in 0..269 {
        game.pull_blocks();
    }
    assert_eq!(game.mode, 1);
    game.pull_blocks();
    assert_eq!(game.mode, 3);
    assert_eq!(game.unlocked, 2);
    game.update(4);
    assert!(!game.portal);
    assert_eq!(game.portal_timer, 0);
    assert_eq!(game.max_speed, 0.0);
}
#[test]
fn gravity_toggle_is_edge_triggered_and_spin_runs_once_per_frame() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.blocks = [0; 200];
    game.blocks[0] = 1;
    game.balls[0].attached = false;
    game.balls[0].x = 300.0;
    game.balls[0].y = 350.0;
    game.balls[0].vx = 2.0;
    game.balls[0].vy = -1.0;
    game.balls[0].spin = 3.0;
    game.update(0);
    assert!((game.balls[0].vx - 2.15).abs() < 0.00001);
    assert!((game.balls[0].spin - 2.94).abs() < 0.00001);
    game.update(131072);
    assert!(game.gravity);
    assert_eq!(game.balls[0].spin, 0.0);
    game.update(131072 << 4);
    assert!(game.gravity);
    game.update(131072);
    assert!(!game.gravity);
}

#[test]
fn paused_portal_freezes_orbit_clock_and_absorption() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.portal = true;
    game.update(0);
    game.update(8);
    assert_eq!(game.mode, 2);
    let frame = game.frame;
    let xs = game.block_x;
    let ys = game.block_y;
    let timer = game.portal_timer;
    for _ in 0..30 {
        game.update(0);
    }
    assert_eq!(game.frame, frame);
    assert_eq!(game.block_x, xs);
    assert_eq!(game.block_y, ys);
    assert_eq!(game.portal_timer, timer);
    game.update(8);
    assert_eq!(game.mode, 1);
    assert_eq!(game.frame, frame + 1);
}

#[test]
fn scaled_force_steps_match_the_compiled_pinned_upstream_fragment() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-forces.json")).unwrap();
    assert_eq!(
        reference["revision"],
        "9d613e78c9ed5657ae7fda9206cd4a82a893b7c8"
    );
    for fixture in reference["fixtures"].as_array().unwrap() {
        let values = fixture["velocity_spin"].as_array().unwrap();
        let mut ball = physics::Ball {
            x: 0.0,
            y: 0.0,
            vx: values[0].as_f64().unwrap() as f32 * 0.5,
            vy: values[1].as_f64().unwrap() as f32 * 0.5,
            spin: values[2].as_f64().unwrap() as f32 * 0.5,
            attached: false,
        };
        for _ in 0..fixture["steps"].as_u64().unwrap() {
            ball.apply_forces(fixture["gravity"].as_bool().unwrap());
        }
        for (actual, expected) in [ball.vx, ball.vy, ball.spin]
            .iter()
            .zip(fixture["expected"].as_array().unwrap())
        {
            assert!((*actual - expected.as_f64().unwrap() as f32 * 0.5).abs() < 0.00001);
        }
    }
}
