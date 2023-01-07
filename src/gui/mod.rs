use crate::camera::ReceiverSharedFrame;

mod stage;
mod video_view;
mod egui_video_view;

pub fn run_gui(camera_frame: ReceiverSharedFrame) {
    miniquad::start(miniquad::conf::Conf::default(), move |ctx| {
        Box::new(stage::Stage::new(ctx, camera_frame))
    });
}
