use egui_miniquad::EguiMq;
use miniquad::*;

use super::video_view::VideoView;
use crate::camera::CameraFrameReceiver;

pub(crate) struct Stage {
    egui_mq: EguiMq,
    video_view: VideoView,
    camera_frame: CameraFrameReceiver,
}

impl Stage {
    pub(crate) fn new(ctx: &mut Context, camera_frame: CameraFrameReceiver) -> Self {
        Self {
            egui_mq: EguiMq::new(ctx),
            video_view: VideoView::new(ctx),
            camera_frame,
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self, ctx: &mut Context) {
        if let Ok(true) = self.camera_frame.has_changed() {
            if let Some(frame) = &*self.camera_frame.borrow() {
                let (width, height, _) = frame.size_and_pixel_format();
                self.video_view.update(ctx, frame.data(), width, height);
            }
        }
    }

    fn draw(&mut self, ctx: &mut Context) {
        ctx.begin_default_pass(PassAction::clear_color(0.0, 0.0, 0.0, 1.0));
        ctx.end_render_pass();

        let video_texture_id = self.video_view.draw(ctx).gl_internal_id() as _;
        let video_texture_id = egui::TextureId::User(video_texture_id);

        self.egui_mq.run(ctx, |_mq_ctx, egui_ctx| {
            egui::Window::new("vidrs").show(egui_ctx, |ui| {
                let width = self.video_view.width() / 2;
                let height = self.video_view.height() / 2;
                ui.image(video_texture_id, egui::Vec2::new(width as _, height as _));

                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("Quit").clicked() {
                    // TODO tell the other that we are exitting
                    std::process::exit(0);
                }
            });
        });

        self.egui_mq.draw(ctx);

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
