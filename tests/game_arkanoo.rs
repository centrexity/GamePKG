#[path = "support/arkanoo_canonical.rs"]
mod canonical;
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
#[path = "../ports/arkanoo/source/src/background.rs"]
mod background;
#[path = "../ports/arkanoo/source/src/editor.rs"]
mod editor;
#[path = "../ports/arkanoo/source/src/levels.rs"]
mod levels;
#[path = "../ports/arkanoo/source/src/menu.rs"]
mod menu;
#[path = "../ports/arkanoo/source/src/pattern_rng.rs"]
mod pattern_rng;
#[path = "../ports/arkanoo/source/src/physics.rs"]
// Guest source cannot use vec!; its bounded allocation uses capacity + resize.
#[allow(clippy::slow_vector_initialization)]
mod physics;
#[path = "../ports/arkanoo/source/src/procedural.rs"]
mod procedural;
fn menu_game() -> physics::Game {
    let mut game = physics::Game::new();
    game.source_input(5, 0.0, 0.0, 40);
    game
}
#[test]
fn penguin_life_loss_grab_clear_flight_and_gameover_freeze_match_original() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-penguin.json")).unwrap();
    for trace in fixture["traces"].as_array().unwrap() {
        let mut game = menu_game();
        game.mode = 1;
        game.lives = trace["lives"].as_u64().unwrap() as u32;
        game.score = 100;
        game.balls.clear();
        game.update(0);
        let loss = trace["loss"].as_array().unwrap();
        assert_eq!(
            [game.lives, game.score, game.balls.len() as u32, game.mode],
            [
                loss[0].as_u64().unwrap() as u32,
                loss[1].as_u64().unwrap() as u32,
                loss[2].as_u64().unwrap() as u32,
                loss[3].as_u64().unwrap() as u32
            ]
        );
        for (index, frame) in trace["frames"].as_array().unwrap().iter().enumerate() {
            if index > 0 {
                if let Some(ball) = game.balls.first_mut() {
                    ball.launch_timer = u32::MAX;
                }
                game.update(0);
            }
            if frame[2].as_f64().unwrap() == 4.0 {
                assert!(game.penguin.is_none());
                continue;
            }
            let p = game.penguin.as_ref().unwrap();
            for (actual, expected) in [
                p.x,
                p.y,
                p.state as f32,
                p.frame_count as f32,
                if game.stolen_heart.is_some() {
                    1.0
                } else {
                    0.0
                },
            ]
            .iter()
            .zip(frame.as_array().unwrap())
            {
                assert!(
                    (*actual - expected.as_f64().unwrap() as f32).abs() < 0.0001,
                    "life {} frame {index}",
                    trace["lives"]
                );
            }
        }
    }
}
#[test]
fn original_penguin_rectangle_art_and_fallback_heart_points_are_preserved() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-penguin.json")).unwrap();
    for case in fixture["drawings"].as_array().unwrap() {
        let p = physics::Penguin {
            x: 1200.75,
            y: 25.25,
            target_x: 1200.0,
            target_y: 25.0,
            state: case["state"].as_u64().unwrap() as u32,
            frame_count: case["frame"].as_u64().unwrap() as u32,
        };
        let shapes = physics::penguin_rects(&p);
        let expected = case["rects"].as_array().unwrap();
        assert_eq!(shapes.len(), expected.len());
        for (shape, expected) in shapes.iter().zip(expected) {
            assert_eq!(
                [
                    i64::from(shape.x),
                    i64::from(shape.y),
                    i64::from(shape.width),
                    i64::from(shape.height),
                    i64::from(shape.color)
                ],
                [
                    expected[0].as_i64().unwrap(),
                    expected[1].as_i64().unwrap(),
                    expected[2].as_i64().unwrap(),
                    expected[3].as_i64().unwrap(),
                    expected[4].as_i64().unwrap()
                ]
            );
        }
    }
    for case in fixture["hearts"].as_array().unwrap() {
        let dots = physics::heart_points(
            case["x"].as_i64().unwrap() as i32,
            case["y"].as_i64().unwrap() as i32,
            12,
        );
        let expected = case["points"].as_array().unwrap();
        assert_eq!(dots.len(), expected.len());
        for (dot, expected) in dots.iter().zip(expected) {
            assert_eq!(
                [i64::from(dot.x), i64::from(dot.y), i64::from(dot.color)],
                [
                    expected[0].as_i64().unwrap(),
                    expected[1].as_i64().unwrap(),
                    expected[2].as_i64().unwrap()
                ]
            );
        }
    }
    assert_eq!(
        gamepkg_host_heart_hash(),
        "86b8ef21b5544f39542c247518155f4a856aab70851bbd76cfb67df4bd0fbd71"
    );
}
fn gamepkg_host_heart_hash() -> String {
    ::gamepkg::verify::digest(include_bytes!("../ports/arkanoo/assets/heart.png"))
}
#[test]
fn penguin_pause_replacement_level_restart_and_editor_test_follow_original_boundaries() {
    let mut g = menu_game();
    g.mode = 1;
    g.balls.clear();
    g.update(0);
    for mode in [2, 6, 4] {
        g.mode = mode;
        let p = g.penguin.as_ref().unwrap();
        let old = [p.x, p.y, p.frame_count as f32];
        g.update(0);
        let p = g.penguin.as_ref().unwrap();
        assert_eq!([p.x, p.y, p.frame_count as f32], old);
    }
    g.mode = 1;
    g.balls.clear();
    g.update(0);
    assert_eq!(g.lives, 1);
    let p = g.penguin.as_ref().unwrap();
    assert_eq!(
        (p.x, p.y, p.target_x, p.frame_count),
        (1230.0, 620.0, 1225.0, 0)
    );
    g.attach();
    assert!(g.penguin.is_none() && g.stolen_heart.is_none());
    g.balls.clear();
    g.update(0);
    assert!(g.penguin.is_some());
    g.reset_run();
    assert!(g.penguin.is_none() && g.stolen_heart.is_none());
    g.penguin = Some(physics::Penguin::new(1200.0, 25.0));
    g.stolen_heart = Some([1200.0, 25.0]);
    g.editor_test();
    assert!(g.penguin.is_none() && g.stolen_heart.is_none());
}
#[test]
fn ball_trail_thresholds_history_and_prephysics_timing_match_original() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-trails.json")).unwrap();
    let mut b = physics::Ball::new(53.0, 163.0);
    b.attached = false;
    b.launch_timer = 100;
    for stage in fixture["trace"].as_array().unwrap() {
        let a = stage["action"].as_array().unwrap();
        b.vx = a[0].as_f64().unwrap() as f32 * 0.5;
        b.vy = a[1].as_f64().unwrap() as f32 * 0.5;
        b.spin = a[2].as_f64().unwrap() as f32 * 0.5;
        b.attached = a[4].as_bool().unwrap();
        for expected in stage["history"].as_array().unwrap() {
            if b.attached {
                b.launch_timer -= 1;
            } else {
                b.advance(false);
            }
            let expected = expected.as_array().unwrap();
            assert_eq!(b.trail.len(), expected.len());
            for (point, expected) in b.trail.iter().zip(expected) {
                assert!((point[0] - expected[0].as_f64().unwrap() as f32).abs() < 0.002);
                assert!((point[1] - expected[1].as_f64().unwrap() as f32).abs() < 0.002);
            }
        }
    }
}
#[test]
fn animation_clock_continues_past_32_bit_frames_without_changing_word_size() {
    let mut game = menu_game();
    game.mode = 1;
    game.frame = u64::from(u32::MAX);
    game.update(0);
    assert_eq!(game.frame, u64::from(u32::MAX) + 1);
    game.mode = 2;
    game.update(0);
    assert_eq!(game.frame, u64::from(u32::MAX) + 1);
    game.mode = 1;
    game.frame = u64::MAX;
    game.update(0);
    assert_eq!(game.frame, 0);
}
#[test]
fn original_trail_comet_pulse_ring_and_ordered_colors_match_all_recorded_points() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-trails.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let a = case["ball"].as_array().unwrap();
        let mut b = physics::Ball::new(
            a[0].as_f64().unwrap() as f32 * 0.5 + 3.0,
            a[1].as_f64().unwrap() as f32 * 0.5 + 63.0,
        );
        b.vx = a[2].as_f64().unwrap() as f32 * 0.5;
        b.vy = a[3].as_f64().unwrap() as f32 * 0.5;
        for point in case["trail"].as_array().unwrap() {
            b.trail.push([
                point[0].as_f64().unwrap() as f32,
                point[1].as_f64().unwrap() as f32,
            ]);
        }
        let actual = physics::ball_effect_points(&b, case["frame"].as_u64().unwrap());
        let expected = case["points"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (dot, expected) in actual.iter().zip(expected) {
            assert_eq!(
                [i64::from(dot.x), i64::from(dot.y), i64::from(dot.color)],
                [
                    expected[0].as_i64().unwrap(),
                    expected[1].as_i64().unwrap(),
                    expected[2].as_i64().unwrap()
                ]
            );
        }
        let image = physics::effect_image(&actual);
        assert_eq!(
            image.is_some(),
            actual
                .iter()
                .any(|p| (0..1280).contains(&p.x) && (0..720).contains(&p.y) && p.color & 255 != 0)
        );
    }
}
#[test]
fn particle_lifetimes_rotations_alpha_and_ordered_shard_points_match_original() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-particles.json")).unwrap();
    gamepkg::samples(&[(1u64 << 62) | (1u64 << 38)]);
    let constructed = physics::Particle::new(6.25, 58.375, 1.0, -2.0, 0x1964c8ff);
    let expected = fixture["constructor"].as_array().unwrap();
    assert_eq!(
        [
            constructed.rotation,
            constructed.rotation_speed,
            constructed.max_life as f32,
            constructed.size as f32
        ],
        [
            expected[0].as_f64().unwrap() as f32,
            expected[1].as_f64().unwrap() as f32,
            expected[2].as_f64().unwrap() as f32,
            expected[3].as_f64().unwrap() as f32
        ]
    );
    for case in fixture["cases"].as_array().unwrap() {
        let initial: Vec<f32> = case["initial"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap() as f32)
            .collect();
        let mut p = physics::Particle {
            x: initial[0],
            y: initial[1],
            vx: initial[2],
            vy: initial[3],
            rotation: initial[4],
            rotation_speed: initial[5],
            life: initial[6] as u32,
            max_life: initial[6] as u32,
            size: initial[7] as i32,
            color: 0x1964c8ff,
        };
        for frame in case["frames"].as_array().unwrap() {
            let expected = frame["state"].as_array().unwrap();
            for (actual, expected) in [p.x, p.y, p.vy, p.rotation].iter().zip(expected) {
                assert!((*actual - expected.as_f64().unwrap() as f32).abs() < 0.0001);
            }
            assert_eq!(u64::from(p.alpha()), expected[4].as_f64().unwrap() as u64);
            assert_eq!(p.life > 0, expected[5].as_f64().unwrap() == 1.0);
            let points: Vec<[i32; 2]> = frame["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| [p[0].as_i64().unwrap() as i32, p[1].as_i64().unwrap() as i32])
                .collect();
            assert_eq!(
                p.shard_coordinates(),
                points,
                "integer rotation, duplicates and point order changed"
            );
            p.advance();
        }
    }
}
#[test]
fn particle_batch_clips_composites_and_retains_large_bursts_without_draw_caps() {
    let mut game = menu_game();
    for _ in 0..200 {
        game.burst(320.0, 240.0, 0xff0000ff);
    }
    assert!((2000..=3000).contains(&game.particles.len()));
    let image = physics::particle_image(&game.particles).unwrap();
    assert_eq!(
        (image.left, image.top, image.width, image.height),
        (635, 355, 11, 11)
    );
    assert_eq!(image.pixels.len(), 11 * 11 * 4);
    assert!(image.pixels.chunks_exact(4).any(|p| p == [255, 0, 0, 255]));
    for p in &mut game.particles {
        p.x = -100.0;
        p.y = -100.0;
    }
    assert!(physics::particle_image(&game.particles).is_none());
    assert!(physics::particle_image(&[]).is_none());
    gamepkg::samples(&[0, 0, 20, 5]);
    let mut p = physics::Particle::new(0.0, 60.0, 0.0, 0.0, 0xff0000ff);
    p.life = 10;
    let image = physics::particle_image(&[p]).unwrap();
    assert_eq!(
        (image.left, image.top, image.width, image.height),
        (0, 0, 6, 6)
    );
    assert_eq!(&image.pixels[..4], &[255, 0, 0, 127]);
    gamepkg::samples(&[u64::MAX, 239, 254]);
    let p = physics::Particle::new(0.0, 60.0, 0.0, 0.0, 0xffffffff);
    assert_eq!((p.life, p.size), (39, 7));
    assert!(p.rotation < 360.0 && p.rotation_speed < 10.0);
    game.mode = 2;
    let before = game.particles[0].life;
    game.update(0);
    assert_eq!(game.particles[0].life, before, "pause aged particles");
}
#[test]
fn original_particle_triggers_keep_launch_record_portal_and_absorption_counts() {
    let mut game = menu_game();
    game.mode = 1;
    game.fire_or_launch();
    assert_eq!(game.particles.len(), 20);
    assert!(game
        .particles
        .iter()
        .all(|p| p.color == 0xffc832ff && (20..40).contains(&p.life) && (3..8).contains(&p.size)));
    game.fire_or_launch();
    assert_eq!(
        game.particles.len(),
        20,
        "a flying ball spawned another manual-launch burst"
    );
    let mut portal = menu_game();
    portal.mode = 1;
    portal.balls[0].attached = false;
    portal.balls[0].vx = 30.0;
    portal.balls[0].vy = 0.0;
    portal.update(0);
    assert!(portal.portal);
    assert_eq!(portal.particles.len(), 105);
    assert_eq!(
        portal
            .particles
            .iter()
            .filter(|p| p.color == 0x9632ffff)
            .count(),
        100
    );
    assert!(portal.particles.iter().all(|p| p.life + 1 == p.max_life));
    portal.update(0);
    assert_eq!(
        portal.particles.len(),
        105,
        "portal activation replayed its burst"
    );
    let mut absorb = menu_game();
    absorb.mode = 1;
    absorb.portal = true;
    absorb.blocks = [0; 200];
    absorb.blocks[0] = 1;
    absorb.block_x[0] = 305.0;
    absorb.block_y[0] = 235.0;
    absorb.update(0);
    assert_eq!(absorb.particles.len(), 5);
    assert!(absorb
        .particles
        .iter()
        .all(|p| p.color == 0x9632ffff && p.life + 1 == p.max_life));
}
#[test]
fn packed_particle_randomness_keeps_independent_uniform_ranges_and_bounded_calls() {
    let mut ages = [0u32; 20];
    let mut sizes = [0u32; 5];
    for age in 0u64..240 {
        for size in 0u64..255 {
            let word = (1u64 << 62) | (1u64 << 39) | (size << 8) | age;
            gamepkg::samples(&[word, 12345]);
            let p = physics::Particle::new(0.0, 60.0, 0.0, 0.0, 0xffffffff);
            ages[(p.life - 20) as usize] += 1;
            sizes[(p.size - 3) as usize] += 1;
            assert_eq!((p.rotation, p.rotation_speed), (90.0, 0.0));
            assert_eq!(
                gamepkg::random_u64(),
                12345,
                "accepted fields consumed extra host words"
            );
        }
    }
    assert_eq!(ages, [3060; 20]);
    assert_eq!(sizes, [12240; 5]);
    gamepkg::samples(&[65535, 255, 240, 239, 255, 254, 12345]);
    let p = physics::Particle::new(0.0, 60.0, 0.0, 0.0, 0xffffffff);
    assert_eq!((p.life, p.size), (39, 7));
    assert_eq!(
        gamepkg::random_u64(),
        12345,
        "integer rejection skipped or reused sample bytes"
    );
    gamepkg::samples(&[(1u64 << 62) | (1u64 << 39)]);
    assert_eq!(physics::random_pair(), [0.25, 0.5]);
}
#[test]
fn splash_restart_and_editor_return_match_pinned_original_state_code() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-flow.json")).unwrap();
    for row in fixture["startup"].as_array().unwrap() {
        let mut game = physics::Game::new();
        for _ in 0..row[0].as_u64().unwrap() {
            game.update(0);
        }
        assert_eq!(u64::from(game.mode), row[1].as_u64().unwrap());
        assert_eq!(u64::from(game.splash_timer), row[2].as_u64().unwrap());
        assert_eq!(game.frame, 0, "splash advanced gameplay");
        assert!(!game.cursor_confined());
    }
    for rows in fixture["skip_restart"].as_array().unwrap().chunks(2) {
        for mouse in [false, true] {
            let mut game = physics::Game::new();
            for _ in 0..rows[0][0].as_u64().unwrap() {
                game.update(0);
            }
            if mouse {
                game.source_input(2, 10.0, 10.0, 1);
            } else {
                game.source_input(5, 0.0, 0.0, 40);
            }
            assert_eq!(u64::from(game.mode), rows[0][1].as_u64().unwrap());
            assert_eq!(u64::from(game.splash_timer), rows[0][2].as_u64().unwrap());
            assert!(!game.started, "splash skip also activated New Game");
            game.started = true;
            game.reset_run();
            game.update(0);
            assert_eq!(u64::from(game.mode), rows[1][1].as_u64().unwrap());
            assert_eq!(u64::from(game.splash_timer), rows[1][2].as_u64().unwrap());
        }
    }
    for (index, rows) in fixture["editor_return"]
        .as_array()
        .unwrap()
        .chunks(2)
        .enumerate()
    {
        let mut game = menu_game();
        game.started = index == 1;
        game.mode = 6;
        game.score = 123;
        game.lives = 2;
        game.source_input(9, 0.0, 0.0, 27);
        assert_eq!(
            [game.mode, game.score, game.lives],
            [rows[0][0].as_u64().unwrap() as u32, 123, 2]
        );
        game.source_input(5, 0.0, 0.0, 41);
        assert_eq!(
            [game.mode, game.score, game.lives],
            [rows[1][0].as_u64().unwrap() as u32, 123, 2]
        );
    }
    let mut splash = physics::Game::new();
    splash.source_input(2, 320.0, 185.0, 2);
    splash.source_input(9, 0.0, 0.0, 101);
    splash.source_input(9, 0.0, 0.0, 113);
    splash.source_input(5, 0.0, 0.0, 41);
    assert_eq!(splash.mode, 7);
    assert!(!splash.quit_requested);
    splash.source_input(5, 0.0, 0.0, 68);
    assert_eq!(splash.fullscreen_requests, 1);
}
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    assert_eq!(
        game.particle_spawns.len(),
        200,
        "destroyed bricks lost queued effects"
    );
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
        trail: Vec::new(),
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
            trail: Vec::new(),
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
        trail: Vec::new(),
    }
}

