use miniquad::*;

pub struct VideoView {
    pipeline: Pipeline,
    quad: Bindings,
    pass: RenderPass,
    rx: f32,
    ry: f32,
    nv21: NV21Textures,
}

impl VideoView {
    pub fn new(ctx: &mut Context) -> Self {
        Self {
            pipeline: new_offscreen_quad_pipeline(ctx),
            pass: new_render_pass(ctx),
            quad: new_quad(ctx),
            rx: 0.0,
            ry: 0.0,
            nv21: NV21Textures::new(ctx, &[], 0, 0),
        }
    }

    pub fn width(&self) -> u32 {
        self.nv21.texture_y.width
    }

    pub fn height(&self) -> u32 {
        self.nv21.texture_y.height
    }

    pub fn update(&mut self, ctx: &mut Context, yuv: &[u8], width: u32, height: u32) {
        self.nv21.update(ctx, yuv, width, height);
    }

    pub fn draw(&mut self, ctx: &mut Context) -> Texture {
        use glam::*;

        if self.quad.images.is_empty() {
            self.quad.images.push(self.nv21.texture_y);
            self.quad.images.push(self.nv21.texture_uv);
        }

        let (width, height) = ctx.screen_size();
        let proj = Mat4::perspective_rh_gl(60.0f32.to_radians(), width / height, 0.01, 10.0);
        let view = Mat4::look_at_rh(
            vec3(0.0, 1.5, 3.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
        );
        let view_proj = proj * view;
        // self.rx += 0.01;
        // self.ry += 0.03;
        let mvp = view_proj * Mat4::from_rotation_x(self.rx) * Mat4::from_rotation_y(self.ry);
        let uniforms = offscreen_shader::Uniforms { mvp };

        ctx.begin_pass(self.pass, PassAction::clear_color(0.0, 1.0, 1.0, 1.));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.quad);
        ctx.apply_uniforms(&uniforms);
        ctx.draw(0, 6, 1); // the number of indices?
        ctx.end_render_pass();
        self.pass.texture(ctx)
    }
}

