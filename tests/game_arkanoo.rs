mod gamepkg {
    thread_local! {
        static WORDS: std::cell::RefCell<std::collections::VecDeque<u64>> = const { std::cell::RefCell::new(std::collections::VecDeque::new()) };
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(7) };
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
#[path = "../ports/arkanoo/source/src/audio.rs"]
mod audio;
#[path = "../ports/arkanoo/source/src/editor.rs"]
mod editor;
#[path = "../ports/arkanoo/source/src/levels.rs"]
mod levels;
#[path = "../ports/arkanoo/source/src/menu.rs"]
mod menu;
#[path = "../ports/arkanoo/source/src/pattern_rng.rs"]
mod pattern_rng;
#[path = "../ports/arkanoo/source/src/physics.rs"]
mod physics;
#[test]
fn campaign_and_endless_are_nonempty() {
    assert_eq!(
        levels::generate(0),
        [1; 200],
        "original level-zero fallback"
    );
    for level in 1..30 {
        let blocks = levels::generate(level);
        let count = blocks.iter().filter(|cell| **cell != 0).count();
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
    assert_eq!(game.paddle, 0.0);
    game.update(8);
    let x = game.balls[0].x;
    game.update(512);
    assert_eq!(game.balls[0].x, x);
    game.update(8);
    assert_eq!(game.mode, 1);
}
#[test]
fn original_bonus_counts_and_timers_are_retained() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.collect(0);
    assert_eq!(game.width, 90.0);
    for _ in 0..10 {
        game.collect(1);
        game.collect(3);
    }
    assert_eq!(game.balls.len(), 11);
    assert_eq!(game.ammo, 10);
    game.collect(2);
    assert_eq!(game.ghost, 600);
    game.update(4);
    assert_eq!(
        game.ammo, 10,
        "launching attached balls must not also fire a rocket"
    );
    assert!(game.rockets.is_empty());
    game.update(4);
    assert_eq!(game.ammo, 9);
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
    game.balls[0].y = 114.0;
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
    game.balls[0].y = 114.0;
    game.balls[0].vx = 0.0;
    game.balls[0].vy = -4.0;
    game.update(0);
    assert_eq!(game.blocks[0], 0);
    assert_eq!(game.blocks[20], 0);
    assert_eq!(game.score, 20);
    assert!(game.balls[0].vy < 0.0);
    game.rockets.push((334.0, 114.0));
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
    assert_eq!(game.mode, 5);
    assert_eq!(game.unlocked, 10);
    game.update(4);
    assert_eq!(game.level, 10);
    assert!(game.blocks.contains(&1));
}

#[test]
fn temporary_bonuses_expire_and_drop_cooldown_applies() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.balls[0].launch_timer = u32::MAX;
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
    assert!(game.drops.len() <= 1);
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
        launch_timer: 0,
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
    game.block_x[0] = 265.0;
    game.block_y[0] = 235.0;
    game.pull_blocks();
    assert_eq!(game.block_x[0], 269.0);
    assert_eq!(game.block_y[0], 235.0);
    game.block_x[0] = 305.0;
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
            launch_timer: 0,
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
        launch_timer: 0,
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

#[test]
fn world_steps_match_original_update_and_actual_sdl_intersections() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-world.json")).unwrap();
    for fixture in reference["fixtures"].as_array().unwrap() {
        let values: Vec<f32> = fixture["ball"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap() as f32)
            .collect();
        let mut ball = collision_ball(
            values[0] * 0.5 + 3.0,
            values[1] * 0.5 + 63.0,
            values[2] * 0.5,
            values[3] * 0.5,
        );
        ball.spin = values[4] * 0.5;
        for expected in fixture["expected"].as_array().unwrap() {
            if ball.active() {
                ball.advance(fixture["gravity"].as_bool().unwrap());
            }
            let actual = [
                (ball.x - 3.0) * 2.0,
                (ball.y - 63.0) * 2.0,
                ball.vx * 2.0,
                ball.vy * 2.0,
                ball.spin * 2.0,
                if ball.active() { 1.0 } else { 0.0 },
            ];
            for (actual, expected) in actual.iter().zip(expected.as_array().unwrap()) {
                assert!(
                    (actual - expected.as_f64().unwrap() as f32).abs() < 0.002,
                    "{actual} != {expected}"
                );
            }
        }
    }
    for fixture in reference["intersections"].as_array().unwrap() {
        fn scaled(value: &serde_json::Value) -> [f32; 4] {
            let mut result = [0.0; 4];
            for i in 0..4 {
                result[i] = value[i].as_f64().unwrap() as f32 * 0.5;
            }
            result[1] += physics::TOP;
            result
        }
        let result = physics::overlap(scaled(&fixture["first"]), scaled(&fixture["second"]));
        assert_eq!(
            [result.0 * 2.0, result.1 * 2.0],
            [
                fixture["expected"][0].as_f64().unwrap() as f32,
                fixture["expected"][1].as_f64().unwrap() as f32
            ]
        );
    }
}

#[test]
fn original_world_geometry_and_paddle_speed_have_uniform_scale() {
    let mut game = physics::Game::new();
    assert_eq!((game.block_x[0], game.block_y[0]), (20.0, 100.0));
    assert_eq!((game.block_x[199], game.block_y[199]), (590.0, 190.0));
    assert_eq!(game.balls[0].y, 392.0);
    game.mode = 1;
    let paddle = game.paddle;
    game.update(512);
    assert_eq!(game.paddle - paddle, 9.0);
    let mut ball = collision_ball(0.0, 61.0, -3.0, -2.0);
    ball.spin = 0.0;
    ball.advance(false);
    assert_eq!((ball.x, ball.y, ball.vx, ball.vy), (3.0, 63.0, 3.0, 2.0));
}

#[test]
fn paddle_motion_centered_bonus_and_expiry_match_unchanged_original() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-world.json")).unwrap();
    for fixture in reference["paddles"].as_array().unwrap() {
        let mut game = physics::Game::new();
        for (event, expected) in fixture["events"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["expected"].as_array().unwrap())
        {
            let action = event[0].as_str().unwrap();
            let mut movement = 0.0;
            for _ in 0..event[1].as_u64().unwrap() {
                let input = match action {
                    "left" => 256,
                    "right" => 512,
                    "both" => 768,
                    "long" => {
                        game.collect(0);
                        0
                    }
                    "ghost" => {
                        game.collect(2);
                        0
                    }
                    _ => 0,
                };
                movement = game.update_paddle(input);
            }
            assert_eq!(
                [
                    game.paddle * 2.0,
                    game.width * 2.0,
                    movement * 2.0,
                    game.long as f32,
                    game.ghost as f32
                ],
                expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
                    .as_slice(),
                "action {action}"
            );
        }
    }
}

