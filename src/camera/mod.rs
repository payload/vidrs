mod avfoundation;
mod camera;
mod eye;
mod mac_avfoundation;

pub use camera::*;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(avfoundation::AvFoundation));
    backends.push(CameraBackend::new(eye::Eye));
    backends
}
