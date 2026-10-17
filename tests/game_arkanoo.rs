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

fn collision_ball(x: f32, y: f32, vx: f32, vy: f32) -> physics::Ball {
    physics::Ball {
        x,
        y,
        vx,
        vy,
        spin: 7.0,
        attached: false,
    }
}

#[test]
fn ball_contacts_restore_original_upward_ejection_and_shockwave() {
    let mut game = physics::Game::new();
    game.balls = vec![
        collision_ball(100.0, 200.0, 8.0, 2.0),
        collision_ball(105.0, 200.0, -9.0, -6.0),
    ];
    game.collide_balls();
    assert_eq!((game.balls[0].vx, game.balls[0].vy), (-2.5, -4.0));
    assert_eq!((game.balls[1].vx, game.balls[1].vy), (2.5, -6.0));
    assert_eq!(game.balls[0].spin, 7.0);
    assert_eq!(game.particles.len(), 36);
    assert_eq!(game.sound, 1);
    for particle in &game.particles {
        assert_eq!((particle.x, particle.y), (102.5, 200.0));
        assert_eq!(particle.color, 0xc8ffffff);
        assert!((particle.vx.hypot(particle.vy) - 3.0).abs() < 0.00001);
    }
}

#[test]
fn ball_contacts_keep_strict_distance_and_original_coincident_response() {
    let mut game = physics::Game::new();
    game.balls = vec![
        collision_ball(100.0, 200.0, 1.0, 2.0),
        collision_ball(106.0, 200.0, -1.0, 3.0),
    ];
    game.collide_balls();
    assert!(game.particles.is_empty());
    assert_eq!(game.balls[0].vx, 1.0);
    game.balls[1].x = 100.0;
    game.collide_balls();
    // The unchanged original sends exact coincidences right, not NaN/random.
    assert_eq!((game.balls[0].vx, game.balls[1].vx), (2.5, 2.5));
    assert_eq!(game.particles.len(), 36);
}

#[test]
fn ball_contacts_snapshot_all_pairs_and_skip_portal_or_lost_balls() {
    let mut game = physics::Game::new();
    game.balls = vec![
        collision_ball(100.0, 200.0, 1.0, 2.0),
        collision_ball(102.0, 200.0, -1.0, 3.0),
        collision_ball(104.0, 200.0, 9.0, 10.0),
    ];
    game.portal = true;
    game.collide_balls();
    assert!(game.particles.is_empty());
    game.portal = false;
    game.collide_balls();
    assert_eq!(game.particles.len(), 108);
    assert_eq!(
        game.balls
            .iter()
            .map(|ball| (ball.vx, ball.vy))
            .collect::<Vec<_>>(),
        vec![(-2.5, -4.0), (-2.5, -4.0), (2.5, -10.0)]
    );
    game.particles.clear();
    for ball in &mut game.balls {
        ball.y = 490.0;
    }
    game.collide_balls();
    assert!(game.particles.is_empty());
}

#[test]
fn contacts_match_unchanged_pinned_original_block() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-contacts.json")).unwrap();
    for fixture in reference["fixtures"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.portal = fixture["portal"].as_bool().unwrap();
        game.balls.clear();
        for ball in fixture["balls"].as_array().unwrap() {
            // Original coordinates are top-left; the port uses half-scale centers.
            game.balls.push(collision_ball(
                ball[0].as_f64().unwrap() as f32 * 0.5 + 3.0,
                ball[1].as_f64().unwrap() as f32 * 0.5 + 3.0,
                ball[2].as_f64().unwrap() as f32 * 0.5,
                ball[3].as_f64().unwrap() as f32 * 0.5,
            ));
        }
        game.collide_balls();
        for (ball, expected) in game
            .balls
            .iter()
            .zip(fixture["expected"].as_array().unwrap())
        {
            assert_eq!(
                [ball.vx * 2.0, ball.vy * 2.0],
                [
                    expected[0].as_f64().unwrap() as f32,
                    expected[1].as_f64().unwrap() as f32
                ]
            );
        }
        assert_eq!(
            game.particles.len(),
            fixture["particles"].as_u64().unwrap() as usize
        );
    }
}
