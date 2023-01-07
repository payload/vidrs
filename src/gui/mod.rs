use crate::camera::CameraFrameReceiver;

mod stage;
mod video_view;

pub fn run_gui(camera_frame: CameraFrameReceiver) {
    miniquad::start(miniquad::conf::Conf::default(), move |ctx| {
        Box::new(stage::Stage::new(ctx, camera_frame))
    });
}