#[test]
fn brick_resolution_matches_original_code_and_sdl_for_all_sides_and_ghosts() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-world.json")).unwrap();
    for fixture in reference["blocks"].as_array().unwrap() {
        let values: Vec<f32> = fixture["ball"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap() as f32)
            .collect();
        let mut ball = collision_ball(
            values[0] * 0.5 + 3.0,
            values[1] * 0.5 + 63.0,
            values[2] * 0.5,
            values[3] * 0.5,
        );
        let hit = ball.resolve_block(20.0, 100.0, fixture["ghost"].as_bool().unwrap());
        let actual = [
            (ball.x - 3.0) * 2.0,
            (ball.y - 63.0) * 2.0,
            ball.vx * 2.0,
            ball.vy * 2.0,
            if hit { 1.0 } else { 0.0 },
        ];
        for (actual, expected) in actual.iter().zip(fixture["expected"].as_array().unwrap()) {
            assert!(
                (actual - expected.as_f64().unwrap() as f32).abs() < 0.0001,
                "{actual} != {expected}"
            );
        }
    }
}

#[test]
fn automatic_and_manual_launch_frames_match_unchanged_original_for_each_direction() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-transitions.json")).unwrap();
    for fixture in reference["launches"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.mode = 1;
        let direction = fixture["direction"].as_u64().unwrap();
        gamepkg::samples(&[3 + direction]);
        for (frame, expected) in fixture["expected"].as_array().unwrap().iter().enumerate() {
            game.update(if frame == 0 && fixture["manual"].as_bool().unwrap() {
                4
            } else {
                0
            });
            let ball = &game.balls[0];
            let actual = [
                (ball.x - 3.0) * 2.0,
                (ball.y - 63.0) * 2.0,
                ball.vx * 2.0,
                ball.vy * 2.0,
                ball.launch_timer as f32,
                if ball.attached { 1.0 } else { 0.0 },
            ];
            for (actual, expected) in actual.iter().zip(expected.as_array().unwrap()) {
                assert_eq!(
                    *actual,
                    expected.as_f64().unwrap() as f32,
                    "direction {direction}, frame {frame}"
                );
            }
        }
    }
}

#[test]
fn loss_and_level_completion_match_unchanged_original_fragments() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-transitions.json")).unwrap();
    for fixture in reference["finishes"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.mode = 1;
        game.level = fixture["level"].as_u64().unwrap() as u32;
        game.lives = fixture["lives"].as_u64().unwrap() as u32;
        game.lost_life = fixture["lost"].as_bool().unwrap();
        game.finish_level();
        assert_eq!(
            [game.lives, game.mode],
            [
                fixture["expected"][0].as_u64().unwrap() as u32,
                fixture["expected"][1].as_u64().unwrap() as u32
            ]
        );
    }
    for fixture in reference["losses"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.mode = 1;
        game.lives = fixture["lives"].as_u64().unwrap() as u32;
        game.score = fixture["score"].as_u64().unwrap() as u32;
        game.portal = fixture["portal"].as_bool().unwrap();
        game.width = 90.0;
        game.balls.clear();
        game.update(0);
        assert_eq!(
            [
                game.lives,
                game.score,
                game.balls.len() as u32,
                u32::from(game.lost_life),
                game.mode
            ],
            fixture["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<_>>()
                .as_slice()
        );
        if let Some(expected) = fixture.get("ball") {
            let ball = &game.balls[0];
            assert_eq!(
                [
                    (ball.x - 3.0) * 2.0,
                    (ball.y - 63.0) * 2.0,
                    ball.vx * 2.0,
                    ball.vy * 2.0,
                    ball.launch_timer as f32
                ],
                expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
                    .as_slice()
            );
        }
    }
}

#[test]
fn life_loss_preserves_items_and_new_level_resets_them_at_original_boundaries() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.collect(0);
    game.collect(2);
    game.collect(3);
    game.drops.push(physics::Drop {
        x: 100.0,
        y: 300.0,
        kind: 0,
    });
    game.rockets.push((500.0, 300.0));
    game.balls.clear();
    game.score = 100;
    game.update(0);
    assert_eq!(
        (game.lives, game.score, game.width, game.long, game.ghost, game.ammo),
        (2, 80, 90.0, 299, 599, 1)
    );
    assert_eq!(
        (game.drops.len(), game.rockets.len(), game.balls.len()),
        (1, 1, 1)
    );
    assert!(game.balls[0].attached);
    assert_eq!(game.balls[0].launch_timer, 30);
    game.blocks = [0; 200];
    game.update(0);
    assert_eq!((game.mode, game.lives), (3, 3));
    game.update(4);
    assert_eq!(
        (
            game.level,
            game.paddle,
            game.last_paddle,
            game.width,
            game.ammo,
            game.long,
            game.ghost
        ),
        (2, 285.0, 285.0, 70.0, 0, 0, 0)
    );
    assert!(!game.lost_life);
    assert!(game.drops.is_empty());
    assert!(game.rockets.is_empty());
}

#[test]
fn timers_are_per_ball_and_manual_launch_does_not_relaunch_flying_balls() {
    let mut game = physics::Game::new();
    game.mode = 1;
    for _ in 0..10 {
        game.update(0);
    }
    game.collect(1);
    assert_eq!(
        (game.balls[0].launch_timer, game.balls[1].launch_timer),
        (20, 30)
    );
    assert!(game.balls[1].attached);
    game.balls.remove(0);
    gamepkg::samples(&[3, 4]);
    game.balls[0].launch();
    game.balls[0].launch();
    assert_eq!((game.balls[0].vx, game.balls[0].vy), (-2.0, -2.0));
    assert_eq!(
        physics::below(3),
        1,
        "flying ball consumed a second launch sample"
    );
    gamepkg::samples(&[0, 5]);
    assert_eq!(
        physics::below(3),
        2,
        "biased modulo sampling accepted the rejection tail"
    );
}