#[test]
fn ball_contacts_restore_original_upward_ejection_and_shockwave() {
    let mut game = menu_game();
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
        assert_eq!((particle.x, particle.y), (205.0, 280.0));
        assert_eq!(particle.color, 0xc8ffffff);
        assert!((particle.vx.hypot(particle.vy) - 6.0).abs() < 0.00001);
    }
}

#[test]
fn ball_contacts_keep_strict_distance_and_original_coincident_response() {
    let mut game = menu_game();
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
    let mut game = menu_game();
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
        let mut game = menu_game();
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
    let mut game = menu_game();
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
        let mut game = menu_game();
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
        let mut game = menu_game();
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
        let mut game = menu_game();
        game.mode = 1;
        game.level = fixture["level"].as_u64().unwrap() as u32;
        game.lives = fixture["lives"].as_u64().unwrap() as u32;
        game.lost_life = fixture["lost"].as_bool().unwrap();
        game.finish_level();
        assert_eq!(game.sound, 0);
        assert_eq!(
            [game.lives, game.mode],
            [
                fixture["expected"][0].as_u64().unwrap() as u32,
                fixture["expected"][1].as_u64().unwrap() as u32
            ]
        );
    }
    for fixture in reference["losses"].as_array().unwrap() {
        let mut game = menu_game();
        game.mode = 1;
        game.lives = fixture["lives"].as_u64().unwrap() as u32;
        game.score = fixture["score"].as_u64().unwrap() as u32;
        game.portal = fixture["portal"].as_bool().unwrap();
        game.width = 90.0;
        game.balls.clear();
        game.update(0);
        assert_eq!(game.sound, if game.portal { 0 } else { 5 });
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
        let mut game = menu_game();
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
            max_life: 20,
            rotation: 0.0,
            rotation_speed: 0.0,
            size: 3,
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
        let mut game = menu_game();
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
            game.sound = 0;
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
            assert_eq!(
                game.sound,
                expected["sound"].as_u64().unwrap() as u32,
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
        let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
    game.blocks = [0; 200];
    game.mode = 1;
    assert!(game.set_block(0, 3));
    game.update(0);
    assert_eq!(game.mode, 3);
    assert_eq!(game.blocks[0], 1);
    assert_eq!(game.score, 0);
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    assert_eq!((game.mode, game.level, game.lives), (7, 1, 3));
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
    let mut game = menu_game();
    game.source_input(9, 0.0, 0.0, 101);
    assert_eq!(game.mode, 6);
    game.source_input(2, 600.0, 370.0, 1);
    game.source_input(2, 600.0, 370.0, 1);
    assert_eq!(game.editor.background, 3);
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
    assert_eq!(
        game.editor.background, 3,
        "Testing a board discarded the selected background"
    );
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    let mut game = menu_game();
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
    assert_eq!(game.mode, 2);
    assert!(!game.menu.settings);
    game.score = 999;
    game.lives = 1;
    game.ammo = 20;
    game.long = 10;
    game.portal = true;
    game.source_input(2, 320.0, 185.0, 1);
    assert_eq!(
        (game.mode, game.score, game.lives, game.ammo, game.long),
        (1, 999, 1, 20, 10)
    );
    assert!(game.portal, "Resume discarded the run after editor return");
    assert_eq!(game.unlocked, 9);
    assert!(game.gravity);
    game.mode = 2;
    game.source_input(2, 320.0, 210.0, 1);
    assert_eq!((game.mode, game.level, game.score), (7, 1, 0));
    game.source_input(5, 0.0, 0.0, 40);
    assert_eq!(game.unlocked, 9);
    assert!(game.gravity);
    game.source_input(2, 320.0, 310.0, 1);
    assert!(game.quit_requested);
    game.source_input(2, 320.0, 185.0, 1);
    game.update(4);
    assert_eq!(game.mode, 2);
    let mut title = menu_game();
    title.score = 99;
    title.menu.settings = true;
    title.source_input(9, 0.0, 0.0, 114);
    assert_eq!(title.score, 0);
    assert!(!title.menu.settings);
    title.source_input(5, 0.0, 0.0, 40);
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
    let mut game = menu_game();
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
        for frame in [
            0,
            1,
            99,
            123456789,
            u64::from(u32::MAX),
            u64::from(u32::MAX) + 1,
            u64::MAX,
        ] {
            let normal = audio::effect_samples(kind, frame, 64, false);
            let quiet = audio::effect_samples(kind, frame, 32, false);
            let loud = audio::effect_samples(kind, frame, 128, false);
            for index in 0..800 {
                let sign = if (u128::from(frame) + index as u128) % period < period / 2 {
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

#[test]
fn signed_i32_refraction_ranges_match_unchanged_original_rand_calls() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-ice.json")).unwrap();
    let cases: Vec<_> = reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["frame"] == 0 && v["health"] == 2)
        .collect();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let x = case["x"].as_i64().unwrap() as i32;
        let y = case["y"].as_i64().unwrap() as i32;
        let mut rng = pattern_rng::Rng::new((x * y) as u64);
        for line in case["commands"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v[0] == "L")
            .skip(2)
            .take(3)
        {
            assert_eq!(rng.range_i32(5, 55), line[1].as_i64().unwrap() as i32 - x);
            assert_eq!(rng.range_i32(5, 15), line[2].as_i64().unwrap() as i32 - y);
            assert_eq!(rng.range_i32(5, 55), line[3].as_i64().unwrap() as i32 - x);
            assert_eq!(rng.range_i32(5, 15), line[4].as_i64().unwrap() as i32 - y);
        }
    }
}

#[test]
fn original_portal_stages_preserve_every_ordered_point_color_and_frame_precision() {
    use std::io::Read;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-portal.json")).unwrap();
    let mut maximum = 0;
    for case in fixture["cases"].as_array().unwrap() {
        let frame = case["frame"].as_u64().unwrap();
        let timer = case["timer"].as_u64().unwrap();
        let dots = physics::portal_points(frame, timer);
        let mut trace = Vec::new();
        for dot in &dots {
            trace.extend(dot.x.to_le_bytes());
            trace.extend(dot.y.to_le_bytes());
            trace.extend(dot.color.to_le_bytes());
        }
        let compressed = hex::decode(case["trace_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(dots.len(), case["point_count"].as_u64().unwrap() as usize);
        assert_eq!(trace, expected, "portal frame {frame} timer {timer}");
        assert_eq!(gamepkg_digest(&trace), case["trace_sha256"]);
        if let Some(image) = physics::effect_image(&dots) {
            assert_eq!(
                serde_json::json!([
                    image.left,
                    image.top,
                    image.width as i32,
                    image.height as i32
                ]),
                case["bounds"]
            );
            let compressed = hex::decode(case["sdl_dark_rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut original = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut original)
                .unwrap();
            let mut delta = 0;
            for (pixel, source) in image.pixels.chunks_exact(4).zip(original.chunks_exact(4)) {
                for channel in 0..3 {
                    let value = (u32::from(pixel[channel]) * u32::from(pixel[3])
                        + [11, 16, 32][channel] * (255 - u32::from(pixel[3])))
                        / 255;
                    delta = delta.max(value.abs_diff(u32::from(source[channel])));
                }
            }
            println!("portal frame {frame} timer {timer}: grouped layer vs original software SDL maximum channel delta {delta}");
            assert!(delta <= 4, "original software SDL grouping bound exceeded");
            maximum = maximum.max(delta);
        } else {
            assert!(case["bounds"].is_null());
        }
    }
    println!("portal software SDL worst grouping delta {maximum}");
}

fn gamepkg_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(bytes))
}

#[test]
fn original_campaign_background_points_colors_and_release_overflow_match() {
    use std::io::Read;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-background.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let level = case["level"].as_u64().unwrap() as u32;
        let frame = case["frame"].as_u64().unwrap();
        let scene = background::campaign(level, frame).unwrap();
        assert_eq!(u64::from(scene.color), case["background"].as_u64().unwrap());
        let mut actual = Vec::new();
        for dot in &scene.points {
            actual.extend(dot.x.to_le_bytes());
            actual.extend(dot.y.to_le_bytes());
            actual.extend(dot.color.to_le_bytes());
        }
        let compressed = hex::decode(case["trace_zlib_hex"].as_str().unwrap()).unwrap();
        let mut original = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut original)
            .unwrap();
        assert_eq!(actual, original, "level {level} frame {frame}");
        assert_eq!(gamepkg_digest(&actual), case["trace_sha256"]);
        assert_eq!(
            scene.points.len(),
            case["point_count"].as_u64().unwrap() as usize
        );
        assert_canonical_point_frame(&scene, canonical::case(level, frame));
    }
    for level in [0, 1, 6, 10, u32::MAX] {
        assert!(background::campaign(level, 0).is_none());
    }
}

fn check_procedural_background_fixture(text: &str) {
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let level = case["level"].as_u64().unwrap() as u32;
        let frame = case["frame"].as_u64().unwrap();
        assert_eq!(
            procedural::procedural_theme(level),
            case["theme"].as_u64().unwrap() as u32
        );
        let scene = procedural::procedural_background(level, frame).unwrap();
        assert_eq!(u64::from(scene.color), case["background"].as_u64().unwrap());
        let mut actual = Vec::new();
        for dot in &scene.points {
            actual.extend(dot.x.to_le_bytes());
            actual.extend(dot.y.to_le_bytes());
            actual.extend(dot.color.to_le_bytes());
        }
        assert_eq!(gamepkg_digest(&actual), case["trace_sha256"]);
        assert_eq!(
            scene.points.len(),
            case["point_count"].as_u64().unwrap() as usize
        );
        assert_canonical_point_frame(&scene, canonical::case(level, frame));
    }
    for record in fixture["rng_u8"].as_array().unwrap() {
        let mut rng = pattern_rng::Rng::new(record["seed"].as_u64().unwrap());
        for (index, value) in record["values"].as_array().unwrap().iter().enumerate() {
            let (low, high) = [(0u8, 255u8), (100, 200), (100, 255), (0, 17)][index % 4];
            assert_eq!(u64::from(rng.range_u8(low, high)), value.as_u64().unwrap());
        }
    }
    if let Some(records) = fixture.get("rng_half") {
        for record in records.as_array().unwrap() {
            let mut rng = pattern_rng::Rng::new(record["seed"].as_u64().unwrap());
            for value in record["values"].as_array().unwrap() {
                assert_eq!(rng.half_probability(), value.as_bool().unwrap());
            }
        }
    }
    for level in [0, 1, 6, 7, 8, 9] {
        assert!(procedural::procedural_background(level, 0).is_none());
    }
}

#[test]
fn original_procedural_snow_smoke_tornado_and_typed_color_rng_match() {
    check_procedural_background_fixture(include_str!("fixtures/arkanoo-procedural.json"));
}
#[test]
fn original_procedural_fire_and_deep_sea_match_ordered_pixels_and_rng() {
    check_procedural_background_fixture(include_str!("fixtures/arkanoo-fire-sea-background.json"));
}

#[test]
fn original_supernova_and_space_wars_preserve_ordered_pixels_and_half_probability() {
    check_procedural_background_fixture(include_str!("fixtures/arkanoo-space-background.json"));
}

#[test]
fn original_star_voyage_preserves_mixed_typed_root_stream_and_star_pixels() {
    check_procedural_background_fixture(include_str!("fixtures/arkanoo-star-voyage.json"));
}

#[test]
fn background_line_points_match_sdl_default_and_record_software_degenerate_edges() {
    use std::io::Read;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/sdl-background-lines.json")).unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let segment: Vec<_> = case["segment"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let mut dots = Vec::new();
        background::background_line_points(
            &mut dots,
            [segment[0], segment[1]],
            [segment[2], segment[3]],
            [64, 64],
            0xc864ff96,
        );
        assert!(dots.iter().all(|p| p.color == 0xc864ff96));
        let actual: std::collections::BTreeSet<_> = dots
            .iter()
            .filter(|p| (0..64).contains(&p.x) && (0..64).contains(&p.y))
            .map(|p| (p.y * 64 + p.x) as usize)
            .collect();
        for mode in ["opengl", "software"] {
            let data =
                hex::decode(case["images"][mode]["rgba_zlib_hex"].as_str().unwrap()).unwrap();
            let mut pixels = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut pixels)
                .unwrap();
            assert_eq!(pixels.len(), 64 * 64 * 4);
            assert_eq!(gamepkg_digest(&pixels), case["images"][mode]["rgba_sha256"]);
            let expected: std::collections::BTreeSet<_> = pixels
                .chunks_exact(4)
                .enumerate()
                .filter(|(_, p)| p[..3] != [11, 16, 32])
                .map(|(i, _)| i)
                .collect();
            if mode == "opengl" || case["coverage_pixel_differences"] == 0 {
                assert_eq!(actual, expected, "SDL {mode} line {segment:?}");
            } else {
                assert_eq!(segment[..2], segment[2..]);
                assert!(
                    expected.is_empty(),
                    "Software SDL drops the zero-length line"
                );
                assert_eq!(actual.len(), 1);
                assert_eq!(case["coverage_pixel_differences"], 1);
            }
        }
    }
}

