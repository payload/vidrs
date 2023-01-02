use miniquad::*;

#[repr(C)]
struct Vec2 {
    x: f32,
    y: f32,
}
#[repr(C)]
struct Vertex {
    pos: Vec2,
    uv: Vec2,
}

struct Stage {
    pipeline: Pipeline,
    bindings: Bindings,
}

fn quad_vertices() -> ([Vertex; 4], [u16; 6]) {
    let vertex = |x, y, u, v| Vertex {
        pos: Vec2 { x, y },
        uv: Vec2 { x: u, y: v },
    };
    (
        [
            vertex(-0.5, -0.5, 0., 0.),
            vertex(0.5, -0.5, 1., 0.),
            vertex(0.5, 0.5, 1., 1.),
            vertex(-0.5, 0.5, 0., 1.),
        ],
        [0, 1, 2, 0, 2, 3],
    )
}

fn quad_buffers(ctx: &mut Context) -> (Buffer, Buffer) {
    let (vertices, indices) = quad_vertices();
    (
        Buffer::immutable(ctx, BufferType::VertexBuffer, &vertices),
        Buffer::immutable(ctx, BufferType::IndexBuffer, &indices),
    )
}

impl Stage {
    pub fn new(ctx: &mut Context) -> Stage {
        let (vertex_buffer, index_buffer) = quad_buffers(ctx);
        let yuv = include_bytes!("../camera_frame.420v.1280.720");
        let w = 1280;
        let h = 720;
        let y_len = w * h;
        let uv_len = w * h / 2;

        let texture_y = Texture::from_data_and_format(
            ctx,
            &yuv[..y_len],
            TextureParams {
                format: TextureFormat::Alpha,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Nearest,
                width: w as _,
                height: h as _,
            },
        );
        let texture_uv = Texture::from_data_and_format(
            ctx,
            &yuv[y_len..(y_len + uv_len)],
            TextureParams {
                format: TextureFormat::Depth,
                wrap: TextureWrap::Clamp,
                filter: FilterMode::Nearest,
                width: (w / 2) as _,
                height: (h / 2) as _,
            },
        );

        let bindings = Bindings {
            vertex_buffers: vec![vertex_buffer],
            index_buffer: index_buffer,
            images: vec![texture_y, texture_uv],
        };

        let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::meta()).unwrap();

        let pipeline = Pipeline::new(
            ctx,
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
            ],
            shader,
        );

        Stage { pipeline, bindings }
    }
}

impl EventHandler for Stage {
    fn update(&mut self, _ctx: &mut Context) {}

    fn draw(&mut self, ctx: &mut Context) {
        let t = date::now();

        ctx.begin_default_pass(Default::default());

        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);
        for i in 0..10 {
            let t = t + i as f64 * 0.3;

            ctx.apply_uniforms(&shader::Uniforms {
                offset: (t.sin() as f32 * 0.5, (t * 3.).cos() as f32 * 0.5),
            });
            ctx.draw(0, 6, 1);
        }
        ctx.end_render_pass();

        ctx.commit_frame();
    }
}

fn main() {
    miniquad::start(conf::Conf::default(), |mut ctx| {
        Box::new(Stage::new(&mut ctx))
    });
}

mod shader {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 100
    attribute vec2 pos;
    attribute vec2 uv;
    uniform vec2 offset;
    varying lowp vec2 xy;

    void main() {
        gl_Position = vec4(pos + offset, 0, 1);
        xy = uv;
    }
    "#;

    pub const FRAGMENT: &str = r#"#version 100
    varying lowp vec2 xy;
    uniform sampler2D tex_y;
    uniform sampler2D tex_uv;

    void main() {
        lowp float r, g, b, y, u, v;

        y = texture2D(tex_y, vec2(xy.x, 1.0 - xy.y)).r;
        lowp vec4 uv = texture2D(tex_uv, vec2(xy.x, 1.0 - xy.y));
        u = uv.r - 0.5;
        v = uv.g - 0.5;

        r = y + 1.13983*v;
        g = y - 0.39465*u - 0.58060*v;
        b = y + 2.03211*u;

        // gl_FragColor = vec4(y, y, y, 1.0);
        // gl_FragColor = vec4(u, u, u, 1.0);
        // gl_FragColor = vec4(v, v, v, 1.0);
        gl_FragColor = vec4(r, g, b, 1.0);
    }
    "#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: ["tex_y", "tex_uv"].into_iter().map(String::from).collect(),
            uniforms: UniformBlockLayout {
                uniforms: vec![UniformDesc::new("offset", UniformType::Float2)],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub offset: (f32, f32),
    }
}