#[test]
fn victory_endless_progression_large_scores_and_simultaneous_final_loss_are_retained() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.level = 9;
    game.lost_life = true;
    game.lives = 2;
    game.blocks = [0; 200];
    game.update(0);
    assert_eq!((game.mode, game.lives), (5, 3));
    game.update(4);
    assert_eq!(game.level, 10);
    game.level = 999;
    game.blocks = [0; 200];
    game.update(0);
    game.update(4);
    assert_eq!(game.level, 1000);
    game.score = 9999999;
    game.award(5);
    assert_eq!(game.score, 10000004);
    game.mode = 1;
    game.lives = 1;
    game.blocks = [0; 200];
    game.balls.clear();
    game.update(0);
    assert_eq!((game.mode, game.lives), (3, 1));
}

#[test]
fn level_reset_matches_unchanged_original_with_items_and_high_level_numbers() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-transitions.json")).unwrap();
    for fixture in reference["resets"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.mode = 3;
        game.level = fixture["level"].as_u64().unwrap() as u32;
        game.score = 100;
        game.lives = 2;
        game.paddle = 50.0;
        game.width = 90.0;
        game.ammo = 3;
        game.long = 200;
        game.ghost = 300;
        game.lost_life = true;
        game.portal = true;
        game.portal_timer = 17;
        game.max_speed = 4000.0;
        game.drops.push(physics::Drop {
            x: 100.0,
            y: 300.0,
            kind: 0,
        });
        game.rockets.push((100.0, 300.0));
        game.particles.push(physics::Particle {
            x: 100.0,
            y: 300.0,
            vx: 1.0,
            vy: 1.0,
            life: 20,
            color: 0xffffffff,
        });
        game.update(4);
        assert_eq!(game.mode, 1);
        let ball = &game.balls[0];
        let actual = [
            game.level as f32,
            game.score as f32,
            game.lives as f32,
            game.paddle * 2.0,
            game.width * 2.0,
            game.ammo as f32,
            game.long as f32,
            game.ghost as f32,
            game.balls.len() as f32,
            (ball.x - 3.0) * 2.0,
            (ball.y - 63.0) * 2.0,
            ball.launch_timer as f32,
            game.drops.len() as f32,
            game.rockets.len() as f32,
            game.particles.len() as f32,
            if game.lost_life { 1.0 } else { 0.0 },
            if game.portal { 1.0 } else { 0.0 },
            game.portal_timer as f32,
            game.max_speed,
        ];
        assert_eq!(
            actual.as_slice(),
            fixture["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap() as f32)
                .collect::<Vec<_>>()
                .as_slice()
        );
    }
}

#[test]
fn special_bricks_and_rocket_phases_match_unchanged_original_with_real_sdl() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-bricks.json")).unwrap();
    for fixture in reference["fixtures"].as_array().unwrap() {
        let mut game = physics::Game::new();
        game.blocks = [0; 200];
        game.balls.clear();
        game.cooldown = 60;
        game.ghost = if fixture["ghost"].as_bool().unwrap_or(false) {
            600
        } else {
            0
        };
        for (index, block) in fixture["blocks"].as_array().unwrap().iter().enumerate() {
            assert!(game.set_block(index, block[2].as_u64().unwrap() as u8));
            game.block_x[index] = block[0].as_f64().unwrap() as f32 * 0.5;
            game.block_y[index] = block[1].as_f64().unwrap() as f32 * 0.5 + physics::TOP;
        }
        if let Some(rockets) = fixture["rocket"].as_array() {
            for rocket in rockets {
                game.rockets.push((
                    rocket[0].as_f64().unwrap() as f32 * 0.5,
                    rocket[1].as_f64().unwrap() as f32 * 0.5 + physics::TOP,
                ));
            }
        }
        gamepkg::samples(&[199; 100]); // Original oracle suppresses random bonus drops.
        for expected in fixture["expected"].as_array().unwrap() {
            if let Some(ball) = fixture["ball"].as_array() {
                game.balls.clear();
                game.balls.push(collision_ball(
                    ball[0].as_f64().unwrap() as f32 * 0.5 + 3.0,
                    ball[1].as_f64().unwrap() as f32 * 0.5 + 63.0,
                    ball[2].as_f64().unwrap() as f32 * 0.5,
                    ball[3].as_f64().unwrap() as f32 * 0.5,
                ));
                game.collide_blocks(0);
                let ball = &game.balls[0];
                let actual = [
                    (ball.x - 3.0) * 2.0,
                    (ball.y - 63.0) * 2.0,
                    ball.vx * 2.0,
                    ball.vy * 2.0,
                ];
                for (actual, expected) in actual.iter().zip(expected["ball"].as_array().unwrap()) {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32).abs() < 0.0001,
                        "{}",
                        fixture["name"]
                    );
                }
            } else {
                game.update_rockets();
            }
            assert_eq!(
                [
                    game.score,
                    game.rockets.len() as u32,
                    game.drops.len() as u32
                ],
                expected["state"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>()
                    .as_slice(),
                "{}",
                fixture["name"]
            );
            for (rocket, expected) in game
                .rockets
                .iter()
                .zip(expected["rockets"].as_array().unwrap())
            {
                assert_eq!(
                    [rocket.0 * 2.0, (rocket.1 - physics::TOP) * 2.0],
                    [
                        expected[0].as_f64().unwrap() as f32,
                        expected[1].as_f64().unwrap() as f32
                    ]
                );
            }
            for (index, expected) in expected["blocks"].as_array().unwrap().iter().enumerate() {
                assert_eq!(
                    [game.blocks[index], game.block_health[index]],
                    [
                        expected[0].as_u64().unwrap() as u8,
                        expected[1].as_u64().unwrap() as u8
                    ],
                    "{} block {index}",
                    fixture["name"]
                );
            }
        }
    }
}

