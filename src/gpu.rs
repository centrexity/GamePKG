//! Host-owned OpenGL programs for compiled, resource-free fragment passes.
use anyhow::{bail, Context, Result};
use eframe::{glow, glow::HasContext};
use std::sync::Arc;
pub struct Renderer {
    gl: Arc<glow::Context>,
    programs: Vec<glow::Program>,
    vao: glow::VertexArray,
}
impl Renderer {
    pub fn new(gl: Arc<glow::Context>, sources: &[String]) -> Result<Self> {
        let vao = unsafe { gl.create_vertex_array().map_err(anyhow::Error::msg)? };
        let mut renderer = Self {
            gl,
            programs: Vec::new(),
            vao,
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
                    if linked && self.gl.get_active_uniforms(program) > 0 {
                        self.gl.delete_program(program);
                        Err(anyhow::anyhow!(
                            "package fragment passes cannot access uniforms or texture bindings"
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
        if let Some(program) = self.programs.get(index) {
            unsafe {
                self.gl.use_program(Some(*program));
                self.gl.bind_vertex_array(Some(self.vao));
                self.gl.draw_arrays(glow::TRIANGLES, 0, 3);
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
        }
    }
}
