//! The 3D view's drawing (OpenGL through `egui_glow`'s callback): the model's mesh lit, NM's capsules and boxes
//! translucent over it, lines for the bones, with a depth buffer of its own; and the camera that frames it all.

use std::sync::{Arc, Mutex};

use delirium::prelude::*;
use eframe::glow::{self, HasContext};

/// A vertex: position (m, Source's axes), normal, colour (RGBA; alpha below 1 draws without writing depth).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// Triangles or lines to draw this frame.
#[derive(Default)]
pub struct Batch {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
    pub lines: bool,
    /// Drawn over everything (handles, bones).
    pub on_top: bool,
}

impl Batch {
    pub fn tri(&mut self, a: Vertex, b: Vertex, c: Vertex) {
        let n = self.verts.len() as u32;
        self.verts.extend([a, b, c]);
        self.idx.extend([n, n + 1, n + 2]);
    }

    pub fn line(&mut self, a: Vec3, b: Vec3, color: [f32; 4]) {
        let n = self.verts.len() as u32;
        self.verts.push(Vertex { pos: a.to_array(), normal: [0.0, 0.0, 1.0], color });
        self.verts.push(Vertex { pos: b.to_array(), normal: [0.0, 0.0, 1.0], color });
        self.idx.extend([n, n + 1]);
    }

    /// A convex hull's triangles (`dms_vphysics::hull`), flat shaded, placed by `f`.
    pub fn hull(&mut self, points: &[Vec3], tris: &[[usize; 3]], f: Transform, color: [f32; 4]) {
        for t in tris {
            let [a, b, c] = t.map(|i| f.transform_point(points[i]));
            let n = (b - a).cross(c - a).normalize_or_zero().to_array();
            let v = |p: Vec3| Vertex { pos: p.to_array(), normal: n, color };
            self.tri(v(a), v(b), v(c));
        }
    }
}

/// An orbiting camera about `target` (m), Z up.
#[derive(Clone, Copy)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera { target: Vec3::new(0.0, 0.0, 0.9), yaw: 0.6, pitch: 0.15, distance: 3.2 }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        self.target + Vec3::new(self.yaw.cos() * self.pitch.cos(), self.yaw.sin() * self.pitch.cos(), self.pitch.sin()) * self.distance
    }

    /// World to clip space for a view `aspect` wide.
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        // OpenGL's right-handed perspective and look-at, written out.
        let (near, far, f) = (0.02f32, 200.0f32, 1.0 / (25f32.to_radians()).tan());
        let proj = Mat4::from_cols(
            Vec4::new(f / aspect.max(0.01), 0.0, 0.0, 0.0),
            Vec4::new(0.0, f, 0.0, 0.0),
            Vec4::new(0.0, 0.0, (far + near) / (near - far), -1.0),
            Vec4::new(0.0, 0.0, 2.0 * far * near / (near - far), 0.0),
        );
        let eye = self.eye();
        let fw = (self.target - eye).normalize();
        let s = fw.cross(Vec3::Z).normalize_or(Vec3::Y);
        let u = s.cross(fw);
        let view = Mat4::from_cols(
            Vec4::new(s.x, u.x, -fw.x, 0.0),
            Vec4::new(s.y, u.y, -fw.y, 0.0),
            Vec4::new(s.z, u.z, -fw.z, 0.0),
            Vec4::new(-s.dot(eye), -u.dot(eye), fw.dot(eye), 1.0),
        );
        proj * view
    }
}

/// Projection of world points to the view's pixels and rays back.
#[derive(Clone, Copy)]
pub struct View {
    pub m: Mat4,
    pub rect: eframe::egui::Rect,
    pub eye: Vec3,
}

impl View {
    pub fn new(cam: &Camera, rect: eframe::egui::Rect) -> View {
        View { m: cam.matrix(rect.width() / rect.height().max(1.0)), rect, eye: cam.eye() }
    }

    /// A world point on the screen, `None` behind the eye.
    pub fn project(&self, p: Vec3) -> Option<eframe::egui::Pos2> {
        let c = self.m * p.extend(1.0);
        if c.w <= 1e-4 {
            return None;
        }
        let n = c.truncate() / c.w;
        Some(eframe::egui::pos2(self.rect.left() + (n.x * 0.5 + 0.5) * self.rect.width(), self.rect.top() + (0.5 - n.y * 0.5) * self.rect.height()))
    }

    /// The ray through a screen point: origin and direction.
    pub fn ray(&self, s: eframe::egui::Pos2) -> (Vec3, Vec3) {
        let x = (s.x - self.rect.left()) / self.rect.width() * 2.0 - 1.0;
        let y = 1.0 - (s.y - self.rect.top()) / self.rect.height() * 2.0;
        let inv = self.m.inverse();
        let a = inv * Vec4::new(x, y, -1.0, 1.0);
        let b = inv * Vec4::new(x, y, 1.0, 1.0);
        let (a, b) = (a.truncate() / a.w, b.truncate() / b.w);
        (a, (b - a).normalize_or(Vec3::X))
    }

