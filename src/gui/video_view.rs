use miniquad::*;

pub struct VideoView {
    pipeline: Pipeline,
    quad: Bindings,
    pass: RenderPass,
    nv21: NV21Textures,
}

impl VideoView {
    pub fn new(ctx: &mut Context) -> Self {
        Self {
            pipeline: new_offscreen_quad_pipeline(ctx),
            pass: new_render_pass(ctx),
            quad: new_quad(ctx),
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
        if self.quad.images.is_empty() {
            self.quad.images.push(self.nv21.texture_y);
            self.quad.images.push(self.nv21.texture_uv);
        }

        ctx.begin_pass(self.pass, PassAction::clear_color(0.0, 1.0, 1.0, 1.));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.quad);
        ctx.draw(0, 6, 1); // the number of indices?
        ctx.end_render_pass();
        self.pass.texture(ctx)
    }
}

fn new_offscreen_quad_pipeline(ctx: &mut Context) -> Pipeline {
    let vertex_attributes = [
        VertexAttribute::new("pos", VertexFormat::Float3),
        VertexAttribute::new("uv", VertexFormat::Float2),
    ];
    let buffer_layout = [BufferLayout {
        stride: 20,
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
        // x y z            u v
        -1.0, -1.0, 0.0,    0.0, 0.0,
         1.0, -1.0, 0.0,    1.0, 0.0,
         1.0,  1.0, 0.0,    1.0, 1.0,
        -1.0,  1.0, 0.0,    0.0, 1.0,
    ];
    let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices);

    let indices: &[u16] = &[0, 1, 2, 0, 2, 3];
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
    attribute vec2 uv;

    varying lowp vec2 frag_uv;

    void main() {
        gl_Position = pos;
        frag_uv = uv;
    }
    "#;

    pub const FRAGMENT: &str = r#"#version 100
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
    }
    "#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex_y".to_string(), "tex_uv".to_string()],
            uniforms: UniformBlockLayout { uniforms: vec![] },
        }
    }

    pub fn new(ctx: &mut Context) -> Shader {
        Shader::new(ctx, VERTEX, FRAGMENT, meta()).unwrap()
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
