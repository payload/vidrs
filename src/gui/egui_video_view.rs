//! Source code example of how to create your own widget.
//! This is meant to be read as a tutorial, hence the plethora of comments.

use std::sync::Arc;

use egui::{PaintCallback, PaintCallbackInfo, Shape};

/// iOS-style toggle switch:
///
/// ``` text
///      _____________
///     /       /.....\
///    |       |.......|
///     \_______\_____/
/// ```
///
/// ## Example:
/// ``` ignore
/// toggle_ui(ui, &mut my_bool);
/// ```
pub fn toggle_ui(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    // Widget code can be broken up in four steps:
    //  1. Decide a size for the widget
    //  2. Allocate space for it
    //  3. Handle interactions with the widget (if any)
    //  4. Paint the widget

    // 1. Deciding widget size:
    // You can query the `ui` how much space is available,
    // but in this example we have a fixed size widget based on the height of a standard button:
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);

    // 2. Allocating space:
    // This is where we get a region of the screen assigned.
    // We also tell the Ui to sense clicks in the allocated region.
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    // 3. Interact: Time to check for clicks!
    if response.clicked() {
        *on = !*on;
        response.mark_changed(); // report back that the value changed
    }

    // Attach some meta-data to the response which can be used by screen readers:
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    // 4. Paint!
    // Make sure we need to paint:
    if ui.is_rect_visible(rect) {
        // Let's ask for a simple animation from egui.
        // egui keeps track of changes in the boolean associated with the id and
        // returns an animated value in the 0-1 range for how much "on" we are.
        let how_on = ui.ctx().animate_bool(response.id, *on);
        // We will follow the current style by asking
        // "how should something that is being interacted with be painted?".
        // This will, for instance, give us different colors when the widget is hovered or clicked.
        let visuals = ui.style().interact_selectable(&response, *on);
        // All coordinates are in absolute screen coordinates so we use `rect` to place the elements.
        let rect = rect.expand(visuals.expansion);
        let radius = 0.5 * rect.height();
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        // Paint the circle, animating it from left to right with `how_on`:
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.75 * radius, visuals.bg_fill, visuals.fg_stroke);
    }

    // All done! Return the interaction response so the user can check what happened
    // (hovered, clicked, ...) and maybe show a tooltip:
    response
}

// A wrapper that allows the more idiomatic usage pattern: `ui.add(toggle(&mut my_bool))`
/// iOS-style toggle switch.
///
/// ## Example:
/// ``` ignore
/// ui.add(toggle(&mut my_bool));
/// ```
pub fn toggle(on: &mut bool) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| toggle_ui(ui, on)
}

use egui_miniquad::CallbackFn;
use miniquad::*;

/// Takes NV21 video range frames and draws them into a RGBA texture.
pub struct NV21VideoView {
    resources: Arc<(Pipeline, Bindings)>,
    texture_y: Texture,
    texture_uv: Texture,
}

pub trait EguiShape {
    fn ui(&self, ui: &mut egui::Ui) -> Shape;
}

impl EguiShape for NV21VideoView {
    fn ui(&self, ui: &mut egui::Ui) -> Shape {
        let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
        let resources = self.resources.clone();

        Shape::from(PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new(move |info, ctx| {
                Self::draw(ctx, resources.as_ref())
            })),
        })
    }
}

impl NV21VideoView {
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

        let texture_y =
            Texture::from_data_and_format(ctx, &[], texture_params(TextureFormat::Alpha));
        let texture_uv =
            Texture::from_data_and_format(ctx, &[], texture_params(TextureFormat::LuminanceAlpha));

        let shader = offscreen_shader::new(ctx);
        let resources = Arc::new((
            Pipeline::with_params(
                ctx,
                &[vertex_buffer_layout],
                &vertex_attributes,
                shader,
                PipelineParams {
                    depth_test: Comparison::LessOrEqual,
                    depth_write: true,
                    ..Default::default()
                },
            ),
            Bindings {
                vertex_buffers: vec![vertex_buffer],
                index_buffer,
                images: vec![texture_y, texture_uv],
            },
        ));

        Self {
            resources,
            texture_y,
            texture_uv,
        }
    }

    pub fn width(&self) -> u32 {
        self.texture_y.width
    }

    pub fn height(&self) -> u32 {
        self.texture_y.height
    }

    pub fn update(&mut self, ctx: &mut Context, yuv: &[u8], width: u32, height: u32) {
        let (y, uv) = yuv.split_at((width * height) as _);

        if self.width() != width || self.height() != height {
            self.texture_y.resize(ctx, width, height, Some(y));
            self.texture_uv.resize(ctx, width / 2, height / 2, Some(uv));
        } else {
            self.texture_y.update(ctx, y);
            self.texture_uv.update(ctx, uv);
        }
    }

    /// Applies pipeline, bindings and maybe uniforms and does a draw call.
    pub fn draw(ctx: &mut Context, (pipeline, bindings): &(Pipeline, Bindings)) {

        ctx.apply_pipeline(pipeline);
        // ctx.apply_bindings(bindings);
        // ctx.draw(0, 6, 1);
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