    /// Where the ray through `s` meets the plane through `p` facing the eye.
    pub fn on_plane(&self, s: eframe::egui::Pos2, p: Vec3) -> Vec3 {
        let (o, d) = self.ray(s);
        let n = (self.eye - p).normalize_or(Vec3::X);
        let den = d.dot(n);
        if den.abs() < 1e-6 {
            return p;
        }
        o + d * ((p - o).dot(n) / den)
    }

    /// The world length a pixel spans at `p` (m): handles keep their size on the screen.
    pub fn pixel(&self, p: Vec3) -> f32 {
        let side = (self.eye - p).cross(Vec3::Z).normalize_or(Vec3::X);
        match (self.project(p), self.project(p + side * 0.1)) {
            (Some(a), Some(b)) if a.distance(b) > 1e-3 => 0.1 / a.distance(b),
            _ => 0.001,
        }
    }

    /// How far along `dir` (a world direction from `p`) a mouse move of `delta` pixels goes (m).
    pub fn along(&self, p: Vec3, dir: Vec3, delta: eframe::egui::Vec2) -> f32 {
        let (Some(a), Some(b)) = (self.project(p), self.project(p + dir * 0.1)) else { return 0.0 };
        let d = b - a;
        let len2 = d.length_sq();
        if len2 < 1e-3 {
            return 0.0;
        }
        delta.dot(d) / len2 * 0.1
    }
}

const VS: &str = r#"
in vec3 a_pos;
in vec3 a_normal;
in vec4 a_color;
uniform mat4 u_m;
out vec3 v_normal;
out vec4 v_color;
void main() {
    gl_Position = u_m * vec4(a_pos, 1.0);
    v_normal = a_normal;
    v_color = a_color;
}
"#;

const FS: &str = r#"
precision mediump float;
in vec3 v_normal;
in vec4 v_color;
uniform float u_lit;
uniform float u_alpha;
out vec4 o_color;
void main() {
    vec3 n = normalize(v_normal);
    // Two-sided: a mesh seen from inside is lit as from outside.
    float l = mix(1.0, 0.35 + 0.65 * abs(dot(n, normalize(vec3(0.4, 0.3, 0.85)))) + 0.15 * max(dot(n, normalize(vec3(-0.5, -0.6, 0.2))), 0.0), u_lit);
    o_color = vec4(v_color.rgb * l, v_color.a * u_alpha);
}
"#;

struct Gpu {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    /// The model's mesh, uploaded once per change: its buffers and index count.
    mesh: Option<(glow::VertexArray, glow::Buffer, glow::Buffer, i32, u64)>,
}

/// What a frame draws: the mesh (kept on the GPU while `mesh_version` stays) and this frame's batches.
#[derive(Default)]
pub struct Scene {
    pub m: Mat4,
    pub mesh: Option<Arc<Batch>>,
    pub mesh_version: u64,
    /// The mesh's opacity: below 1 it's drawn after the solid batches without writing depth, the parts seen through.
    pub mesh_alpha: f32,
    pub batches: Vec<Batch>,
}

/// The renderer's GPU side, made on the first paint.
#[derive(Default)]
pub struct Renderer {
    gpu: Option<Gpu>,
}

unsafe fn vertex_layout(gl: &glow::Context) {
    let stride = std::mem::size_of::<Vertex>() as i32;
    // SAFETY: the bound VAO and buffer; attribute locations bound before linking.
    unsafe {
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 3, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride, 24);
    }
}

