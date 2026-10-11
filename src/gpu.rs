//! Host-owned OpenGL programs for compiled fragment passes with an optional bounded image.
use anyhow::{bail, Context, Result};
use eframe::{glow, glow::HasContext};
use std::sync::Arc;
pub struct Renderer {
    gl: Arc<glow::Context>,
    programs: Vec<glow::Program>,
    vao: glow::VertexArray,
    texture: glow::Texture,
}
impl Renderer {
    pub fn new(gl: Arc<glow::Context>, sources: &[String]) -> Result<Self> {
        let vao = unsafe { gl.create_vertex_array().map_err(anyhow::Error::msg)? };
        let texture = match unsafe { gl.create_texture() } {
            Ok(t) => t,
            Err(e) => {
                unsafe {
                    gl.delete_vertex_array(vao);
                }
                return Err(anyhow::anyhow!(e));
            }
        };
        let mut renderer = Self {
            gl,
            programs: Vec::new(),
            vao,
            texture,
        };
        for source in sources {
            renderer.programs.push(renderer.program(source)?);
        }
        Ok(renderer)
    }
    fn shader(&self, kind: u32, source: &str) -> Result<glow::Shader> {
        unsafe {
            let shader = self.gl.create_shader(kind).map_err(anyhow::Error::msg)?;
            self.gl.shader_source(shader, source);
            self.gl.compile_shader(shader);
            if !self.gl.get_shader_compile_status(shader) {
                let error = self.gl.get_shader_info_log(shader);
                self.gl.delete_shader(shader);
                bail!("GPU shader compilation failed: {error}");
            }
            Ok(shader)
        }
    }
    fn program(&self, fragment: &str) -> Result<glow::Program> {
        let vertex = self.shader(glow::VERTEX_SHADER, "#version 330 core\nvoid main() { vec2 p = vec2((gl_VertexID << 1) & 2, gl_VertexID & 2); gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0); }")?;
        let fragment = match self.shader(glow::FRAGMENT_SHADER, fragment) {
            Ok(shader) => shader,
            Err(error) => {
                unsafe {
                    self.gl.delete_shader(vertex);
                }
                return Err(error);
            }
        };
        unsafe {
            let program = self.gl.create_program();
            let result = match program {
                Ok(program) => {
                    self.gl.attach_shader(program, vertex);
                    self.gl.attach_shader(program, fragment);
                    self.gl.link_program(program);
                    let linked = self.gl.get_program_link_status(program);
                    let error = self.gl.get_program_info_log(program);
                    self.gl.detach_shader(program, vertex);
                    self.gl.detach_shader(program, fragment);
                    let allowed = (0..self.gl.get_active_uniforms(program)).all(|i| {
                        self.gl.get_active_uniform(program, i).is_some_and(|u| {
                            u.name == "gamepkgTexture_0"
                                && u.utype == glow::SAMPLER_2D
                                && u.size == 1
                        })
                    });
                    if linked && !allowed {
                        self.gl.delete_program(program);
                        Err(anyhow::anyhow!(
                            "package fragment passes permit only the host-owned gamepkgTexture sampler"
                        ))
                    } else if linked {
                        Ok(program)
                    } else {
                        self.gl.delete_program(program);
                        Err(anyhow::anyhow!("GPU shader link failed: {error}"))
                    }
                }
                Err(error) => Err(anyhow::anyhow!(error)),
            };
            self.gl.delete_shader(vertex);
            self.gl.delete_shader(fragment);
            result.context("unable to prepare package fragment pass")
        }
    }
    pub fn paint(&self, index: usize) {
        self.paint_image(index, None);
    }
    pub fn paint_image(&self, index: usize, image: Option<&crate::drawing::image::Image>) {
        if let Some(program) = self.programs.get(index) {
            unsafe {
                self.gl.use_program(Some(*program));
                if let Some(location) = self.gl.get_uniform_location(*program, "gamepkgTexture_0") {
                    self.gl.active_texture(glow::TEXTURE0);
                    self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                    let fallback = [0u8; 4];
                    let (size, pixels) =
                        image.map_or(([1, 1], fallback.as_slice()), |i| (i.size(), i.pixels()));
                    self.gl.tex_image_2d(
                        glow::TEXTURE_2D,
                        0,
                        glow::RGBA8 as i32,
                        size[0] as i32,
                        size[1] as i32,
                        0,
                        glow::RGBA,
                        glow::UNSIGNED_BYTE,
                        glow::PixelUnpackData::Slice(Some(pixels)),
                    );
                    self.gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MIN_FILTER,
                        glow::NEAREST as i32,
                    );
                    self.gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_MAG_FILTER,
                        glow::NEAREST as i32,
                    );
                    self.gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_S,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    self.gl.tex_parameter_i32(
                        glow::TEXTURE_2D,
                        glow::TEXTURE_WRAP_T,
                        glow::CLAMP_TO_EDGE as i32,
                    );
                    self.gl.uniform_1_i32(Some(&location), 0);
                }
                self.gl.bind_vertex_array(Some(self.vao));
                self.gl.draw_arrays(glow::TRIANGLES, 0, 3);
                self.gl.bind_texture(glow::TEXTURE_2D, None);
                self.gl.bind_vertex_array(None);
                self.gl.use_program(None);
            }
        }
    }
}
impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            for program in &self.programs {
                self.gl.delete_program(*program);
            }
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_texture(self.texture);
        }
    }
}
