#[cfg(target_os = "macos")]
mod avfoundation;
#[cfg(target_os = "macos")]
mod mac_avfoundation;

mod camera;

#[cfg(feature = "eye")]
mod eye;

#[cfg(feature = "nokhwa")]
mod nokhwa;

pub use camera::*;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    #[cfg(feature = "eye")]
    backends.push(CameraBackend::new(eye::Eye));
    #[cfg(target_os = "macos")]
    backends.push(CameraBackend::new(avfoundation::AvFoundation));
    #[cfg(feature = "nokhwa")]
    backends.push(CameraBackend::new(nokhwa::Nokhwa));
    backends
}