fn bytes<T>(v: &[T]) -> &[u8] {
    // SAFETY: plain `repr(C)` floats and `u32`s, read as bytes.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}

impl Renderer {
    unsafe fn init(gl: &glow::Context, shader_version: &str) -> Result<Gpu, String> {
        // SAFETY: the callback's live context, on the paint thread.
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = vec![];
            for (kind, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, FS)] {
                let s = gl.create_shader(kind)?;
                gl.shader_source(s, &format!("{shader_version}\n{src}"));
                gl.compile_shader(s);
                if !gl.get_shader_compile_status(s) {
                    return Err(gl.get_shader_info_log(s));
                }
                gl.attach_shader(program, s);
                shaders.push(s);
            }
            gl.bind_attrib_location(program, 0, "a_pos");
            gl.bind_attrib_location(program, 1, "a_normal");
            gl.bind_attrib_location(program, 2, "a_color");
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            for s in shaders {
                gl.detach_shader(program, s);
                gl.delete_shader(s);
            }
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            let ebo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
            vertex_layout(gl);
            gl.bind_vertex_array(None);
            Ok(Gpu { program, vao, vbo, ebo, mesh: None })
        }
    }

    /// Draws `scene` into the callback's viewport.
    pub fn paint(&mut self, info: &eframe::egui::PaintCallbackInfo, painter: &eframe::egui_glow::Painter, scene: &Scene) {
        let gl = painter.gl();
        if self.gpu.is_none() {
            // SAFETY: as in `init`.
            match unsafe { Renderer::init(gl, "#version 140") } {
                Ok(g) => self.gpu = Some(g),
                Err(e) => {
                    eprintln!("dms-rig: the 3D view's shaders: {e}");
                    return;
                }
            }
        }
        let Some(g) = self.gpu.as_mut() else { return };
        let vp = info.viewport_in_pixels();
        let clip = info.clip_rect_in_pixels();
        // SAFETY: egui_glow's live context inside its paint callback; every buffer is ours.
        unsafe {
            gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(clip.left_px, clip.from_bottom_px, clip.width_px, clip.height_px);
            gl.clear_color(0.13, 0.14, 0.16, 1.0);
            gl.depth_mask(true);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::CULL_FACE);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(g.program));
            gl.uniform_matrix_4_f32_slice(gl.get_uniform_location(g.program, "u_m").as_ref(), false, &scene.m.to_cols_array());
            let lit = gl.get_uniform_location(g.program, "u_lit");
            let alpha = gl.get_uniform_location(g.program, "u_alpha");
            gl.uniform_1_f32(alpha.as_ref(), 1.0);
            let clear = |b: &Batch| b.verts.first().is_some_and(|v| v.color[3] < 1.0);
            let draw = |g: &Gpu, b: &Batch| {
                if b.idx.is_empty() {
                    return;
                }
                gl.depth_mask(!clear(b) && !b.on_top);
                if b.on_top {
                    gl.disable(glow::DEPTH_TEST);
                } else {
                    gl.enable(glow::DEPTH_TEST);
                }
                gl.uniform_1_f32(lit.as_ref(), if b.lines { 0.0 } else { 1.0 });
                gl.bind_vertex_array(Some(g.vao));
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(g.vbo));
                gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(g.ebo));
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(&b.verts), glow::STREAM_DRAW);
                gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytes(&b.idx), glow::STREAM_DRAW);
                gl.draw_elements(if b.lines { glow::LINES } else { glow::TRIANGLES }, b.idx.len() as i32, glow::UNSIGNED_INT, 0);
            };
            // Solid first, then the mesh, then what's translucent, then what's on top.
            for b in scene.batches.iter().filter(|b| !b.on_top && !clear(b)) {
                draw(g, b);
            }
            if let Some(mesh) = &scene.mesh {
                if g.mesh.as_ref().is_none_or(|m| m.4 != scene.mesh_version) {
                    if let Some((vao, vbo, ebo, _, _)) = g.mesh.take() {
                        gl.delete_vertex_array(vao);
                        gl.delete_buffer(vbo);
                        gl.delete_buffer(ebo);
                    }
                    if let (Ok(vao), Ok(vbo), Ok(ebo)) = (gl.create_vertex_array(), gl.create_buffer(), gl.create_buffer()) {
                        gl.bind_vertex_array(Some(vao));
                        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(&mesh.verts), glow::STATIC_DRAW);
                        gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(ebo));
                        gl.buffer_data_u8_slice(glow::ELEMENT_ARRAY_BUFFER, bytes(&mesh.idx), glow::STATIC_DRAW);
                        vertex_layout(gl);
                        g.mesh = Some((vao, vbo, ebo, mesh.idx.len() as i32, scene.mesh_version));
                    }
                }
                if let Some((vao, _, _, n, _)) = g.mesh {
                    gl.enable(glow::DEPTH_TEST);
                    gl.depth_mask(scene.mesh_alpha >= 1.0);
                    gl.uniform_1_f32(lit.as_ref(), 1.0);
                    gl.uniform_1_f32(alpha.as_ref(), scene.mesh_alpha);
                    gl.bind_vertex_array(Some(vao));
                    gl.draw_elements(glow::TRIANGLES, n, glow::UNSIGNED_INT, 0);
                    gl.uniform_1_f32(alpha.as_ref(), 1.0);
                }
            }
            for b in scene.batches.iter().filter(|b| !b.on_top && clear(b)) {
                draw(g, b);
            }
            for b in scene.batches.iter().filter(|b| b.on_top) {
                draw(g, b);
            }
            gl.bind_vertex_array(None);
            gl.depth_mask(true);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
        }
    }
}

/// The renderer shared with egui's paint callback.
pub type Shared = Arc<Mutex<Renderer>>;
