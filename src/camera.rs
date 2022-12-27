mod mac_avfoundation;
use mac_avfoundation::Camera;

pub use mac_avfoundation::{Frame, SampleFormat};

pub fn create_camera() -> Camera {
    let mut camera = Camera::default().unwrap();
    let mut format = camera.formats().first().cloned().unwrap();
    format.pixel_format = "f420".to_string();
    camera.set_preferred_format(Some(format));
    camera
}

pub fn start_camera(camera: &mut Camera) {
    camera.start().unwrap();
    // self.start_time = Some(Instant::now());
    let frame = camera.frames().next().unwrap();
    let format = frame.format();
    log::debug!("start_camera: first frame format: {:?}", format);
}

pub fn stop_camera(camera: &mut Camera) {
    camera.stop();
}
