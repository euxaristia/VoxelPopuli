use gl::types::*;
use std::ffi::CString;
use std::ptr;

pub struct Shader {
    pub id: GLuint,
}

impl Shader {
    pub fn new(vertex_src: &str, fragment_src: &str) -> Result<Self, String> {
        unsafe {
            let vertex_shader = Self::compile_shader(gl::VERTEX_SHADER, vertex_src)?;
            let fragment_shader = Self::compile_shader(gl::FRAGMENT_SHADER, fragment_src)?;

            let id = gl::CreateProgram();
            gl::AttachShader(id, vertex_shader);
            gl::AttachShader(id, fragment_shader);
            gl::LinkProgram(id);

            let mut success = gl::FALSE as GLint;
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
            if success == gl::FALSE as GLint {
                let mut len = 0;
                gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut len);
                let mut buf = vec![0; len as usize];
                gl::GetProgramInfoLog(id, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
                return Err(String::from_utf8_lossy(&buf).into_owned());
            }

            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            Ok(Self { id })
        }
    }

    unsafe fn compile_shader(shader_type: GLenum, source: &str) -> Result<GLuint, String> {
        let shader = gl::CreateShader(shader_type);
        let c_str = CString::new(source.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        let mut success = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == gl::FALSE as GLint {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = vec![0; len as usize];
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
            return Err(String::from_utf8_lossy(&buf).into_owned());
        }
        Ok(shader)
    }

    pub fn bind(&self) {
        unsafe { gl::UseProgram(self.id) }
    }

    pub fn unbind() {
        unsafe { gl::UseProgram(0) }
    }

    pub fn get_uniform_location(&self, name: &str) -> i32 {
        let c_name = CString::new(name).unwrap();
        unsafe { gl::GetUniformLocation(self.id, c_name.as_ptr()) }
    }

    pub fn set_int(&self, location: i32, value: i32) {
        unsafe { gl::Uniform1i(location, value) }
    }

    pub fn set_float(&self, location: i32, value: f32) {
        unsafe { gl::Uniform1f(location, value) }
    }

    pub fn set_vec2(&self, location: i32, value: glam::Vec2) {
        unsafe { gl::Uniform2f(location, value.x, value.y) }
    }

    pub fn set_vec3(&self, location: i32, value: glam::Vec3) {
        unsafe { gl::Uniform3f(location, value.x, value.y, value.z) }
    }

    pub fn set_vec4(&self, location: i32, value: glam::Vec4) {
        unsafe { gl::Uniform4f(location, value.x, value.y, value.z, value.w) }
    }

    pub fn set_mat4(&self, location: i32, value: &glam::Mat4) {
        unsafe { gl::UniformMatrix4fv(location, 1, gl::FALSE, value.as_ref().as_ptr()) }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.id) }
    }
}

pub struct Texture2D {
    pub id: GLuint,
    pub width: i32,
    pub height: i32,
}

impl Texture2D {
    pub fn from_data(data: &[u8], width: i32, height: i32) -> Self {
        let mut id = 0;
        unsafe {
            gl::GenTextures(1, &mut id);
            gl::BindTexture(gl::TEXTURE_2D, id);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::REPEAT as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::REPEAT as i32);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
            gl::GenerateMipmap(gl::TEXTURE_2D);
            if gl::GetError() != gl::NO_ERROR {
                // Just in case, fall back to NEAREST if mipmaps fail
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            }
        }
        Self { id, width, height }
    }

    pub fn update(&self, x: i32, y: i32, width: i32, height: i32, data: &[u8]) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.id);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                x,
                y,
                width,
                height,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
        }
    }

    pub fn bind(&self, slot: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + slot);
            gl::BindTexture(gl::TEXTURE_2D, self.id);
        }
    }
}

impl Drop for Texture2D {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.id) }
    }
}

pub struct Mesh {
    vao: GLuint,
    vbo: [GLuint; 4],
    pub vertex_count: i32,
}

impl Mesh {
    pub fn new(vertices: &[f32], texcoords: Option<&[f32]>, normals: Option<&[f32]>, colors: Option<&[u8]>) -> Self {
        let mut vao = 0;
        let mut vbo = [0; 4];
        let vertex_count = (vertices.len() / 3) as i32;

        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(4, vbo.as_mut_ptr());

            gl::BindVertexArray(vao);

            // Vertices
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo[0]);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (vertices.len() * std::mem::size_of::<f32>()) as isize,
                vertices.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );
            gl::VertexAttribPointer(0, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
            gl::EnableVertexAttribArray(0);

            // Texcoords
            if let Some(uvs) = texcoords {
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo[1]);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (uvs.len() * std::mem::size_of::<f32>()) as isize,
                    uvs.as_ptr() as *const _,
                    gl::STATIC_DRAW,
                );
                gl::VertexAttribPointer(1, 2, gl::FLOAT, gl::FALSE, 0, ptr::null());
                gl::EnableVertexAttribArray(1);
            }

            // Normals
            if let Some(norms) = normals {
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo[2]);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (norms.len() * std::mem::size_of::<f32>()) as isize,
                    norms.as_ptr() as *const _,
                    gl::STATIC_DRAW,
                );
                gl::VertexAttribPointer(2, 3, gl::FLOAT, gl::FALSE, 0, ptr::null());
                gl::EnableVertexAttribArray(2);
            }

            // Colors
            if let Some(cols) = colors {
                gl::BindBuffer(gl::ARRAY_BUFFER, vbo[3]);
                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (cols.len() * std::mem::size_of::<u8>()) as isize,
                    cols.as_ptr() as *const _,
                    gl::STATIC_DRAW,
                );
                gl::VertexAttribPointer(3, 4, gl::UNSIGNED_BYTE, gl::TRUE, 0, ptr::null());
                gl::EnableVertexAttribArray(3);
            }

            gl::BindVertexArray(0);
        }

        Self { vao, vbo, vertex_count }
    }

    pub fn draw(&self) {
        if self.vertex_count == 0 { return; }
        unsafe {
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLES, 0, self.vertex_count);
            gl::BindVertexArray(0);
        }
    }
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(4, self.vbo.as_ptr());
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

pub struct RenderTexture2D {
    pub fbo: GLuint,
    pub texture: Texture2D,
    pub rbo: GLuint,
}

impl RenderTexture2D {
    pub fn new(width: i32, height: i32) -> Self {
        let mut fbo = 0;
        let mut rbo = 0;
        let texture = Texture2D::from_data(&vec![0u8; (width * height * 4) as usize], width, height);

        unsafe {
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);

            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                texture.id,
                0,
            );

            gl::GenRenderbuffers(1, &mut rbo);
            gl::BindRenderbuffer(gl::RENDERBUFFER, rbo);
            gl::RenderbufferStorage(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, width, height);
            gl::FramebufferRenderbuffer(
                gl::FRAMEBUFFER,
                gl::DEPTH_STENCIL_ATTACHMENT,
                gl::RENDERBUFFER,
                rbo,
            );

            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                eprintln!("Error: Framebuffer is not complete!");
            }

            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }

        Self { fbo, texture, rbo }
    }

    pub fn bind(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, self.texture.width, self.texture.height);
        }
    }

    pub fn unbind() {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
        }
    }
}

impl Drop for RenderTexture2D {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteRenderbuffers(1, &self.rbo);
            gl::DeleteFramebuffers(1, &self.fbo);
        }
    }
}
