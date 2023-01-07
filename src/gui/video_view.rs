use miniquad::*;

/// Takes NV21 video range frames and draws them into a RGBA texture.
pub struct VideoView {
    pipeline: Pipeline,
    bindings: Bindings,
    pass: RenderPass,
    render_texture: Texture,
    texture_y: Texture,
    texture_uv: Texture,
}

impl VideoView {
    pub fn new(ctx: &mut Context) -> Self {
        let vertex_attributes = [
            VertexAttribute::new("pos", VertexFormat::Float3),
            VertexAttribute::new("uv", VertexFormat::Float2),
        ];
        // quad
        let vertices: &[[f32; 5]] = &[
            // x y z u v
            [-1.0, -1.0, 0.0, 0.0, 0.0],
            [1.0, -1.0, 0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0, 1.0, 1.0],
            [-1.0, 1.0, 0.0, 0.0, 1.0],
        ];
        let vertex_buffer = Buffer::immutable(ctx, BufferType::VertexBuffer, vertices);
        let vertex_buffer_layout = BufferLayout {
            stride: vertex_attributes.iter().map(|a| a.format.byte_len()).sum(),
            ..Default::default()
        };

        let indices: &[u16] = &[0, 1, 2, 0, 2, 3];
        let index_buffer = Buffer::immutable(ctx, BufferType::IndexBuffer, indices);

        let shader = offscreen_shader::new(ctx);
        let pipeline = Pipeline::with_params(
            ctx,
            &[vertex_buffer_layout],
            &vertex_attributes,
            shader,
            PipelineParams {
                depth_test: Comparison::LessOrEqual,
                depth_write: true,
                ..Default::default()
            },
        );

        let texture_y =
            Texture::from_data_and_format(ctx, &[], texture_params(TextureFormat::Alpha));
        let texture_uv =
            Texture::from_data_and_format(ctx, &[], texture_params(TextureFormat::LuminanceAlpha));
        let render_texture = Texture::new_render_texture(ctx, TextureParams::default());

        Self {
            pipeline,
            render_texture,
            texture_y,
            texture_uv,
            pass: RenderPass::new(ctx, render_texture, None),
            bindings: Bindings {
                vertex_buffers: vec![vertex_buffer],
                index_buffer,
                images: vec![texture_y, texture_uv],
            },
        }
    }

    pub fn width(&self) -> u32 {
        self.render_texture.width
    }

    pub fn height(&self) -> u32 {
        self.render_texture.height
    }

    pub fn update(&mut self, ctx: &mut Context, yuv: &[u8], width: u32, height: u32) {
        let (y, uv) = yuv.split_at((width * height) as _);
        let uv = &uv[..(width * height / 2) as usize];

        if self.width() != width || self.height() != height {
            self.texture_y.resize(ctx, width, height, Some(y));
            self.texture_uv.resize(ctx, width / 2, height / 2, Some(uv));
            self.render_texture.resize(ctx, width, height, None);
            self.pass = RenderPass::new(ctx, self.render_texture, None);
        } else {
            self.texture_y.update(ctx, y);
            self.texture_uv.update(ctx, uv);
        }
    }

    pub fn draw(&mut self, ctx: &mut Context) -> Texture {
        ctx.begin_pass(self.pass, PassAction::clear_color(0.0, 1.0, 1.0, 1.));
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);
        ctx.draw(0, 6, 1);
        ctx.end_render_pass();
        self.pass.texture(ctx)
    }
}

fn texture_params(format: TextureFormat) -> TextureParams {
    TextureParams {
        format,
        wrap: TextureWrap::Clamp,
        filter: FilterMode::Nearest,
        ..Default::default()
    }
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
        lowp vec3 yuv, rgb;
        lowp vec3 yuv2r = vec3(1.164, 0.0, 1.596);
        lowp vec3 yuv2g = vec3(1.164, -0.391, -0.813);
        lowp vec3 yuv2b = vec3(1.164, 2.018, 0.0);

        yuv.x = texture2D(tex_y, frag_uv).a - 0.0625;
        yuv.y = texture2D(tex_uv, frag_uv).r - 0.5;
        yuv.z = texture2D(tex_uv, frag_uv).a - 0.5;

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