#[test]
fn indirect_damage_never_drops_bonuses_or_consumes_drop_entropy() {
    for rocket in [false, true] {
        let mut game = physics::Game::new();
        game.blocks = [0; 200];
        game.cooldown = 60;
        for index in 0..4 {
            assert!(game.set_block(index, index as u8));
            game.block_x[index] = 100.0;
            game.block_y[index] = 100.0;
        }
        gamepkg::samples(&[100, 150]);
        game.explode(115.0, 105.0, rocket);
        assert_eq!(game.score, 40);
        assert!(game.drops.is_empty());
        assert_eq!(physics::below(100), 0);
        assert_eq!(game.block_health[1], 2);
        assert_eq!(game.block_health[3], 255);
    }
    let mut game = physics::Game::new();
    game.blocks = [0; 200];
    game.cooldown = 60;
    assert!(game.set_block(0, 1));
    gamepkg::samples(&[100]);
    assert!(!game.hit(0));
    assert_eq!(physics::below(100), 0);
    assert!(game.drops.is_empty());
    gamepkg::samples(&[100, 150]);
    assert!(game.hit(0));
    assert_eq!(game.drops.len(), 1);
    assert_eq!(game.drops[0].kind, 1);
    assert_eq!(game.cooldown, 0);
}

#[test]
fn metal_is_ignored_for_level_completion_but_consumed_by_portals() {
    let mut game = physics::Game::new();
    game.blocks = [0; 200];
    game.mode = 1;
    assert!(game.set_block(0, 3));
    game.update(0);
    assert_eq!(game.mode, 3);
    assert_eq!(game.blocks[0], 1);
    assert_eq!(game.score, 0);
    let mut game = physics::Game::new();
    game.blocks = [0; 200];
    game.mode = 1;
    game.portal = true;
    assert!(game.set_block(0, 3));
    game.block_x[0] = 305.0;
    game.block_y[0] = 235.0;
    game.pull_blocks();
    assert_eq!(game.blocks[0], 0);
    assert_eq!(game.score, 0);
    for _ in 0..270 {
        game.pull_blocks();
    }
    assert_eq!(game.mode, 3);
}

#[test]
fn invalid_editor_brick_types_and_indices_do_not_change_the_level() {
    let mut game = physics::Game::new();
    let blocks = game.blocks;
    let types = game.block_type;
    let health = game.block_health;
    assert!(!game.set_block(200, 0));
    assert!(!game.set_block(usize::MAX, 0));
    assert!(!game.set_block(0, 4));
    assert!(!game.set_block(0, 255));
    assert_eq!(game.blocks, blocks);
    assert_eq!(game.block_type, types);
    assert_eq!(game.block_health, health);
}

#[test]
fn scalar_pattern_rng_matches_pinned_original_rand_streams_and_distributions() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-levels.json")).unwrap();
    for fixture in reference["rng"].as_array().unwrap() {
        let seed = fixture["seed"].as_u64().unwrap();
        let mut rng = pattern_rng::Rng::new(seed);
        for expected in fixture["words"].as_array().unwrap() {
            assert_eq!(
                rng.next_u32(),
                expected.as_u64().unwrap() as u32,
                "seed {seed}"
            );
        }
        let mut rng = pattern_rng::Rng::new(seed);
        for (index, expected) in fixture["ranges"].as_array().unwrap().iter().enumerate() {
            let (low, high) = [(0, 12), (2, 5), (0, 3), (2, 6)][index % 4];
            assert_eq!(
                rng.range(low, high),
                expected.as_u64().unwrap(),
                "seed {seed}, range {index}"
            );
        }
        let mut rng = pattern_rng::Rng::new(seed);
        for (index, expected) in fixture["floats"].as_array().unwrap().iter().enumerate() {
            let (low, high) = [(0.6, 0.8), (-0.3, 0.3), (1.5, 3.0)][index % 3];
            assert_eq!(
                rng.float(low, high).to_bits(),
                expected.as_u64().unwrap() as u32,
                "seed {seed}, float {index}"
            );
        }
        let mut rng = pattern_rng::Rng::new(seed);
        for expected in fixture["units"].as_array().unwrap() {
            assert_eq!(
                rng.unit().to_bits(),
                expected.as_u64().unwrap() as u32,
                "seed {seed}"
            );
        }
        let mut rng = pattern_rng::Rng::new(seed);
        for expected in fixture["booleans"].as_array().unwrap() {
            assert_eq!(
                u8::from(rng.sixty_percent()),
                expected.as_u64().unwrap() as u8,
                "seed {seed}"
            );
        }
    }
}

#[test]
fn original_campaign_and_all_twelve_procedural_layouts_match_complete_generator() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-levels.json")).unwrap();
    assert_eq!(reference["canonical_pointer_bits"], 64);
    let mut kinds = std::collections::BTreeSet::new();
    let mut retries = 0;
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap() as u32;
        let actual = levels::generate(level);
        let expected = fixture["cells"].as_str().unwrap().as_bytes();
        assert_eq!(expected.len(), 200);
        for index in 0..200 {
            assert_eq!(
                actual[index],
                expected[index] - b'0',
                "level {level}, row {}, col {}",
                index / 20,
                index % 20
            );
        }
        if level >= 10 {
            kinds.insert(levels::Recipe::new(level, 0).kind);
        }
        if fixture["first_attempt_count"].as_u64().unwrap() < 20 {
            retries += 1;
            assert!(actual.iter().filter(|cell| **cell != 0).count() >= 20);
        }
    }
    assert_eq!(kinds.len(), 12);
    assert!(retries > 0, "no low-density retry was tested");
    gamepkg::samples(&[100]);
    let _ = levels::generate(1000);
    assert_eq!(
        physics::below(100),
        0,
        "layout generation consumed play randomness"
    );
}

#[test]
fn mouse_quantization_velocity_and_original_keyboard_actions_are_ordered() {
    let mut game = physics::Game::new();
    game.mode = 1;
    gamepkg::samples(&[3]);
    game.source_input(5, 0.0, 0.0, 44);
    let launch_x = game.balls[0].x;
    game.source_input(1, 500.4, 200.0, 0);
    assert_eq!(game.paddle, 465.0);
    game.update(0);
    assert_eq!(game.balls[0].x, launch_x - 2.0);
    assert_eq!(game.last_paddle, 465.0);
    game.source_input(1, 0.0, 200.0, 0);
    assert_eq!(game.paddle, 0.0);
    game.source_input(1, 640.0, 200.0, 0);
    assert_eq!(game.paddle, 570.0);
    game.source_input(5, 0.0, 0.0, 41);
    assert_eq!(game.mode, 2);
    let paddle = game.paddle;
    game.source_input(1, 320.0, 200.0, 0);
    assert_eq!(game.paddle, paddle);
    game.source_input(5, 0.0, 0.0, 41);
    assert_eq!(game.mode, 1);
    game.source_input(5, 0.0, 0.0, 20);
    game.source_input(9, 0.0, 0.0, 97);
    assert!(
        game.blocks.contains(&1),
        "physical Q with logical A must not trigger Q shortcut"
    );
    game.source_input(9, 0.0, 0.0, 113);
    game.update(0);
    assert_eq!(game.mode, 3);
    game.source_input(2, 100.0, 100.0, 1);
    assert_eq!(game.level, 2);
    game.mode = 2;
    game.source_input(9, 0.0, 0.0, 114);
    assert_eq!((game.mode, game.level, game.lives), (0, 1, 3));
}