fn check_line_background_fixture(text: &str) {
    use std::io::Read;
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let frame = case["frame"].as_u64().unwrap();
        let level = case["level"].as_u64().unwrap() as u32;
        let theme = case["theme"].as_u64().unwrap() as u32;
        assert_eq!(procedural::procedural_theme(level), theme);
        let segments = match theme {
            0 => procedural::cyber_segments(level, frame).1,
            3 => procedural::aurora_segments(level, frame).1,
            6 => procedural::rain_segments(level, frame),
            13 => procedural::kaleidoscope_segments(frame),
            14 => procedural::warp_segments(level, frame),
            _ => panic!("Unknown line theme"),
        };
        assert_eq!(
            segments.len(),
            case["segment_count"].as_u64().unwrap() as usize
        );
        let mut trace = Vec::new();
        for (segment, original) in segments.iter().zip(case["segments"].as_array().unwrap()) {
            let values = [
                i64::from(segment.start[0]),
                i64::from(segment.start[1]),
                i64::from(segment.end[0]),
                i64::from(segment.end[1]),
                i64::from(segment.color),
            ];
            assert_eq!(serde_json::json!(values), *original);
            for value in [
                segment.start[0],
                segment.start[1],
                segment.end[0],
                segment.end[1],
            ] {
                trace.extend(value.to_le_bytes());
            }
            trace.extend(segment.color.to_le_bytes());
        }
        assert_eq!(gamepkg_digest(&trace), case["segment_sha256"]);
        if theme == 13 {
            assert_eq!(case["coverage_backend_differences"], 0);
        }
        let scene = procedural::procedural_background(level, frame).unwrap();
        assert_eq!(u64::from(scene.color), case["background"].as_u64().unwrap());
        let groups = case["opengl_layer_counts_zlib_hex"]
            .as_array()
            .unwrap()
            .len();
        let mut counts = vec![vec![0u8; 1280 * 720]; groups];
        for (index, segment) in segments.iter().enumerate() {
            if segment.color & 255 == 0 {
                continue;
            }
            let mut dots = Vec::new();
            background::background_line_points(
                &mut dots,
                segment.start,
                segment.end,
                [1280, 720],
                segment.color,
            );
            for dot in dots {
                counts[index / 48][dot.y as usize * 1280 + dot.x as usize] += 1;
            }
        }
        for (layer, encoded) in case["opengl_layer_counts_zlib_hex"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let data = hex::decode(encoded.as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(
                counts[layer], expected,
                "Actual default GL layer {layer}, frame {frame}"
            );
        }
        let image = background::composited_background(&scene).unwrap();
        let mut actual = Vec::new();
        for _ in 0..1280 * 720 {
            actual.extend(scene.color.to_be_bytes());
        }
        for (index, pixel) in image.pixels.chunks_exact(4).enumerate() {
            let at = ((image.top as usize + index / image.width as usize) * 1280
                + image.left as usize
                + index % image.width as usize)
                * 4;
            for channel in 0..3 {
                actual[at + channel] = ((u32::from(pixel[channel]) * u32::from(pixel[3])
                    + u32::from(actual[at + channel]) * (255 - u32::from(pixel[3])))
                    / 255) as u8;
            }
        }
        for backend in ["sdl_rgba_zlib_hex", "opengl_rgba_zlib_hex"] {
            let data = hex::decode(case[backend].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(actual.len(), expected.len());
            let delta = actual
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            println!(
                "line theme {theme}, frame {frame}, {backend}, maximum grouping delta {delta}"
            );
            if backend == "opengl_rgba_zlib_hex" {
                assert_eq!(delta, 0, "Original default GL composition differs");
            } else {
                assert_eq!(
                    u64::from(delta),
                    case["maximum_backend_rgb_delta"].as_u64().unwrap(),
                    "Recorded original SDL renderer difference changed"
                );
            }
        }
    }
}

#[test]
fn original_kaleidoscope_preserves_line_commands_and_actual_sdl_framebuffers() {
    check_line_background_fixture(include_str!("fixtures/arkanoo-kaleidoscope.json"));
}
#[test]
fn original_rain_and_warp_preserve_line_commands_and_actual_sdl_framebuffers() {
    check_line_background_fixture(include_str!("fixtures/arkanoo-rain-warp.json"));
}

#[test]
fn original_cyber_and_aurora_preserve_line_commands_and_actual_sdl_framebuffers() {
    let text = include_str!("fixtures/arkanoo-cyber-aurora.json");
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    let mut orientations = [false; 2];
    for case in fixture["cases"].as_array().unwrap() {
        if case["theme"] == 3 {
            let first = &case["segments"][0];
            orientations[usize::from(first[0] == first[2])] = true;
        }
    }
    assert_eq!(
        orientations,
        [true, true],
        "Both original aurora branches need reference coverage"
    );
    check_line_background_fixture(text);
}

#[test]
fn original_matrix_and_nebula_preserve_rectangle_commands_and_actual_sdl_framebuffers() {
    use std::io::Read;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-matrix-nebula.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let frame = case["frame"].as_u64().unwrap();
        let level = case["level"].as_u64().unwrap() as u32;
        let theme = case["theme"].as_u64().unwrap() as u32;
        assert_eq!(procedural::procedural_theme(level), theme);
        let (color, rectangles) = match theme {
            4 => procedural::matrix_rectangles(level, frame),
            5 => procedural::nebula_rectangles(level, frame),
            _ => panic!("Unknown rectangle theme"),
        };
        assert_eq!(u64::from(color), case["background"].as_u64().unwrap());
        assert_eq!(
            rectangles.len(),
            case["rectangle_count"].as_u64().unwrap() as usize
        );
        let mut trace = Vec::new();
        let groups = case["opengl_layer_counts_zlib_hex"].as_array().unwrap();
        let mut counts = vec![vec![0u8; 1280 * 720]; groups.len()];
        for (index, (rect, original)) in rectangles
            .iter()
            .zip(case["rectangles"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                serde_json::json!([
                    i64::from(rect.position[0]),
                    i64::from(rect.position[1]),
                    i64::from(rect.size[0]),
                    i64::from(rect.size[1]),
                    i64::from(rect.color)
                ]),
                *original
            );
            trace.extend(rect.position[0].to_le_bytes());
            trace.extend(rect.position[1].to_le_bytes());
            trace.extend(rect.size[0].to_le_bytes());
            trace.extend(rect.size[1].to_le_bytes());
            trace.extend(rect.color.to_le_bytes());
            if rect.color & 255 == 0 {
                continue;
            }
            let mut dots = Vec::new();
            background::rectangle(
                &mut dots,
                rect.position[0],
                rect.position[1],
                rect.size[0] as i32,
                rect.size[1] as i32,
                rect.color,
            );
            for dot in dots {
                counts[index / 48][dot.y as usize * 1280 + dot.x as usize] += 1;
            }
        }
        assert_eq!(gamepkg_digest(&trace), case["rectangle_sha256"]);
        assert_eq!(case["coverage_backend_differences"], 0);
        for (group, encoded) in groups.iter().enumerate() {
            let data = hex::decode(encoded.as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(
                counts[group], expected,
                "Actual SDL rectangle contributions, theme {theme}, frame {frame}, group {group}"
            );
        }
        let mut actual = Vec::new();
        for _ in 0..1280 * 720 {
            actual.extend(color.to_be_bytes());
        }
        if let Some(image) = background::composited_rectangles(&rectangles, color) {
            for (index, pixel) in image.pixels.chunks_exact(4).enumerate() {
                let at = ((image.top as usize + index / image.width as usize) * 1280
                    + image.left as usize
                    + index % image.width as usize)
                    * 4;
                for channel in 0..3 {
                    actual[at + channel] = ((u32::from(pixel[channel]) * u32::from(pixel[3])
                        + u32::from(actual[at + channel]) * (255 - u32::from(pixel[3])))
                        / 255) as u8;
                }
            }
        }
        for backend in ["sdl_rgba_zlib_hex", "opengl_rgba_zlib_hex"] {
            let data = hex::decode(case[backend].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(actual.len(), expected.len());
            let delta = actual
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            println!("rectangle theme {theme}, frame {frame}, {backend}, maximum delta {delta}");
            if backend == "opengl_rgba_zlib_hex" {
                assert_eq!(
                    delta, 0,
                    "Original default GL rectangle composition differs"
                );
            } else {
                assert_eq!(
                    u64::from(delta),
                    case["maximum_backend_rgb_delta"].as_u64().unwrap(),
                    "Recorded original SDL renderer difference changed"
                );
            }
        }
    }
}

fn check_command_background_fixture(text: &str) {
    use std::io::Read;
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    if fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|case| case["theme"] == 2)
    {
        assert!(
            fixture["cases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|case| case["theme"] == 2 && case["commands"][0][3] == 18),
            "Smallest original grid size must remain covered"
        );
    }
    for case in fixture["outlines"].as_array().unwrap() {
        let rect = &case["rect"];
        let command = background::BackgroundCommand {
            kind: 2,
            values: [
                rect[0].as_i64().unwrap() as i32,
                rect[1].as_i64().unwrap() as i32,
                rect[2].as_i64().unwrap() as i32,
                rect[3].as_i64().unwrap() as i32,
            ],
            color: 0xc864ff96,
        };
        let mut dots = Vec::new();
        background::background_command_points(&mut dots, &command);
        let mut counts = vec![0u8; 1280 * 720];
        for dot in dots {
            counts[dot.y as usize * 1280 + dot.x as usize] += 1;
        }
        let data = hex::decode(case["opengl_counts_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(counts, expected, "Actual SDL outline baseline {rect}");
    }
    for case in fixture["cases"].as_array().unwrap() {
        let frame = case["frame"].as_u64().unwrap();
        let level = case["level"].as_u64().unwrap() as u32;
        let theme = case["theme"].as_u64().unwrap() as u32;
        assert_eq!(procedural::procedural_theme(level), theme);
        let (color, commands) = match theme {
            2 => procedural::hex_commands(level, frame),
            8 => procedural::night_commands(level, frame),
            12 => procedural::circuit_commands(level, frame),
            15 => procedural::maze_commands(level, frame),
            10 => procedural::bubble_commands(level, frame),
            _ => panic!("Unknown mixed-command theme"),
        };
        assert_eq!(u64::from(color), case["background"].as_u64().unwrap());
        assert_eq!(
            commands.len(),
            case["command_count"].as_u64().unwrap() as usize
        );
        let mut trace = Vec::new();
        let groups = case["opengl_layer_counts_zlib_hex"].as_array().unwrap();
        let mut counts = vec![vec![0u8; 1280 * 720]; groups.len()];
        for (index, (command, original)) in commands
            .iter()
            .zip(case["commands"].as_array().unwrap())
            .enumerate()
        {
            assert_eq!(
                serde_json::json!([
                    i64::from(command.kind),
                    i64::from(command.values[0]),
                    i64::from(command.values[1]),
                    i64::from(command.values[2]),
                    i64::from(command.values[3]),
                    i64::from(command.color)
                ]),
                *original
            );
            trace.extend(command.kind.to_le_bytes());
            for value in command.values {
                trace.extend(value.to_le_bytes());
            }
            trace.extend(command.color.to_le_bytes());
            if command.color & 255 == 0 {
                continue;
            }
            let mut dots = Vec::new();
            background::background_command_points(&mut dots, command);
            for dot in dots {
                counts[index / 48][dot.y as usize * 1280 + dot.x as usize] += 1;
            }
        }
        assert_eq!(gamepkg_digest(&trace), case["command_sha256"]);
        assert_eq!(case["coverage_backend_differences"], 0);
        for (group, encoded) in groups.iter().enumerate() {
            let data = hex::decode(encoded.as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(
                counts[group], expected,
                "Actual SDL primitive contributions, theme {theme}, frame {frame}, group {group}"
            );
        }
        let mut actual = Vec::new();
        for _ in 0..1280 * 720 {
            actual.extend(color.to_be_bytes());
        }
        if let Some(image) = background::composited_commands(&commands, color) {
            for (index, pixel) in image.pixels.chunks_exact(4).enumerate() {
                let at = ((image.top as usize + index / image.width as usize) * 1280
                    + image.left as usize
                    + index % image.width as usize)
                    * 4;
                for channel in 0..3 {
                    actual[at + channel] = ((u32::from(pixel[channel]) * u32::from(pixel[3])
                        + u32::from(actual[at + channel]) * (255 - u32::from(pixel[3])))
                        / 255) as u8;
                }
            }
        }
        for backend in ["sdl_rgba_zlib_hex", "opengl_rgba_zlib_hex"] {
            let data = hex::decode(case[backend].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(actual.len(), expected.len());
            let delta = actual
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            println!("command theme {theme}, frame {frame}, {backend}, maximum delta {delta}");
            if backend == "opengl_rgba_zlib_hex" {
                assert_eq!(
                    delta, 0,
                    "Original default GL primitive composition differs"
                );
            } else {
                assert_eq!(
                    u64::from(delta),
                    case["maximum_backend_rgb_delta"].as_u64().unwrap(),
                    "Recorded original SDL renderer difference changed"
                );
            }
        }
    }
}

#[test]
fn original_hex_and_bubbles_preserve_commands_and_actual_sdl_framebuffers() {
    check_command_background_fixture(include_str!("fixtures/arkanoo-hex-bubbles.json"));
}
#[test]
fn original_night_circuit_and_maze_preserve_commands_transitions_and_actual_sdl_framebuffers() {
    let text = include_str!("fixtures/arkanoo-night-circuit-maze.json");
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    let first_level = cases[0]["level"].as_u64().unwrap();
    for (frame, shooting, blink) in [(59, true, false), (60, false, true)] {
        let case = cases
            .iter()
            .find(|case| {
                case["level"].as_u64() == Some(first_level) && case["frame"].as_u64() == Some(frame)
            })
            .unwrap();
        assert_eq!(
            case["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command[0] == 1),
            shooting
        );
        assert_eq!(
            case["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|command| command[0] == 0 && command[5] == 0xff0000ffu32),
            blink
        );
    }
    check_command_background_fixture(text);
}
#[test]
fn original_rand_bernoulli_thresholds_preserve_ten_thirty_and_seventy_percent_streams() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-night-circuit-maze.json")).unwrap();
    for case in fixture["rng_probabilities"].as_array().unwrap() {
        let mut rng = pattern_rng::Rng::new(case["seed"].as_u64().unwrap());
        let probability = case["probability"].as_u64().unwrap();
        for expected in case["values"].as_array().unwrap() {
            let actual = match probability {
                0 => rng.ten_percent(),
                1 => rng.thirty_percent(),
                2 => rng.seventy_percent(),
                _ => panic!("Unknown reference probability"),
            };
            assert_eq!(actual, expected.as_bool().unwrap());
        }
    }
}

fn assert_canonical_point_frame(scene: &background::Background, case: &serde_json::Value) {
    use std::io::Read;
    assert_eq!(u64::from(scene.color), case["background"].as_u64().unwrap());
    let mut actual = Vec::new();
    for _ in 0..1280 * 720 {
        actual.extend(scene.color.to_be_bytes());
    }
    let image = if case["theme"] == 9 {
        let (color, rectangles) = procedural::smoke_rectangles(
            case["level"].as_u64().unwrap() as u32,
            case["frame"].as_u64().unwrap(),
        );
        assert_eq!(color, scene.color);
        background::composited_rectangles(&rectangles, color)
    } else {
        background::composited_background(scene)
    };
    if let Some(image) = image {
        for (index, pixel) in image.pixels.chunks_exact(4).enumerate() {
            let at = ((image.top as usize + index / image.width as usize) * 1280
                + image.left as usize
                + index % image.width as usize)
                * 4;
            for channel in 0..3 {
                actual[at + channel] = ((u32::from(pixel[channel]) * u32::from(pixel[3])
                    + u32::from(actual[at + channel]) * (255 - u32::from(pixel[3])))
                    / 255) as u8;
            }
        }
    }
    for backend in ["opengl_rgba_zlib_hex", "sdl_rgba_zlib_hex"] {
        let data = hex::decode(case[backend].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut expected)
            .unwrap();
        assert_eq!(actual.len(), expected.len());
        let delta = actual
            .iter()
            .zip(expected)
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap();
        if backend == "opengl_rgba_zlib_hex" {
            assert_eq!(
                delta, 0,
                "Canonical original frame differs: level {} frame {}",
                case["level"], case["frame"]
            );
        } else {
            assert_eq!(
                u64::from(delta),
                case["maximum_backend_rgb_delta"].as_u64().unwrap(),
                "Recorded original renderer difference changed"
            );
        }
    }
}
fn check_point_background_canonical_fixture(text: &str) {
    use std::io::Read;
    let fixture: serde_json::Value = serde_json::from_str(text).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let level = case["level"].as_u64().unwrap() as u32;
        let frame = case["frame"].as_u64().unwrap();
        let scene = if level <= 9 {
            assert_eq!(case["theme"].as_i64(), Some(-1));
            background::campaign(level, frame).unwrap()
        } else {
            assert_eq!(
                u64::from(procedural::procedural_theme(level)),
                case["theme"].as_u64().unwrap()
            );
            procedural::procedural_background(level, frame).unwrap()
        };
        let commands = case["commands"].as_array().unwrap();
        assert_eq!(
            commands.len(),
            case["command_count"].as_u64().unwrap() as usize
        );
        let groups = case["opengl_layer_counts_zlib_hex"].as_array().unwrap();
        assert_eq!(groups.len(), commands.len().div_ceil(48));
        let mut expected_points = Vec::new();
        let mut trace = Vec::new();
        for (group, commands) in commands.chunks(48).enumerate() {
            let mut counts = vec![0u8; 1280 * 720];
            for original in commands {
                let command = background::BackgroundCommand {
                    kind: original[0].as_u64().unwrap() as u32,
                    values: [
                        original[1].as_i64().unwrap() as i32,
                        original[2].as_i64().unwrap() as i32,
                        original[3].as_i64().unwrap() as i32,
                        original[4].as_i64().unwrap() as i32,
                    ],
                    color: original[5].as_u64().unwrap() as u32,
                };
                assert!(
                    matches!(command.kind, 0 | 3),
                    "Expected original point/fill primitives"
                );
                trace.extend(command.kind.to_le_bytes());
                for value in command.values {
                    trace.extend(value.to_le_bytes());
                }
                trace.extend(command.color.to_le_bytes());
                let start = expected_points.len();
                background::background_command_points(&mut expected_points, &command);
                if command.color & 255 != 0 {
                    for dot in &expected_points[start..] {
                        counts[dot.y as usize * 1280 + dot.x as usize] += 1;
                    }
                }
            }
            let data = hex::decode(groups[group].as_str().unwrap()).unwrap();
            let mut expected = Vec::new();
            flate2::read::ZlibDecoder::new(data.as_slice())
                .read_to_end(&mut expected)
                .unwrap();
            assert_eq!(
                counts, expected,
                "Original SDL contribution map: level {level} frame {frame} group {group}"
            );
        }
        assert_eq!(gamepkg_digest(&trace), case["command_sha256"]);
        let actual_points: Vec<_> = scene
            .points
            .iter()
            .filter(|dot| (0..1280).contains(&dot.x) && (0..720).contains(&dot.y))
            .collect();
        assert_eq!(actual_points.len(), expected_points.len());
        let mismatch = actual_points
            .iter()
            .zip(&expected_points)
            .position(|(a, b)| (a.x, a.y, a.color) != (b.x, b.y, b.color));
        assert!(mismatch.is_none(),"Original ordered primitive pixels differ: level {level} frame {frame} point {mismatch:?}");
        assert_canonical_point_frame(&scene, case);
    }
}
#[test]
fn original_canonical_star_voyage_snow_smoke_and_tornado_match_every_primitive_and_pixel() {
    check_point_background_canonical_fixture(include_str!(
        "fixtures/arkanoo-canonical-particles.json"
    ));
}
#[test]
fn original_canonical_fire_supernova_space_wars_and_deep_sea_match_every_primitive_and_pixel() {
    check_point_background_canonical_fixture(include_str!(
        "fixtures/arkanoo-canonical-effects.json"
    ));
}

#[test]
fn original_canonical_campaign_backgrounds_match_every_primitive_and_pixel() {
    check_point_background_canonical_fixture(include_str!(
        "fixtures/arkanoo-canonical-campaign.json"
    ));
}

#[path = "../ports/arkanoo/source/src/speed_font.rs"]
// Guest code initializes buffers without the forbidden vec! macro.
#[allow(clippy::slow_vector_initialization)]
mod speed_font;
#[test]
fn original_speed_font_labels_match_complete_sdl_ttf_rasters() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-speed-font.json")).unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let speed: u32 = case["text"]
            .as_str()
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let actual = speed_font::raster(speed);
        assert_eq!(actual.width as u64, case["width"].as_u64().unwrap());
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
        let mut expected = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut expected).unwrap();
        assert_eq!(actual.pixels, expected, "{}", case["text"]);
    }
}

#[test]
fn editor_palette_hover_matches_original_hitboxes_and_preserves_selection() {
    let mut edit = editor::Editor::new();
    assert_eq!(edit.hovered_color, 255);
    for index in 0..9u8 {
        let left = 430.0 + f32::from(index) * 22.5;
        for (x, y, expected) in [
            (left, 70.0, index),
            (left + 19.5, 84.5, index),
            (left + 20.0, 75.0, 255),
            (left, 69.5, 255),
            (left, 85.0, 255),
        ] {
            edit.editor_event(1, x, y, 0);
            assert_eq!(edit.hovered_color, expected);
            assert_eq!(edit.selected, if index == 0 { 0 } else { index - 1 });
        }
        edit.editor_event(1, left + 10.0, 75.0, 0);
        edit.editor_event(2, left + 10.0, 75.0, 1);
        assert_eq!((edit.selected, edit.hovered_color), (index, index));
        edit.editor_event(8, 0.0, 0.0, 0);
        assert_eq!(edit.hovered_color, index);
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        edit.editor_event(1, invalid, 75.0, 0);
        assert_eq!(edit.hovered_color, 255);
        edit.editor_event(1, 430.0, invalid, 0);
        assert_eq!(edit.hovered_color, 255);
    }
}

#[test]
fn palette_numbers_reuse_original_glyph_alpha_with_black_ink() {
    for number in 0..10 {
        let actual = speed_font::palette_number(number);
        let label = speed_font::raster(u32::from(number));
        assert_eq!(actual.width, 15);
        assert_eq!(actual.pixels.len(), 15 * 31 * 4);
        for row in 0..31 {
            for col in 0..15 {
                let at = (row * 15 + col) * 4;
                assert_eq!(&actual.pixels[at..at + 3], &[0, 0, 0]);
                assert_eq!(
                    actual.pixels[at + 3],
                    label.pixels[(row * label.width as usize + col) * 4 + 3]
                );
            }
        }
    }
    assert!(speed_font::palette_number(255)
        .pixels
        .iter()
        .all(|value| *value == 0));
}

#[path = "../ports/arkanoo/source/src/editor_font.rs"]
#[allow(clippy::slow_vector_initialization)]
mod editor_font;
#[test]
fn editor_static_labels_match_complete_original_sdl_ttf_rasters() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-editor-labels.json")).unwrap();
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let actual = editor_font::editor_raster(index as u32);
        assert_eq!(u64::from(actual.width), case["width"].as_u64().unwrap());
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        assert_eq!(actual.pixels, expected, "{}", case["text"]);
    }
    let invalid = editor_font::editor_raster(u32::MAX);
    assert_eq!(invalid.width, 0);
    assert!(invalid.pixels.is_empty());
}
#[test]
fn editor_toolbar_hover_uses_original_integer_boxes_and_nonmotion_retention() {
    let mut edit = editor::Editor::new();
    for index in 0..7 {
        let [x, y, w, h] = editor::Editor::toolbar_bounds(index);
        for (px, py, wanted) in [
            (x, y, index as u8),
            (x + w - 1, y + h - 1, index as u8),
            (x + w, y, 255),
            (x, y + h, 255),
        ] {
            edit.editor_event(1, px as f32 * 0.5, 60.0 + py as f32 * 0.5, 0);
            assert_eq!(edit.hovered_button, wanted);
            edit.editor_event(0, 0.0, 0.0, 0);
            assert_eq!(edit.hovered_button, wanted);
        }
    }
    edit.editor_event(1, f32::NAN, 390.0, 0);
    assert_eq!(edit.hovered_button, 255);
}

#[path = "../ports/arkanoo/source/src/dynamic_font.rs"]
#[allow(clippy::slow_vector_initialization)]
mod dynamic_font;
#[path = "../ports/arkanoo/source/src/dynamic_font_data.rs"]
mod dynamic_font_data;
#[test]
fn dynamic_unicode_font_matches_every_recorded_original_sdl_text_surface() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-dynamic-font.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let label = dynamic_font::unicode_layout(text).unwrap();
        assert_eq!(
            i64::from(label.width),
            case["width"].as_i64().unwrap(),
            "{text:?}"
        );
        assert_eq!(
            i64::from(label.height),
            case["height"].as_i64().unwrap(),
            "{text:?}"
        );
        if !case["rendered"].as_bool().unwrap() {
            continue;
        }
        let mut actual = vec![0u8; label.width as usize * label.height as usize * 4];
        for column in (0..label.width).step_by(256) {
            let width = (label.width - column).min(256);
            let tile = dynamic_font::unicode_raster_slice(&label, [200, 200, 200], column, width);
            for row in 0..label.height as usize {
                actual[(row * label.width as usize + column as usize) * 4
                    ..(row * label.width as usize + column as usize + width as usize) * 4]
                    .copy_from_slice(
                        &tile[row * width as usize * 4..(row + 1) * width as usize * 4],
                    );
            }
        }
        let compressed = hex::decode(case["rgba_zlib_hex"].as_str().unwrap()).unwrap();
        let mut expected = Vec::new();
        std::io::Read::read_to_end(
            &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
            &mut expected,
        )
        .unwrap();
        assert_eq!(actual, expected, "{text:?}");
    }
    assert!(dynamic_font::unicode_layout("bad\0name").is_none());
}

