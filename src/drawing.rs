//! Bounded, renderer-independent draw commands in a 640 by 480 virtual canvas.
pub const MAX_COMMANDS: usize = 1024;
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Shader {
        index: usize,
    },
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rgba: u32,
    },
    Circle {
        x: f32,
        y: f32,
        radius: f32,
        rgba: u32,
    },
}
#[derive(Debug, Default)]
pub struct Frame {
    pub background: Option<u32>,
    pub commands: Vec<Command>,
}
impl Frame {
    pub fn reset(&mut self) {
        self.background = None;
        self.commands.clear();
    }
    pub fn shader(&mut self, index: i32, count: usize) -> bool {
        if index < 0 || index as usize >= count || self.commands.len() >= MAX_COMMANDS {
            return false;
        }
        self.commands.push(Command::Shader {
            index: index as usize,
        });
        true
    }
    pub fn rect(&mut self, x: f32, y: f32, width: f32, height: f32, rgba: u32) -> bool {
        if !valid_position(x, y)
            || !width.is_finite()
            || !height.is_finite()
            || !(0.0..=1280.0).contains(&width)
            || !(0.0..=960.0).contains(&height)
            || self.commands.len() >= MAX_COMMANDS
        {
            return false;
        }
        self.commands.push(Command::Rect {
            x,
            y,
            width,
            height,
            rgba,
        });
        true
    }
    pub fn circle(&mut self, x: f32, y: f32, radius: f32, rgba: u32) -> bool {
        if !valid_position(x, y)
            || !radius.is_finite()
            || !(0.0..=640.0).contains(&radius)
            || self.commands.len() >= MAX_COMMANDS
        {
            return false;
        }
        self.commands.push(Command::Circle { x, y, radius, rgba });
        true
    }
}
fn valid_position(x: f32, y: f32) -> bool {
    x.is_finite()
        && y.is_finite()
        && (-640.0..=1280.0).contains(&x)
        && (-480.0..=960.0).contains(&y)
}