#[test]
fn editor_actions_ascii_and_loaded_health_match_the_complete_unchanged_original_module() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-editor.json")).unwrap();
    let mut edit = editor::Editor::new();
    for step in reference["steps"].as_array().unwrap() {
        let op = step["operation"].as_array().unwrap();
        let action = op[0].as_str().unwrap();
        let x = op[1].as_f64().unwrap() as f32 * 0.5;
        let y = op[2].as_f64().unwrap() as f32 * 0.5 + 60.0;
        match action {
            "select" => edit.selected = op[1].as_u64().unwrap() as u8,
            "add" => edit.editor_paint(x, y, false),
            "left" => {
                edit.editor_event(2, x, y, 1);
            }
            "right" => {
                edit.editor_event(2, x, y, 2);
            }
            "move" => {
                edit.editor_event(1, x, y, 0);
            }
            "stop" => {
                edit.editor_event(3, 0.0, 0.0, 1);
            }
            _ => panic!("unknown oracle operation"),
        }
        for index in 0..200 {
            assert_eq!(
                edit.cells[index],
                step["cells"][index].as_u64().unwrap() as u8,
                "{action}, cell {index}"
            );
            if edit.cells[index] != 255 {
                assert_eq!(
                    edit.health[index],
                    step["health"][index].as_u64().unwrap() as u8
                );
            }
        }
    }
    edit.name = "example".into();
    assert!(edit.editor_store());
    assert_eq!(
        edit.patterns[0].ascii_write(),
        reference["ascii_saved"].as_str().unwrap()
    );
    edit.selection = 0;
    edit.editor_load();
    for index in 0..200 {
        assert_eq!(
            edit.cells[index],
            reference["loaded"]["cells"][index].as_u64().unwrap() as u8
        );
        if edit.cells[index] != 255 {
            assert_eq!(
                edit.health[index],
                reference["loaded"]["health"][index].as_u64().unwrap() as u8
            );
        }
    }
    edit.editor_clear();
    assert_eq!(
        edit.confirm,
        reference["confirmations"][0].as_bool().unwrap()
    );
    edit.editor_cancel();
    assert_eq!(
        edit.confirm,
        reference["confirmations"][1].as_bool().unwrap()
    );
    edit.editor_clear();
    edit.editor_clear();
    assert_eq!(edit.cells, [255; 200]);
    edit.naming = true;
    edit.name = "café😀".into();
    for text in reference["backspaces"].as_array().unwrap() {
        edit.editor_key(8);
        assert_eq!(edit.name, text.as_str().unwrap());
    }
    for (index, background) in reference["backgrounds"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        edit.editor_event(2, if index < 8 { 600.0 } else { 530.0 }, 370.0, 1);
        assert_eq!(edit.background, background.as_u64().unwrap() as u32);
    }
    for fixture in reference["imports"].as_array().unwrap() {
        let pattern = editor::Pattern::ascii_read("example", fixture["source"].as_str().unwrap());
        for index in 0..200 {
            assert_eq!(
                pattern.cells[index],
                fixture["cells"][index].as_u64().unwrap() as u8
            );
        }
    }
}

#[test]
fn editor_archive_migrates_legacy_and_rejects_damage_without_partial_replacement() {
    let mut edit = editor::Editor::new();
    edit.name = "../../café😀".into();
    edit.selected = 6;
    edit.editor_paint(20.0, 100.0, false);
    assert!(edit.editor_store());
    edit.name = "other".into();
    assert!(edit.editor_store());
    let bytes = edit.save_encode(u32::MAX, u32::MAX, true);
    assert_eq!(&bytes[..4], b"ARK2");
    let mut loaded = editor::Editor::new();
    assert_eq!(loaded.save_decode(&bytes), Some([u32::MAX, u32::MAX, 1]));
    assert_eq!(loaded.patterns.len(), 2);
    assert_eq!(loaded.patterns[0].name, "../../café😀");
    assert_eq!(loaded.patterns[0].cells[0], 6);
    for end in 0..bytes.len() {
        assert!(
            loaded.save_decode(&bytes[..end]).is_none(),
            "accepted truncation {end}"
        );
        assert_eq!(loaded.patterns.len(), 2);
    }
    let mut bad = bytes.clone();
    bad.extend([0]);
    assert!(loaded.save_decode(&bad).is_none());
    for offset in [16, 20] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(loaded.save_decode(&bad).is_none());
    }
    let mut bad = bytes.clone();
    bad[24] = 255;
    assert!(loaded.save_decode(&bad).is_none());
    let mut bad = bytes.clone();
    bad[24 + loaded.patterns[0].name.len()] = 9;
    assert!(loaded.save_decode(&bad).is_none());
    let mut legacy = b"ARK1".to_vec();
    legacy.extend(u32::MAX.to_le_bytes());
    legacy.extend(1001u32.to_le_bytes());
    legacy.extend(1u32.to_le_bytes());
    assert_eq!(
        editor::Editor::new().save_decode(&legacy),
        Some([u32::MAX, 1001, 1])
    );
    loaded.readable = false;
    let before = loaded.save_encode(1, 2, false);
    assert!(!loaded.editor_store());
    assert_eq!(before, loaded.save_encode(1, 2, false));
}

