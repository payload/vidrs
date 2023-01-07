mod camera;
mod mac_avfoundation;

pub use camera::*;

use std::sync::Arc;

use mac_avfoundation as av;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(AvFoundation));
    backends
}

/*****************************************************************************/

struct AvFoundation;

impl Backend for AvFoundation {
    fn all_devices(&self) -> Vec<CameraDevice> {
        if let Ok(device) = av::Camera::default() {
            vec![CameraDevice::new(std::sync::Mutex::new(device))]
        } else {
            Vec::new()
        }
    }
}

impl Device for std::sync::Mutex<av::Camera> {
    fn all_streams(&self) -> Vec<CameraStream> {
        self.lock()
            .unwrap()
            .formats()
            .into_iter()
            .map(CameraStream::new)
            .collect()
    }

    fn get_smallest_nv21_video_stream(&self) -> CameraStream {
        CameraStream::new(
            self.lock()
                .unwrap()
                .formats()
                .into_iter()
                .filter(|f| &f.pixel_format == "420v")
                .max_by_key(|f| f.height)
                .expect("420v format"),
        )
    }

    fn start(&self, stream: &CameraStream) {
        let mut camera = self.lock().unwrap();
        let format = camera
            .formats()
            .into_iter()
            .find(|f| stream.format() == CameraStream::new(f.clone()).format());
        camera.set_preferred_format(format);
        camera.start().expect("camera start");
    }

    fn stop(&self) {
        self.lock().unwrap().stop()
    }

    fn frames(&self) -> CameraFrameStream {
        use tokio_stream::StreamExt;
        let camera = self.lock().unwrap();
        Box::pin(
            camera
                .frames()
                .map(|f| f.map(|f| Arc::new(CameraFrame::new(f)))),
        )
    }
}

impl Stream for av::DeviceFormat {
    fn format(&self) -> (u32, u32, String) {
        (self.width as _, self.height as _, self.pixel_format.clone())
    }
}

impl Frame for Arc<av::Frame> {
    fn into_arc(self) -> Arc<dyn Frame> {
        self
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        self.as_ref().size_and_pixel_format()
    }

    fn data(&self) -> &[u8] {
        self.as_ref().data()
    }
}

impl Frame for av::Frame {
    fn into_arc(self) -> Arc<dyn Frame> {
        Arc::new(self)
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        let format = self.format();
        (format.width as _, format.height as _, format.pixel_format)
    }

    fn data(&self) -> &[u8] {
        self.pixels().data
    }
}

/*****************************************************************************/
