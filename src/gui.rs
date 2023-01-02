use egui_miniquad as egui_mq;
use miniquad::*;

struct Stage {
    egui_mq: egui_mq::EguiMq,
    video_view: VideoView,
}

pub fn run_gui() {
    miniquad::start(conf::Conf::default(), |ctx| Box::new(Stage::new(ctx)));
}

impl Stage {
    fn new(ctx: &mut Context) -> Self {
        Self {
            egui_mq: egui_mq::EguiMq::new(ctx),
            video_view: VideoView::new(ctx),
        }
    }

    fn draw_egui(&mut self, ctx: &mut Context) {
        let video_texture = self.video_view.draw_offscreen(ctx);
        let video_texture_id = egui::TextureId::User(video_texture.gl_internal_id() as _);

        self.egui_mq.run(ctx, |_mq_ctx, egui_ctx| {
            egui::Window::new("vidrs").show(egui_ctx, |ui| {
                ui.image(video_texture_id, egui::Vec2::new(256.0, 256.0));

                #[cfg(not(target_arch = "wasm32"))]
                {
                    if ui.button("Quit").clicked() {
                        // TODO tell the other that we are exitting
                        std::process::exit(0);
                    }
                }
            });
        });

        self.egui_mq.draw(ctx);

        ctx.commit_frame();
    }
}

impl EventHandler for Stage {
    fn update(&mut self, _ctx: &mut Context) {}

    fn draw(&mut self, ctx: &mut Context) {
        ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        ctx.end_render_pass();

        self.draw_egui(ctx);

        ctx.commit_frame();
    }

    fn mouse_motion_event(&mut self, _: &mut Context, x: f32, y: f32) {
        self.egui_mq.mouse_motion_event(x, y);
    }

    fn mouse_wheel_event(&mut self, _: &mut Context, dx: f32, dy: f32) {
        self.egui_mq.mouse_wheel_event(dx, dy);
    }

    fn mouse_button_down_event(&mut self, ctx: &mut Context, mb: MouseButton, x: f32, y: f32) {
        self.egui_mq.mouse_button_down_event(ctx, mb, x, y);
    }

    fn mouse_button_up_event(&mut self, ctx: &mut Context, mb: MouseButton, x: f32, y: f32) {
        self.egui_mq.mouse_button_up_event(ctx, mb, x, y);
    }

    fn char_event(&mut self, _ctx: &mut Context, character: char, _: KeyMods, _: bool) {
        self.egui_mq.char_event(character);
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: KeyCode, keymods: KeyMods, _: bool) {
        self.egui_mq.key_down_event(ctx, keycode, keymods);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, keymods: KeyMods) {
        self.egui_mq.key_up_event(keycode, keymods);
    }
}

struct VideoView {
    pipeline: Pipeline,
    quad: Bindings,
    pass: RenderPass,
}

impl VideoView {
    fn new(ctx: &mut Context) -> Self {
        Self {
            pipeline: new_offscreen_quad_pipeline(ctx),
            pass: new_render_pass(ctx),
            quad: new_quad(ctx),
        }
    }

    fn draw_offscreen(&self, ctx: &mut Context) -> Texture {
        use glam::*;

        let (width, height) = ctx.screen_size();
        let proj = Mat4::perspective_rh_gl(60.0f32.to_radians(), width / height, 0.01, 10.0);
        let view = Mat4::look_at_rh(
            vec3(0.0, 1.5, 3.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
        );
        let view_proj = proj * view;
        let uniforms = offscreen_shader::Uniforms { mvp: view_proj };

        ctx.begin_pass(self.pass, PassAction::clear_color(0.0, 1.0, 1.0, 1.));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.quad);
        ctx.apply_uniforms(&uniforms);
        ctx.draw(0, 6, 1); // 6 is the number of indices?
        ctx.end_render_pass();
        self.pass.texture(ctx)
    }
}

fn new_offscreen_quad_pipeline(ctx: &mut Context) -> Pipeline {
    let vertex_attributes = [
        VertexAttribute::new("pos", VertexFormat::Float2),
        VertexAttribute::new("uv", VertexFormat::Float2),
    ];
    let buffer_layout = [BufferLayout {
        stride: vertex_attributes.iter().map(|a| a.format.byte_len()).sum(),
        ..Default::default()
    }];
    let shader = offscreen_shader::new(ctx);
    Pipeline::with_params(
        ctx,
        &buffer_layout,
        &vertex_attributes,
        shader,
        PipelineParams {
            depth_test: Comparison::LessOrEqual,
            depth_write: true,
            ..Default::default()
        },
    )
}

fn new_quad(ctx: &mut Context) -> Bindings {
    let vertices = [
        (-0.5, -0.5, 0., 0.),
        (0.5, -0.5, 1., 0.),
        (0.5, 0.5, 1., 1.),
        (-0.5, 0.5, 0., 1.),
    ];
    let vertex_buffers = vec![Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices)];
    let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &[0, 1, 2, 0, 2, 3]);
    Bindings {
        vertex_buffers,
        index_buffer,
        images: vec![],
    }
}

fn new_render_pass(ctx: &mut Context) -> RenderPass {
    let color_img = Texture::new_render_texture(
        ctx,
        TextureParams {
            width: 256,
            height: 256,
            format: TextureFormat::RGBA8,
            ..Default::default()
        },
    );
    let depth_img = Texture::new_render_texture(
        ctx,
        TextureParams {
            width: 256,
            height: 256,
            format: TextureFormat::Depth,
            ..Default::default()
        },
    );

    RenderPass::new(ctx, color_img, depth_img)
}

mod offscreen_shader {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec4 pos;
    attribute vec2 uv;

    varying lowp vec2 tex_uv;

    uniform mat4 mvp;

    void main() {
        gl_Position = pos * mvp;
        tex_uv = uv;
    }
    "#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 tex_uv;

    void main() {
        gl_FragColor = vec4(1.0, 0.0, 0.0, 1.0);
    }
    "#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec![],
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("mvp", UniformType::Mat4)],
            },
        }
    }

    pub fn new(ctx: &mut Context) -> Shader {
        Shader::new(ctx, VERTEX, FRAGMENT, meta()).unwrap()
    }

    #[repr(C)]
    pub struct Uniforms {
        pub mvp: glam::Mat4,
    }
}