#[test]
fn editor_focus_reset_clear_confirmation_save_bounds_and_test_return_preserve_the_board() {
    let mut game = physics::Game::new();
    game.source_input(9, 0.0, 0.0, 101);
    assert_eq!(game.mode, 6);
    game.source_input(9, 0.0, 0.0, 54);
    game.source_input(2, 20.0, 100.0, 1);
    game.source_input(1, 50.0, 100.0, 1);
    assert_eq!(&game.editor.cells[..3], &[6, 6, 255]);
    game.source_input(0, 0.0, 0.0, 0);
    game.source_input(1, 80.0, 100.0, 1);
    assert_eq!(game.editor.cells[2], 255);
    game.source_input(9, 0.0, 0.0, 116);
    assert_eq!(game.mode, 1);
    assert!(game.testing);
    assert_eq!(game.block_type[0], 1);
    assert_eq!(game.block_health[0], 2);
    game.source_input(9, 0.0, 0.0, 113);
    game.update(0);
    assert_eq!(game.mode, 6);
    assert!(!game.testing);
    assert_eq!(game.editor.cells[0], 6);
    game.source_input(9, 0.0, 0.0, 99);
    assert!(game.editor.confirm);
    game.source_input(9, 0.0, 0.0, 55);
    assert!(!game.editor.confirm);
    game.source_input(9, 0.0, 0.0, 99);
    game.source_input(9, 0.0, 0.0, 99);
    assert_eq!(game.editor.cells, [255; 200]);
    let mut edit = editor::Editor::new();
    assert!(!edit.editor_store());
    edit.cells[0] = 1;
    edit.name = "x".repeat(editor::SAVE_BYTES - 224);
    assert!(edit.editor_store());
    assert_eq!(edit.save_encode(0, 1, false).len(), editor::SAVE_BYTES);
    edit.name.push('x');
    assert!(!edit.editor_store());
    assert_eq!(edit.patterns.len(), 1);
    edit.name = "z".into();
    assert!(!edit.editor_store());
    assert_eq!(edit.patterns.len(), 1);
    edit.naming = true;
    edit.editor_event(7, 0.0, 0.0, 0x1f600);
    assert_eq!(edit.name, "z😀");
    edit.editor_key(8);
    assert_eq!(edit.name, "z");
}

#[test]
fn custom_endless_pool_matches_unchanged_original_generator_and_pattern_converter() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-custom-levels.json")).unwrap();
    let mut patterns = Vec::new();
    for (index, cells) in reference["patterns"].as_array().unwrap().iter().enumerate() {
        let mut board = [255; 200];
        for i in 0..200 {
            board[i] = cells[i].as_u64().unwrap() as u8;
        }
        patterns.push(editor::Pattern {
            name: format!("custom{index}"),
            cells: board,
        });
    }
    let mut kinds = std::collections::BTreeSet::new();
    let mut game = physics::Game::new();
    game.editor.patterns = patterns;
    for fixture in reference["layouts"].as_array().unwrap() {
        let level = fixture["level"].as_u64().unwrap() as u32;
        game.level = level;
        game.populate();
        kinds.insert(levels::Recipe::with_patterns(level, 0, 3).kind);
        let actual = levels::generate_cells(level, &game.editor.patterns);
        for (i, cell) in actual.iter().enumerate() {
            let expected = fixture["cells"][i].as_u64().unwrap() as u8;
            assert_eq!(*cell, expected, "level {level}, cell {i}");
            assert_eq!(game.blocks[i] != 0, expected != 255);
            if expected != 255 {
                assert_eq!(game.block_color[i], if expected < 6 { expected } else { 0 });
                assert_eq!(
                    game.block_type[i],
                    if expected < 6 { 0 } else { expected - 5 }
                );
                assert_eq!(
                    game.block_health[i],
                    if expected == 6 {
                        2
                    } else if expected == 8 {
                        255
                    } else {
                        1
                    }
                );
            }
        }
    }
    assert_eq!(
        kinds.len(),
        15,
        "all twelve procedures and three custom selections must be covered"
    );
}

#[test]
fn leaving_a_paused_editor_test_cannot_turn_a_later_campaign_into_test_play() {
    let mut game = physics::Game::new();
    game.source_input(9, 0.0, 0.0, 101);
    game.editor.cells[0] = 0;
    game.source_input(9, 0.0, 0.0, 116);
    assert!(game.testing);
    game.source_input(5, 0.0, 0.0, 41);
    assert_eq!(game.mode, 2);
    game.source_input(9, 0.0, 0.0, 101);
    assert_eq!(game.mode, 6);
    assert!(!game.testing);
    game.source_input(9, 0.0, 0.0, 27);
    assert_eq!(game.mode, 0);
    game.update(4);
    game.blocks = [0; 200];
    game.update(0);
    assert_eq!(game.mode, 3);
}

#[test]
fn editor_middle_button_release_does_not_cancel_a_left_drag() {
    let mut edit = editor::Editor::new();
    edit.editor_event(2, 20.0, 100.0, 1);
    edit.editor_event(3, 20.0, 100.0, 4);
    edit.editor_event(1, 50.0, 100.0, 1);
    assert_eq!(&edit.cells[..3], &[0, 0, 255]);
    edit.editor_event(3, 50.0, 100.0, 1);
    edit.editor_event(1, 80.0, 100.0, 0);
    assert_eq!(edit.cells[2], 255);
}

#[test]
fn editor_toolbar_remains_usable_while_the_pattern_name_is_being_edited() {
    let mut edit = editor::Editor::new();
    edit.cells[0] = 3;
    edit.naming = true;
    edit.editor_key(115);
    assert!(
        edit.patterns.is_empty(),
        "typing S must not trigger its shortcut"
    );
    assert_eq!(edit.editor_event(2, 150.0, 400.0, 1), 0);
    assert_eq!(edit.patterns.len(), 1);
    assert!(edit.naming);
    assert_eq!(
        edit.editor_event(2, 320.0, 400.0, 1),
        2,
        "the Test button must work during name editing"
    );
    edit.editor_event(2, 235.0, 400.0, 1);
    assert!(edit.confirm);
    edit.editor_event(2, 235.0, 400.0, 1);
    assert_eq!(edit.cells, [255; 200]);
}

#[test]
fn failed_editor_saves_roll_back_the_browser_collection_without_losing_the_working_board() {
    let mut edit = editor::Editor::new();
    edit.cells[0] = 6;
    edit.editor_store();
    edit.editor_commit(false);
    assert!(edit.patterns.is_empty());
    assert_eq!(edit.cells[0], 6);
    assert!(!edit.save_requested);
    edit.editor_store();
    edit.editor_commit(true);
    assert_eq!(edit.patterns.len(), 1);
    edit.cells[0] = 8;
    edit.editor_store();
    edit.editor_commit(false);
    assert_eq!(edit.patterns[0].cells[0], 6);
    assert_eq!(edit.cells[0], 8);
    edit.name = "other".into();
    edit.editor_store();
    edit.editor_commit(false);
    assert_eq!(edit.patterns.len(), 1);
    assert_eq!(edit.name, "other");
}

