//! Bounded, renderer-independent draw commands in a 640 by 480 virtual canvas.
pub mod image;
pub const MAX_COMMANDS: usize = 1024;
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Replace the current clip, or restore the complete virtual canvas.
    Clip {
        rect: Option<[f32; 4]>,
    },
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
    /// Bilinear sampling of straight RGBA, multiplying alpha after filtering.
    StraightLinearImage {
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
    image_limit_bytes: usize,
}
impl Frame {
    pub fn for_profile(profile: Option<crate::manifest::RenderProfile>) -> Self {
        Self {
            image_limit_bytes: profile
                .map_or(MAX_IMAGE_BYTES, crate::manifest::RenderProfile::frame_bytes),
            ..Self::default()
        }
    }
    fn image_limit(&self) -> usize {
        if self.image_limit_bytes == 0 {
            MAX_IMAGE_BYTES
        } else {
            self.image_limit_bytes
        }
    }
    pub fn reset(&mut self) {
        self.background = None;
        self.commands.clear();
        self.image_bytes = 0;
    }
    /// Discard prior drawing, retaining this tick's transfer charges.
    pub fn clear(&mut self, rgba: u32) {
        self.commands.clear();
        self.background = Some(rgba);
    }
    pub fn clip(&mut self, rect: Option<[f32; 4]>) -> bool {
        if self.commands.len() >= MAX_COMMANDS {
            return false;
        }
        if let Some([x, y, w, h]) = rect {
            if ![x, y, w, h].iter().all(|v| v.is_finite())
                || x < 0.0
                || y < 0.0
                || w < 0.0
                || h < 0.0
                || x + w > 640.0
                || y + h > 480.0
            {
                return false;
            }
        }
        self.commands.push(Command::Clip { rect });
        true
    }
    pub fn can_image(&self, x: f32, y: f32, width: f32, height: f32, bytes: usize) -> bool {
        valid_position(x, y)
            && width.is_finite()
            && height.is_finite()
            && (0.0..=1280.0).contains(&width)
            && (0.0..=960.0).contains(&height)
            && self.commands.len() < MAX_COMMANDS
            && bytes <= self.image_limit().saturating_sub(self.image_bytes)
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
    pub fn straight_linear_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        image: std::sync::Arc<image::Image>,
    ) -> bool {
        if !self.can_image(x, y, width, height, image.pixels().len()) {
            return false;
        }
        self.image_bytes += image.pixels().len();
        self.commands.push(Command::StraightLinearImage {
            x,
            y,
            width,
            height,
            image,
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

#[cfg(test)]
mod clip_tests {
    use super::*;
    #[test]
    fn clip_replacement_reset_and_empty_regions_are_bounded_commands() {
        let mut frame = Frame::default();
        for rect in [
            [0.0, 60.0, 640.0, 360.0],
            [640.0, 480.0, 0.0, 0.0],
            [10.5, 20.25, 1.5, 0.5],
        ] {
            assert!(frame.clip(Some(rect)));
            assert_eq!(
                frame.commands.last(),
                Some(&Command::Clip { rect: Some(rect) })
            );
        }
        for rect in [
            [-1.0, 0.0, 1.0, 1.0],
            [0.0, 0.0, -1.0, 1.0],
            [0.0, 0.0, 641.0, 480.0],
            [640.0, 0.0, 1.0, 1.0],
            [0.0, 480.0, 0.0, 1.0],
            [f32::NAN, 0.0, 1.0, 1.0],
            [0.0, f32::INFINITY, 0.0, 0.0],
            [f32::MAX, 0.0, f32::MAX, 1.0],
        ] {
            let count = frame.commands.len();
            assert!(!frame.clip(Some(rect)));
            assert_eq!(frame.commands.len(), count);
        }
        assert!(frame.clip(None));
        assert_eq!(frame.commands.last(), Some(&Command::Clip { rect: None }));
        while frame.commands.len() < MAX_COMMANDS {
            assert!(frame.clip(None));
        }
        assert!(!frame.clip(None));
        assert!(!frame.clip(Some([0.0, 0.0, 1.0, 1.0])));
        frame.reset();
        assert!(frame.clip(None));
    }
    #[test]
    fn clearing_discards_clip_but_does_not_refund_invisible_image_transfers() {
        let mut frame = Frame::default();
        let image = image::Image::new(512, 512, &vec![255; image::MAX_BYTES]).unwrap();
        assert!(frame.clip(Some([0.0, 0.0, 0.0, 0.0])));
        for _ in 0..8 {
            assert!(frame.image(0.0, 0.0, 512.0, 512.0, image.clone()));
        }
        frame.clear(0x123456ff);
        assert!(frame.commands.is_empty());
        assert_eq!(frame.background, Some(0x123456ff));
        assert!(!frame.image(0.0, 0.0, 1.0, 1.0, image.clone()));
        frame.reset();
        assert!(frame.image(0.0, 0.0, 1.0, 1.0, image));
    }
}

#[cfg(test)]
mod straight_filter_tests {
    use super::*;
    #[test]
    fn straight_sampling_shares_existing_transfer_and_command_budgets() {
        let image =
            std::sync::Arc::new(image::Image::new(512, 512, &vec![127; 512 * 512 * 4]).unwrap());
        let mut frame = Frame::default();
        for _ in 0..8 {
            assert!(frame.straight_linear_image(0.0, 0.0, 20.0, 20.0, image.clone()));
        }
        assert!(!frame.image_arc(0.0, 0.0, 20.0, 20.0, image.clone(), true));
        frame.clear(0);
        assert!(!frame.straight_linear_image(0.0, 0.0, 20.0, 20.0, image.clone()));
        frame.reset();
        for _ in 0..MAX_COMMANDS {
            assert!(frame.clip(None));
        }
        assert!(!frame.straight_linear_image(0.0, 0.0, 20.0, 20.0, image));
    }
}
