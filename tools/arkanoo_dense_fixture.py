"""Shared private state fixture; production renderer is unchanged."""
probe='''
    if input >= 0x9f000001 && input <= 0x9f000006 {
        if let Ok(mut slot) = GAME.lock() {
            if let Some(game) = slot.as_mut() {
                game.mode = 1; game.level = input - 0x9f000000;
                game.frame = 0; game.score = 0; game.fps = 0;
                game.balls.clear(); game.particles.clear();
                for index in 0..200 { game.set_block(index, 2); }
                for index in 0..3000 {
                    game.particles.push(physics::Particle {
                        x: if index%2==0 {0.0} else {1279.0},
                        y: if index%4<2 {0.0} else {719.0},
                        vx:0.0, vy:0.0, life:30, max_life:30,
                        rotation:0.0, rotation_speed:0.0, size:7, color:0x1964c8ff,
                    });
                }
                render::draw(game);
                DENSE_EFFECT
                return ((game.editor.patterns.len() as u64) << 32) | game.particles.len() as u64;
            }
        }
        return 0;
    }
'''