#[test]
fn campaign_restart_keeps_saved_patterns_and_gravity_preferences() {
    let mut game = physics::Game::new();
    game.editor.cells[0] = 7;
    game.editor.editor_store();
    game.editor.editor_commit(true);
    game.gravity = true;
    game.best = 500;
    game.unlocked = 12;
    game.mode = 4;
    game.update(4);
    assert_eq!(game.mode, 1);
    assert_eq!(game.editor.patterns.len(), 1);
    assert_eq!(game.editor.patterns[0].cells[0], 7);
    assert!(game.gravity);
    assert_eq!((game.best, game.unlocked), (500, 12));
}

#[test]
fn editor_file_commands_only_queue_intent_until_the_host_binding_accepts() {
    let mut game = physics::Game::new();
    game.source_input(9, 0.0, 0.0, 101);
    game.source_input(9, 0.0, 0.0, 105);
    assert_eq!(game.editor.file_requested, 1);
    assert_eq!(game.editor.file_handle, None);
    assert_eq!(game.editor.file_operation, 0);
    game.source_input(9, 0.0, 0.0, 120);
    assert_eq!(game.editor.file_requested, 2);
    game.editor.file_requested = 0;
    game.editor.naming = true;
    game.source_input(9, 0.0, 0.0, 105);
    assert_eq!(game.editor.file_requested, 0, "typing I started an import");
}

#[test]
fn window_controls_follow_original_gameplay_pause_editor_and_quit_bindings() {
    let mut game = physics::Game::new();
    assert!(!game.cursor_confined());
    for mode in 0..7 {
        game.mode = mode;
        assert_eq!(game.cursor_confined(), matches!(mode, 1 | 3 | 4 | 5));
    }
    game.mode = 6;
    game.editor.naming = true;
    game.source_input(5, 0.0, 0.0, 68);
    assert_eq!(game.fullscreen_requests, 1);
    game.source_input(5, 0.0, 0.0, 68 | 65536);
    assert_eq!(
        game.fullscreen_requests, 1,
        "OS repeat retriggered fullscreen"
    );
    game.mode = 2;
    game.source_input(9, 0.0, 0.0, 114);
    assert_eq!(
        game.fullscreen_requests, 1,
        "restart discarded an earlier ordered fullscreen key"
    );
    for mode in [2, 4, 5] {
        game.mode = mode;
        game.quit_requested = false;
        game.source_input(9, 0.0, 0.0, 113);
        assert!(game.quit_requested);
        let before = game.mode;
        game.source_input(9, 0.0, 0.0, 114);
        game.update(4);
        assert_eq!(game.mode, before, "events after Quit still ran");
    }
    let mut game = physics::Game::new();
    game.mode = 1;
    game.source_input(9, 0.0, 0.0, 113);
    assert!(!game.quit_requested);
    assert_eq!(game.blocks, [0; 200]);
}

#[test]
fn main_menu_primary_actions_match_unchanged_original_mouse_rectangles() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-menu.json")).unwrap();
    let mut controls = menu::Menu::new();
    for case in reference["cases"].as_array().unwrap() {
        controls.settings = case["settings"].as_bool().unwrap();
        let x = case["point"][0].as_f64().unwrap() as f32;
        let y = case["point"][1].as_f64().unwrap() as f32;
        let expected = case["action"].as_u64().unwrap() as u32;
        assert_eq!(controls.hit(x, y), expected, "{case}");
        assert_eq!(controls.menu_event(1, x, y, 0), 0);
        assert_eq!(controls.hover, expected);
        assert_eq!(
            controls.menu_event(2, x, y, 2),
            0,
            "right click activated menu"
        );
        assert_eq!(controls.menu_event(2, x, y, 1), expected);
        controls.menu_event(0, 0.0, 0.0, 0);
        assert_eq!(controls.hover, 0);
    }
    for (x, y) in [(f32::NAN, 200.0), (320.0, f32::INFINITY)] {
        assert_eq!(controls.hit(x, y), 0);
    }
}
#[test]
fn menu_actions_preserve_pause_state_and_apply_controls_without_playing_behind_settings() {
    let mut game = physics::Game::new();
    game.unlocked = 9;
    game.source_input(2, 320.0, 185.0, 1);
    assert_eq!(
        (game.mode, game.level),
        (1, 1),
        "New Game skipped original campaign levels"
    );
    game.score = 321;
    game.balls[0].x = 123.0;
    game.mode = 2;
    game.source_input(2, 320.0, 285.0, 1);
    assert!(game.menu.settings);
    let frame = game.frame;
    game.update(0);
    assert_eq!(game.frame, frame);
    game.source_input(2, 320.0, 270.0, 1);
    assert_eq!(game.fullscreen_requests, 1);
    assert!(game.menu.settings);
    assert_eq!(game.mode, 2);
    game.source_input(2, 320.0, 450.0, 1);
    assert!(!game.menu.settings);
    game.source_input(2, 320.0, 235.0, 1);
    assert!(game.gravity && game.dirty);
    game.source_input(2, 320.0, 185.0, 1);
    assert_eq!((game.mode, game.score, game.balls[0].x), (1, 321, 123.0));
    game.mode = 2;
    game.source_input(2, 320.0, 260.0, 1);
    assert_eq!(game.mode, 6);
    game.source_input(9, 0.0, 0.0, 27);
    assert_eq!(game.mode, 0);
    assert!(!game.menu.settings);
    game.score = 999;
    game.lives = 1;
    game.ammo = 20;
    game.long = 10;
    game.portal = true;
    game.source_input(2, 320.0, 185.0, 1);
    assert_eq!(
        (game.mode, game.score, game.lives, game.ammo, game.long),
        (1, 0, 3, 0, 0)
    );
    assert!(!game.portal);
    assert_eq!(game.unlocked, 9);
    assert!(game.gravity);
    game.mode = 2;
    game.source_input(2, 320.0, 210.0, 1);
    assert_eq!((game.mode, game.level, game.score), (0, 1, 0));
    assert_eq!(game.unlocked, 9);
    assert!(game.gravity);
    game.source_input(2, 320.0, 310.0, 1);
    assert!(game.quit_requested);
    game.source_input(2, 320.0, 185.0, 1);
    game.update(4);
    assert_eq!(game.mode, 0);
    let mut title = physics::Game::new();
    title.score = 99;
    title.menu.settings = true;
    title.source_input(9, 0.0, 0.0, 114);
    assert_eq!(title.score, 0);
    assert!(!title.menu.settings);
    title.source_input(9, 0.0, 0.0, 113);
    assert!(title.quit_requested);
}