#[test]
fn menu_fullscreen_caption_state_survives_run_restart() {
    let mut game = menu_game();
    game.menu.fullscreen = true;
    game.reset_run();
    assert!(game.menu.fullscreen);
}

#[test]
fn fps_counts_the_original_strict_one_second_window_and_survives_run_restart() {
    let mut game = physics::Game::new();
    for (now, expected) in [
        (0, 1),
        (16666, 2),
        (33333, 3),
        (999999, 4),
        (1000000, 4),
        (1000001, 5),
        (1016666, 5),
        (2000000, 3),
    ] {
        game.frame_clock(now);
        assert_eq!(game.fps, expected, "time {now}");
    }
    let retained = game.fps_times.clone();
    game.reset_run();
    assert_eq!(game.fps, 3);
    assert_eq!(game.fps_times, retained);
    game.frame_clock((1u64 << 32) + 23);
    assert_eq!(game.fps, 1);
    game.frame_clock((1u64 << 32) + 24);
    assert_eq!(game.fps, 2);
}
#[test]
fn speed_record_timestamp_updates_only_for_new_records_and_resets_with_the_run() {
    let mut game = physics::Game::new();
    game.mode = 1;
    game.frame = 10;
    game.blocks = [0; 200];
    game.blocks[0] = 1;
    game.balls[0].attached = false;
    game.balls[0].x = 300.0;
    game.balls[0].y = 300.0;
    game.balls[0].vx = 4.0;
    game.balls[0].vy = 0.0;
    game.update(0);
    assert_eq!(game.max_speed_record_frame, 11);
    assert_eq!(game.max_speed, 480.0);
    game.balls[0].vx = 1.0;
    game.update(0);
    assert_eq!(game.max_speed_record_frame, 11);
    game.populate();
    assert_eq!(game.max_speed, 0.0);
    assert_eq!(game.max_speed_record_frame, 11);
    game.reset_run();
    assert_eq!(game.max_speed_record_frame, 0);
}

