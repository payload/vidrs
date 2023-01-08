#[cfg(target_os = "macos")]
mod avfoundation;
#[cfg(target_os = "macos")]
mod mac_avfoundation;

mod camera;
mod eye;

pub use camera::*;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(eye::Eye));
    #[cfg(target_os = "macos")]
    backends.push(CameraBackend::new(avfoundation::AvFoundation));
    backends
}