#[test]
fn effects_slider_matches_original_motion_drag_quantization_and_release() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-menu.json")).unwrap();
    let mut controls = menu::Menu::new();
    controls.settings = true;
    assert_eq!(controls.sfx_volume, 64);
    assert!(!controls.sfx_muted);
    assert!(!controls.save_failed);
    for case in reference["slider"].as_array().unwrap() {
        let e = &case["event"];
        controls.menu_event(
            e[0].as_u64().unwrap() as u32,
            e[1].as_f64().unwrap() as f32,
            e[2].as_f64().unwrap() as f32,
            e[3].as_u64().unwrap() as u32,
        );
        assert_eq!(
            controls.sfx_volume,
            case["volume"].as_u64().unwrap() as u32,
            "{case}"
        );
    }
    controls.menu_event(2, 320.0, 245.0, 1);
    controls.menu_event(1, 300.0, 245.0, 1);
    assert!(controls.dragging_volume);
    controls.menu_event(0, 0.0, 0.0, 0);
    let volume = controls.sfx_volume;
    controls.menu_event(1, 370.0, 245.0, 1);
    assert_eq!(controls.sfx_volume, volume, "focus reset left a live drag");
    controls.menu_event(2, 320.0, 245.0, 1);
    controls.menu_event(1, 300.0, 245.0, 1);
    controls.menu_event(1, f32::NAN, 245.0, 1);
    assert!(!controls.dragging_volume);
    let mut game = physics::Game::new();
    game.menu.settings = true;
    game.source_input(2, 320.0, 225.0, 1);
    assert!(game.menu.sfx_muted && game.dirty);
    game.source_input(2, 320.0, 245.0, 1);
    game.source_input(1, 295.0, 245.0, 1);
    assert_eq!(game.menu.sfx_volume, 32);
    assert!(game.menu.sfx_muted, "moving a muted slider unmuted effects");
    game.reset_run();
    assert_eq!(game.menu.sfx_volume, 32);
    assert!(game.menu.sfx_muted);
}
#[test]
fn synthesized_effect_gain_mute_and_phase_are_bounded_and_word_size_independent() {
    for kind in 1..=4 {
        let period = match kind {
            1 => 100,
            2 => 64,
            3 => 48,
            _ => 180,
        };
        for frame in [0, 1, 99, 123456789, u32::MAX] {
            let normal = audio::effect_samples(kind, frame, 64, false);
            let quiet = audio::effect_samples(kind, frame, 32, false);
            let loud = audio::effect_samples(kind, frame, 128, false);
            for index in 0..800 {
                let sign = if (u64::from(frame) + index as u64) % period < period / 2 {
                    1
                } else {
                    -1
                };
                let original = (sign * (800 - index) as i32 * 5) as i16;
                assert_eq!(normal[index], original);
                assert_eq!(quiet[index], original / 2);
                assert_eq!(loud[index], original * 2);
            }
            assert_eq!(audio::effect_samples(kind, frame, u32::MAX, false), loud);
            assert_eq!(audio::effect_samples(kind, frame, 0, false), [0; 800]);
            assert_eq!(audio::effect_samples(kind, frame, 128, true), [0; 800]);
        }
    }
}
#[test]
fn effects_archive_migrates_old_formats_preserves_capacity_and_rejects_bad_settings() {
    let mut edit = editor::Editor::new();
    edit.cells[0] = 7;
    edit.name = "café".into();
    assert!(edit.editor_store());
    let legacy = edit.save_encode(123, 999, true);
    let mut loaded = editor::Editor::new();
    for volume in 0..=128 {
        for muted in [false, true] {
            let bytes = edit.save_encode_settings(123, 999, true, volume, muted);
            assert_eq!(&bytes[..4], b"ARK3");
            assert_eq!(bytes.len(), legacy.len());
            assert_eq!(loaded.save_decode(&bytes), Some([123, 999, 1]));
            assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (volume, muted));
            assert_eq!(loaded.patterns[0].cells[0], 7);
        }
    }
    assert_eq!(loaded.save_decode(&legacy), Some([123, 999, 1]));
    assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (64, false));
    let mut ark1 = b"ARK1".to_vec();
    for value in [123u32, 999, 1] {
        ark1.extend(value.to_le_bytes());
    }
    assert_eq!(loaded.save_decode(&ark1), Some([123, 999, 1]));
    assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (64, false));
    let valid = edit.save_encode_settings(123, 999, true, 17, true);
    loaded.save_decode(&valid).unwrap();
    for invalid_flags in (10..32)
        .map(|bit| 1u32 << bit)
        .chain((129..=255).map(|value| value << 1))
    {
        let mut bad = valid.clone();
        bad[12..16].copy_from_slice(&invalid_flags.to_le_bytes());
        assert!(loaded.save_decode(&bad).is_none());
        assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (17, true));
        assert_eq!(loaded.patterns[0].cells[0], 7);
    }
    for end in 0..valid.len() {
        assert!(loaded.save_decode(&valid[..end]).is_none());
        assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (17, true));
    }
    let mut future = valid.clone();
    future[3] = b'4';
    assert!(loaded.save_decode(&future).is_none());
    let mut full = editor::Editor::new();
    full.name = "x".repeat(editor::SAVE_BYTES - 224);
    full.cells[0] = 6;
    assert!(full.editor_store());
    let old = full.save_encode(1, 2, false);
    let new = full.save_encode_settings(1, 2, false, 0, true);
    assert_eq!(old.len(), editor::SAVE_BYTES);
    assert_eq!(new.len(), editor::SAVE_BYTES);
    assert_eq!(loaded.save_decode(&old), Some([1, 2, 0]));
    assert_eq!(loaded.save_decode(&new), Some([1, 2, 0]));
    assert_eq!((loaded.sfx_volume, loaded.sfx_muted), (0, true));
    assert_eq!(loaded.patterns[0].name.len(), editor::SAVE_BYTES - 224);
}