fn new_offscreen_quad_pipeline(ctx: &mut Context) -> Pipeline {
    let vertex_attributes = [
        VertexAttribute::new("pos", VertexFormat::Float3),
        VertexAttribute::new("color", VertexFormat::Float4),
        VertexAttribute::new("uv", VertexFormat::Float2),
    ];
    let buffer_layout = [BufferLayout {
        stride: 36,
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
    #[rustfmt::skip]
    let vertices: &[f32] = &[
        /* pos               color                   uvs */
        -1.0, -1.0, -1.0,    1.0, 0.5, 0.5, 1.0,     0.0, 0.0,
         1.0, -1.0, -1.0,    1.0, 0.5, 0.5, 1.0,     1.0, 0.0,
         1.0,  1.0, -1.0,    1.0, 0.5, 0.5, 1.0,     1.0, 1.0,
        -1.0,  1.0, -1.0,    1.0, 0.5, 0.5, 1.0,     0.0, 1.0,

        -1.0, -1.0,  1.0,    0.5, 1.0, 0.5, 1.0,     0.0, 0.0,
         1.0, -1.0,  1.0,    0.5, 1.0, 0.5, 1.0,     1.0, 0.0,
         1.0,  1.0,  1.0,    0.5, 1.0, 0.5, 1.0,     1.0, 1.0,
        -1.0,  1.0,  1.0,    0.5, 1.0, 0.5, 1.0,     0.0, 1.0,

        -1.0, -1.0, -1.0,    0.5, 0.5, 1.0, 1.0,     0.0, 0.0,
        -1.0,  1.0, -1.0,    0.5, 0.5, 1.0, 1.0,     1.0, 0.0,
        -1.0,  1.0,  1.0,    0.5, 0.5, 1.0, 1.0,     1.0, 1.0,
        -1.0, -1.0,  1.0,    0.5, 0.5, 1.0, 1.0,     0.0, 1.0,

         1.0, -1.0, -1.0,    1.0, 0.5, 0.0, 1.0,     0.0, 0.0,
         1.0,  1.0, -1.0,    1.0, 0.5, 0.0, 1.0,     1.0, 0.0,
         1.0,  1.0,  1.0,    1.0, 0.5, 0.0, 1.0,     1.0, 1.0,
         1.0, -1.0,  1.0,    1.0, 0.5, 0.0, 1.0,     0.0, 1.0,

        -1.0, -1.0, -1.0,    0.0, 0.5, 1.0, 1.0,     0.0, 0.0,
        -1.0, -1.0,  1.0,    0.0, 0.5, 1.0, 1.0,     1.0, 0.0,
         1.0, -1.0,  1.0,    0.0, 0.5, 1.0, 1.0,     1.0, 1.0,
         1.0, -1.0, -1.0,    0.0, 0.5, 1.0, 1.0,     0.0, 1.0,

        -1.0,  1.0, -1.0,    1.0, 0.0, 0.5, 1.0,     0.0, 0.0,
        -1.0,  1.0,  1.0,    1.0, 0.0, 0.5, 1.0,     1.0, 0.0,
         1.0,  1.0,  1.0,    1.0, 0.0, 0.5, 1.0,     1.0, 1.0,
         1.0,  1.0, -1.0,    1.0, 0.0, 0.5, 1.0,     0.0, 1.0
    ];

    let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);

    #[rustfmt::skip]
    let indices: &[u16] = &[
         0,  1,  2,   0,  2,  3,
         6,  5,  4,   7,  6,  4,
         8,  9, 10,   8, 10, 11,
        14, 13, 12,  15, 14, 12,
        16, 17, 18,  16, 18, 19,
        22, 21, 20,  23, 22, 20
    ];

    let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, &indices);
    Bindings {
        vertex_buffers: vec![vertex_buffer],
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
    attribute vec4 color;
    attribute vec2 uv;

    varying lowp vec4 frag_color;
    varying lowp vec2 frag_uv;

    uniform mat4 mvp;

    void main() {
        gl_Position = mvp * pos;
        frag_color = color;
        frag_uv = uv;
    }
    "#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec4 frag_color;
    varying lowp vec2 frag_uv;

    uniform sampler2D tex_y;
    uniform sampler2D tex_uv;

    void main() {
        lowp vec2 coord = frag_uv;

        lowp vec3 yuv, rgb;
        lowp vec3 yuv2r = vec3(1.164, 0.0, 1.596);
        lowp vec3 yuv2g = vec3(1.164, -0.391, -0.813);
        lowp vec3 yuv2b = vec3(1.164, 2.018, 0.0);

        yuv.x = texture2D(tex_y, coord).a - 0.0625;
        yuv.y = texture2D(tex_uv, coord).r - 0.5;
        yuv.z = texture2D(tex_uv, coord).a - 0.5;

        rgb.x = dot(yuv, yuv2r);
        rgb.y = dot(yuv, yuv2g);
        rgb.z = dot(yuv, yuv2b);

        gl_FragColor = vec4(rgb, 1.0);
        // gl_FragColor = vec4(frag_uv, 0.0, 1.0);
        // gl_FragColor = vec4(texture2D(tex_y, coord).a, 0.0, 0.0, 1.0);
    }
    "#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex_y".to_string(), "tex_uv".to_string()],
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

struct NV21Textures {
    texture_y: Texture,
    texture_uv: Texture,
}

impl NV21Textures {
    pub fn new(ctx: &mut Context, yuv: &[u8], w: u32, h: u32) -> Self {
        let (y, uv) = yuv.split_at((w * h) as _);

        let texture_y = Texture::from_data_and_format(
            ctx,
            y,
            TextureParams {
                format: TextureFormat::Alpha,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Nearest,
                width: w,
                height: h,
            },
        );
        let texture_uv = Texture::from_data_and_format(
            ctx,
            uv,
            TextureParams {
                format: TextureFormat::LuminanceAlpha,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Nearest,
                width: (w / 2),
                height: (h / 2),
            },
        );

        Self {
            texture_y,
            texture_uv,
        }
    }

    pub fn update(&mut self, ctx: &mut Context, yuv: &[u8], width: u32, height: u32) {
        let (y, uv) = yuv.split_at((width * height) as _);

        if self.texture_y.width != width || self.texture_y.height != height {
            self.texture_y.resize(ctx, width, height, Some(y));
            self.texture_uv.resize(ctx, width / 2, height / 2, Some(uv));
        } else {
            self.texture_y.update(ctx, y);
            self.texture_uv.update(ctx, uv);
        }
    }
}
