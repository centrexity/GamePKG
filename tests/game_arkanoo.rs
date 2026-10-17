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
