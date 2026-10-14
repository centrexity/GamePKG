// Campaign predicates, paddle spin, scoring and bonuses adapted from Arkanoo.
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub spin: f32,
    pub attached: bool,
}
pub struct Drop {
    pub x: f32,
    pub y: f32,
    pub kind: u32,
}
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: u32,
}
pub struct Game {
    pub mode: u32,
    pub level: u32,
    pub unlocked: u32,
    pub score: u32,
    pub best: u32,
    pub lives: u32,
    pub paddle: f32,
    pub width: f32,
    pub long: u32,
    pub ghost: u32,
    pub ammo: u32,
    pub balls: Vec<Ball>,
    pub drops: Vec<Drop>,
    pub rockets: Vec<(f32, f32)>,
    pub particles: Vec<Particle>,
    pub blocks: [u8; 200],
    pub frame: u32,
    pub seed: u32,
    pub launch: u32,
    pub cooldown: u32,
    pub sound: u32,
    pub dirty: bool,
}
impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            mode: 0,
            level: 1,
            unlocked: 1,
            score: 0,
            best: 0,
            lives: 3,
            paddle: 285.0,
            width: 70.0,
            long: 0,
            ghost: 0,
            ammo: 0,
            balls: Vec::new(),
            drops: Vec::new(),
            rockets: Vec::new(),
            particles: Vec::new(),
            blocks: [0; 200],
            frame: 0,
            seed: 54321,
            launch: 90,
            cooldown: 60,
            sound: 0,
            dirty: false,
        };
        game.populate();
        game.attach();
        game
    }
    pub fn random(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }
    pub fn populate(&mut self) {
        for row in 0..10 {
            for col in 0..20 {
                self.blocks[row * 20 + col] = if pattern(self.level, row, col) { 1 } else { 0 };
            }
        }
    }
    pub fn attach(&mut self) {
        self.long = 0;
        self.ghost = 0;
        self.width = 70.0;
        self.paddle = self.paddle.clamp(8.0, 632.0 - self.width);
        self.balls.clear();
        self.balls.push(Ball {
            x: self.paddle + self.width / 2.0,
            y: 423.0,
            vx: 2.0,
            vy: -3.0,
            spin: 0.0,
            attached: true,
        });
        self.launch = 90;
        self.drops.clear();
        self.rockets.clear();
        self.particles.clear();
        self.paddle = self.paddle.clamp(8.0, 632.0 - self.width);
    }
    pub fn burst(&mut self, x: f32, y: f32) {
        for n in 0..8 {
            if self.particles.len() >= 64 {
                break;
            }
            let angle = n as f32 * core::f32::consts::FRAC_PI_4;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * 2.0,
                vy: angle.sin() * 2.0,
                life: 22,
            });
        }
    }
    pub fn award(&mut self, points: u32) {
        self.score = self.score.saturating_add(points).min(9999999);
        // Persist only completed-level/lost-life checkpoints, never every frame.
        if self.score > self.best {
            self.best = self.score;
        }
    }
    pub fn hit(&mut self, index: usize) {
        if self.blocks[index] == 0 {
            return;
        }
        self.blocks[index] = 0;
        self.award(10);
        self.sound = 2;
        let x = 20.0 + (index % 20) as f32 * 30.0;
        let y = 72.0 + (index / 20) as f32 * 18.0;
        self.burst(x + 14.0, y + 8.0);
        let roll = self.random();
        if roll % 100 < 15 && self.cooldown >= 60 && self.drops.len() < 8 {
            let weight = (roll / 100) % 100;
            let kind = if weight < 50 {
                0
            } else if weight < 75 {
                1
            } else if weight < 90 {
                2
            } else {
                3
            };
            self.drops.push(Drop {
                x: x + 14.0,
                y,
                kind,
            });
            self.cooldown = 0;
        }
    }
    pub fn collect(&mut self, kind: u32) {
        self.award(2);
        self.sound = 3;
        match kind {
            0 => {
                self.width = 100.0;
                self.long = 300;
                self.paddle = self.paddle.clamp(8.0, 632.0 - self.width);
            }
            1 => {
                if self.balls.len() < 4 {
                    self.balls.push(Ball {
                        x: self.paddle + self.width / 2.0,
                        y: 420.0,
                        vx: -2.5,
                        vy: -3.0,
                        spin: 0.0,
                        attached: false,
                    });
                }
            }
            2 => self.ghost = 600,
            _ => self.ammo = (self.ammo + 1).min(9),
        }
    }
    pub fn update(&mut self, input: u32) {
        self.frame = self.frame.wrapping_add(1);
        if input & 8 != 0 && (self.mode == 1 || self.mode == 2) {
            self.mode = if self.mode == 1 { 2 } else { 1 };
            return;
        }
        if self.mode != 1 {
            if input & 4 != 0 {
                if self.mode == 0 {
                    self.level = self.unlocked;
                    self.populate();
                    self.attach();
                }
                if self.mode == 3 {
                    self.level = (self.level + 1).min(999);
                    self.populate();
                    self.attach();
                }
                if self.mode == 4 {
                    self.level = 1;
                    self.lives = 3;
                    self.score = 0;
                    self.ammo = 0;
                    self.populate();
                    self.attach();
                }
                self.mode = 1;
            }
            return;
        }
        let previous = self.paddle;
        if input & 257 != 0 {
            self.paddle -= 6.0;
        }
        if input & 514 != 0 {
            self.paddle += 6.0;
        }
        self.paddle = self.paddle.clamp(8.0, 632.0 - self.width);
        let movement = self.paddle - previous;
        if self.long > 0 {
            self.long -= 1;
            if self.long == 0 {
                self.width = 70.0;
            }
        }
        if self.ghost > 0 {
            self.ghost -= 1;
        }
        self.cooldown = self.cooldown.saturating_add(1);
        if self.launch > 0 {
            self.launch -= 1;
        }
        if input & 4 != 0 && self.ammo > 0 && self.rockets.len() < 4 {
            self.ammo -= 1;
            self.rockets.push((self.paddle + self.width / 2.0, 422.0));
            self.sound = 3;
        }
        for i in 0..self.balls.len() {
            if self.balls[i].attached {
                self.balls[i].x = self.paddle + self.width / 2.0;
                if self.launch == 0 || input & 4 != 0 {
                    self.balls[i].attached = false;
                    self.sound = 1;
                } else {
                    continue;
                }
            }
            // Four bounded substeps prevent tunnelling through thin bricks/paddle.
            for _ in 0..4 {
                let ball = &mut self.balls[i];
                ball.vx = (ball.vx + ball.spin * 0.0125).clamp(-5.5, 5.5);
                ball.spin *= 0.995;
                ball.x += ball.vx * 0.25;
                ball.y += ball.vy * 0.25;
                if ball.x < 12.0 {
                    ball.x = 12.0;
                    ball.vx = ball.vx.abs();
                    self.sound = 1;
                }
                if ball.x > 628.0 {
                    ball.x = 628.0;
                    ball.vx = -ball.vx.abs();
                    self.sound = 1;
                }
                if ball.y < 60.0 {
                    ball.y = 60.0;
                    ball.vy = ball.vy.abs();
                    self.sound = 1;
                }
                if ball.vy > 0.0
                    && ball.y >= 423.0
                    && ball.y <= 438.0
                    && ball.x >= self.paddle - 4.0
                    && ball.x <= self.paddle + self.width + 4.0
                {
                    ball.y = 423.0;
                    ball.vy = -ball.vy.abs().min(5.5);
                    let offset = (ball.x - (self.paddle + self.width / 2.0)) / (self.width / 2.0);
                    ball.vx = (offset * 3.5 + movement * 0.18).clamp(-5.5, 5.5);
                    if ball.vx.abs() < 1.0 {
                        ball.vx = if offset < 0.0 { -1.0 } else { 1.0 };
                    }
                    ball.spin = movement * 0.25;
                    self.award(5);
                    self.sound = 1;
                }
                let mut collided = 200;
                for index in 0..200 {
                    if self.blocks[index] == 0 {
                        continue;
                    }
                    let x = 20.0 + (index % 20) as f32 * 30.0;
                    let y = 72.0 + (index / 20) as f32 * 18.0;
                    let ball = &mut self.balls[i];
                    if ball.x + 4.0 > x
                        && ball.x - 4.0 < x + 28.0
                        && ball.y + 4.0 > y
                        && ball.y - 4.0 < y + 16.0
                    {
                        if self.ghost == 0 {
                            let overlap_x = (ball.x + 4.0).min(x + 28.0) - (ball.x - 4.0).max(x);
                            let overlap_y = (ball.y + 4.0).min(y + 16.0) - (ball.y - 4.0).max(y);
                            if overlap_x < overlap_y {
                                ball.x = if ball.x < x + 14.0 { x - 4.0 } else { x + 32.0 };
                                ball.vx = -ball.vx;
                            } else {
                                ball.y = if ball.y < y + 8.0 { y - 4.0 } else { y + 20.0 };
                                ball.vy = -ball.vy;
                            }
                        }
                        collided = index;
                        break;
                    }
                }
                if collided < 200 {
                    self.hit(collided);
                }
            }
        }
        let mut i = 0;
        while i < self.balls.len() {
            if self.balls[i].y > 485.0 {
                self.balls.remove(i);
            } else {
                i += 1;
            }
        }
        if self.balls.is_empty() {
            self.lives -= 1;
            self.score = self.score.saturating_sub(20);
            self.dirty = true;
            self.sound = 4;
            if self.lives == 0 {
                self.mode = 4;
            } else {
                self.attach();
            }
        }
        let mut i = 0;
        while i < self.drops.len() {
            self.drops[i].y += 1.4;
            let drop = &self.drops[i];
            if drop.y >= 422.0
                && drop.y <= 440.0
                && drop.x >= self.paddle - 8.0
                && drop.x <= self.paddle + self.width + 8.0
            {
                let kind = drop.kind;
                self.drops.remove(i);
                self.collect(kind);
            } else if drop.y > 480.0 {
                self.drops.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.rockets.len() {
            self.rockets[i].1 -= 6.0;
            let (rx, ry) = self.rockets[i];
            let mut struck = 200;
            for index in 0..200 {
                let x = 20.0 + (index % 20) as f32 * 30.0;
                let y = 72.0 + (index / 20) as f32 * 18.0;
                if self.blocks[index] > 0 && rx >= x && rx <= x + 28.0 && ry >= y && ry <= y + 18.0
                {
                    struck = index;
                    break;
                }
            }
            if struck < 200 {
                let col = struck % 20;
                let row = struck / 20;
                for index in 0..200 {
                    let dx = (index % 20) as i32 - col as i32;
                    let dy = (index / 20) as i32 - row as i32;
                    if dx * dx + dy * dy <= 4 {
                        self.hit(index);
                    }
                }
                self.rockets.remove(i);
                self.sound = 4;
            } else if ry < 56.0 {
                self.rockets.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.particles.len() {
            let particle = &mut self.particles[i];
            particle.x += particle.vx;
            particle.y += particle.vy;
            particle.vy += 0.06;
            particle.life -= 1;
            if particle.life == 0 {
                self.particles.remove(i);
            } else {
                i += 1;
            }
        }
        let mut remaining = 0;
        for index in 0..200 {
            remaining += self.blocks[index] as u32;
        }
        if remaining == 0 && self.mode == 1 {
            self.mode = 3;
            self.unlocked = self.unlocked.max((self.level + 1).min(999));
            self.dirty = true;
            self.sound = 3;
        }
    }
}
// Upstream's nine handcrafted campaign layout predicates, scaled to our canvas.
pub fn pattern(level: u32, row: usize, col: usize) -> bool {
    match level {
        1 => true,
        2 => (row + col) % 2 == 0,
        3 => row % 2 == 0,
        4 => col % 2 == 0 || col % 2 == 1 && row == 0 || row == 9,
        5 => (col as i32 - 10).abs() <= row as i32,
        6 => (col as i32 - 10).abs() + (row as i32 - 5).abs() <= 7,
        7 => {
            let dx = col as f32 - 10.0;
            let dy = row as f32 - 5.0;
            (dy.atan2(dx) * 2.0 + (dx * dx + dy * dy).sqrt() * 0.5).sin() > 0.0
        }
        8 => {
            let dx = col as f32 - 10.0;
            let dy = row as f32 - 5.0;
            ((dx * dx + dy * dy).sqrt() as i32) % 3 != 1
        }
        9 => {
            !(col % 4 == 1 && row % 4 == 1 || col % 4 == 2 && row % 4 == 2) && (col + row) % 7 != 0
        }
        _ => {
            let seed = level
                .wrapping_mul(54321)
                .wrapping_add((row * 20 + col) as u32 * 77777);
            seed.wrapping_mul(1664525).wrapping_add(1013904223) % 100 < 65
        }
    }
}