#[test]
fn original_record_and_gravity_indicator_colors_match_captured_sdl_values() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-hud.json")).unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let values = case["values"].as_array().unwrap();
        let frame = values[5].as_u64().unwrap();
        let record = values[6].as_u64().unwrap();
        let expected = |name: &str| -> Vec<u8> {
            case[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect()
        };
        assert_eq!(
            physics::speed_record_color(frame, record).as_slice(),
            expected("record_color")
        );
        assert_eq!(
            physics::gravity_color(frame).as_slice(),
            expected("gravity_color")
        );
    }
}

#[test]
fn sparse_particle_tiles_preserve_every_original_rgba_byte_grid_and_order() {
    let mut seed = 0x7d45c3b2u64;
    for count in [0, 1, 12, 137, 3000] {
        let mut particles = Vec::new();
        for index in 0..count {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            particles.push(physics::Particle {
                x: if index % 4 == 0 {
                    -2.75
                } else {
                    (seed % 1320) as f32 - 20.25
                },
                y: if index % 4 == 1 {
                    719.75
                } else {
                    ((seed >> 16) % 760) as f32 - 20.5
                },
                vx: 0.0,
                vy: 0.0,
                life: (index % 31) as u32,
                max_life: 31,
                rotation: (seed % 1000) as f32 * 0.01,
                rotation_speed: 0.0,
                size: index % 7 + 1,
                color: ((seed as u32) & 0xffffff00) | 255,
            });
        }
        let original = physics::particle_image(&particles);
        let tiles = physics::particle_tiles(&particles);
        if let Some(original) = original {
            let mut restored = vec![0; original.pixels.len()];
            let mut previous = None;
            for tile in &tiles {
                let key = (tile.top, tile.left);
                assert!(previous.is_none_or(|p| p < key));
                previous = Some(key);
                assert!(
                    (tile.left - original.left) % 128 == 0 && (tile.top - original.top) % 128 == 0
                );
                assert!(tile.width <= 128 && tile.height <= 128);
                assert!(tile.pixels.chunks_exact(4).any(|p| p[3] > 0));
                for row in 0..tile.height as usize {
                    let offset =
                        ((tile.top - original.top) as usize + row) * original.width as usize * 4
                            + (tile.left - original.left) as usize * 4;
                    restored[offset..offset + tile.width as usize * 4].copy_from_slice(
                        &tile.pixels
                            [row * tile.width as usize * 4..(row + 1) * tile.width as usize * 4],
                    );
                }
            }
            assert_eq!(
                restored, original.pixels,
                "{count} ordered overlapping shards"
            );
        } else {
            assert!(tiles.is_empty());
        }
    }
    let mut particles = Vec::new();
    for index in 0..3000 {
        particles.push(physics::Particle {
            x: if index % 2 == 0 { 0.0 } else { 1279.0 },
            y: if index % 4 < 2 { 0.0 } else { 719.0 },
            vx: 0.0,
            vy: 0.0,
            life: 30,
            max_life: 30,
            rotation: 0.0,
            rotation_speed: 0.0,
            size: 7,
            color: 0x1964c8ff,
        });
    }
    let tiles = physics::particle_tiles(&particles);
    assert_eq!(tiles.len(), 4);
    assert_eq!(tiles.iter().map(|t| t.pixels.len()).sum::<usize>(), 212992);
    for p in &mut particles {
        p.life = 0;
    }
    assert!(physics::particle_tiles(&particles).is_empty());
}

