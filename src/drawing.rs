//! Bounded, renderer-independent draw commands in a 640 by 480 virtual canvas.
pub mod image;
pub const MAX_COMMANDS: usize = 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: std::sync::Arc<image::Image>,
    },
    LinearImage {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: std::sync::Arc<image::Image>,
    },
    ShaderImage {
        index: usize,
        image: std::sync::Arc<image::Image>,
    },
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
    image_bytes: usize,
}
impl Frame {
    pub fn reset(&mut self) {
        self.background = None;
        self.commands.clear();
        self.image_bytes = 0;
    }
    pub fn can_image(&self, x: f32, y: f32, width: f32, height: f32, bytes: usize) -> bool {
        valid_position(x, y)
            && width.is_finite()
            && height.is_finite()
            && (0.0..=1280.0).contains(&width)
            && (0.0..=960.0).contains(&height)
            && self.commands.len() < MAX_COMMANDS
            && bytes <= MAX_IMAGE_BYTES.saturating_sub(self.image_bytes)
    }
    pub fn image(&mut self, x: f32, y: f32, width: f32, height: f32, image: image::Image) -> bool {
        self.image_arc(x, y, width, height, std::sync::Arc::new(image), false)
    }
    pub fn linear_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: image::Image,
    ) -> bool {
        self.image_arc(x, y, width, height, std::sync::Arc::new(image), true)
    }
    pub fn image_arc(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: std::sync::Arc<image::Image>,
        linear: bool,
    ) -> bool {
        let bytes = image.pixels().len();
        if !self.can_image(x, y, width, height, bytes) {
            return false;
        }
        self.image_bytes += bytes;
        self.commands.push(if linear {
            Command::LinearImage {
                x,
                y,
                width,
                height,
                image,
            }
        } else {
            Command::Image {
                x,
                y,
                width,
                height,
                image,
            }
        });
        true
    }
    pub fn shader_image(&mut self, index: i32, count: usize, image: image::Image) -> bool {
        if index < 0
            || index as usize >= count
            || !self.can_image(0.0, 0.0, 640.0, 480.0, image.pixels().len())
        {
            return false;
        }
        self.image_bytes += image.pixels().len();
        self.commands.push(Command::ShaderImage {
            index: index as usize,
            image: std::sync::Arc::new(image),
        });
        true
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