#[test]
fn launches_bonuses_and_completion_do_not_invent_original_sound_requests() {
    let mut game = menu_game();
    game.attach();
    game.sound = 0;
    game.fire_or_launch();
    assert_eq!(game.sound, 0);
    for kind in 0..4 {
        game.collect(kind);
        assert_eq!(game.sound, 0);
    }
    game.finish_level();
    assert_eq!(game.sound, 0);
    game.ammo = 2;
    game.balls.clear();
    game.fire_or_launch();
    assert_eq!(game.sound, 3);
    game.sound = 0;
    game.ammo = 0;
    game.fire_or_launch();
    assert_eq!(game.sound, 0);
}

#[test]
fn effect_blend_fast_paths_match_original_integer_equation() {
    for old_alpha in 0u32..=255 {
        for color in 0u32..=255 {
            for old in [0u8, 1, 63, 127, 128, 254, 255] {
                for opacity in [0u32, 1, 127, 254, 255] {
                    let before = [old, 255 - old, old / 2, old_alpha as u8];
                    let incoming = [color, 255 - color, color / 2];
                    let rgba = (incoming[0] << 24) | (incoming[1] << 16) | (incoming[2] << 8) | 255;
                    let mut expected = before;
                    if opacity != 0 {
                        let denominator = opacity * 255 + old_alpha * (255 - opacity);
                        for channel in 0..3 {
                            expected[channel] = ((incoming[channel] * opacity * 255
                                + u32::from(before[channel]) * old_alpha * (255 - opacity))
                                / denominator)
                                as u8;
                        }
                        expected[3] = (denominator / 255) as u8;
                    }
                    let mut actual = before;
                    physics::blend_effect_pixel(&mut actual, 0, rgba, opacity);
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}

#[test]
fn four_effect_channels_keep_stereo_tails_slot_order_mute_and_capacity() {
    let mut mixer = audio::Mixer::new();
    assert!(!mixer.effect_start(vec![7], false));
    assert!(!mixer.effect_start(vec![7, -7], true));
    for channel in 0..4 {
        let pair = [1000 + channel * 100, -500 - channel * 100];
        let mut clip = Vec::new();
        for _ in 0..(800 + channel as usize * 10) {
            clip.extend(pair);
        }
        assert!(mixer.effect_start(clip, false));
    }
    assert!(!mixer.effect_start(vec![1, 2], false));
    let first = mixer.effect_render(128, false).unwrap();
    assert_eq!(&first[..4], &[4600, -2600, 4600, -2600]);
    // The first channel ended; a new clip reuses that slot without replacing tails.
    assert!(mixer.effect_start(vec![20000, -20000], false));
    let second = mixer.effect_render(64, false).unwrap();
    assert_eq!(&second[..4], &[11800, -11050, 1800, -1050]);
    assert_eq!(&second[20..22], &[1250, -750]);
    assert_eq!(&second[40..42], &[650, -400]);
    assert!(second[60..].iter().all(|v| *v == 0));
    assert!(mixer.effect_render(128, false).is_none());
    assert!(mixer.effect_start(vec![1000; 3200], false));
    assert!(mixer.effect_render(128, true).is_none());
    assert!(mixer.effect_render(0, false).is_none());
    assert!(mixer.effect_render(128, false).is_none());
}

#[test]
#[cfg(target_os = "linux")]
fn effect_mix_matches_actual_sdl_channel_gain_clipping_and_order() {
    #[link(name = "SDL2")]
    unsafe extern "C" {
        fn SDL_MixAudioFormat(dst: *mut u8, src: *const u8, format: u16, len: u32, volume: i32);
    }
    for gain in [0u32, 1, 32, 64, 127, 128, u32::MAX] {
        let mut mixer = audio::Mixer::new();
        let mut expected = [0u8; 3200];
        for channel in 0..4 {
            let samples: Vec<i16> = (0..1600)
                .map(|index| {
                    let values = [32767i16, -32768, 30000, -20000, 1, -1, 0, 8123];
                    values[(index + channel * 3) % values.len()]
                })
                .collect();
            let bytes: Vec<u8> = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect();
            unsafe {
                SDL_MixAudioFormat(
                    expected.as_mut_ptr(),
                    bytes.as_ptr(),
                    0x8010,
                    bytes.len() as u32,
                    gain.min(128) as i32,
                );
            }
            assert!(mixer.effect_start(samples, false));
        }
        let actual = mixer.effect_render(gain, false);
        if gain == 0 {
            assert!(actual.is_none());
        } else {
            let expected: Vec<i16> = expected
                .chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]))
                .collect();
            assert_eq!(actual.unwrap().as_slice(), expected, "gain {gain}");
        }
    }
}

#[test]
fn rocket_input_and_physics_callbacks_are_independent_and_reset_keeps_playing_effects() {
    let mut game = menu_game();
    game.mode = 1;
    game.balls.clear();
    game.ammo = 1;
    game.fire_or_launch();
    game.capture_sound();
    assert_eq!(game.sound, 0);
    game.sound = 2;
    game.capture_sound();
    let output = game.effects.effect_render(64, false).unwrap();
    let load = audio::effect_samples(3, game.frame, 64, false);
    let glass = audio::effect_samples(2, game.frame, 64, false);
    for index in 0..800 {
        let wanted = (i32::from(load[index]) + i32::from(glass[index])).clamp(-32768, 32767) as i16;
        assert_eq!(&output[index * 2..index * 2 + 2], &[wanted, wanted]);
    }
    assert!(game.effects.effect_start([1000, -2000].repeat(1600), false));
    game.reset_run();
    assert_eq!(
        &game.effects.effect_render(128, false).unwrap()[..2],
        &[1000, -2000]
    );
    game.mode = 2;
    game.update(0);
    assert_eq!(
        &game.effects.effect_render(64, false).unwrap()[..2],
        &[500, -1000]
    );
}
