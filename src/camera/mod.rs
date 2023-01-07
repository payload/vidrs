use std::{any::Any, sync::Arc};

// mod eye;

mod mac_avfoundation;
use futures::stream::BoxStream;
use mac_avfoundation as av;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(AvFoundation));
    backends
}

pub struct CameraBackend {
    inner: Arc<dyn Backend>,
}

pub struct CameraDevice {
    inner: Arc<dyn Device>,
}

pub struct CameraStream {
    inner: Arc<dyn Stream>,
}

pub struct CameraFrame {
    inner: Arc<dyn Frame>,
}

pub type CameraFrameSender = tokio::sync::watch::Sender<CameraFrameOption>;
pub type CameraFrameReceiver = tokio::sync::watch::Receiver<CameraFrameOption>;
pub type CameraFrameStream = BoxStream<'static, CameraFrameOption>;
pub type CameraFrameOption = Option<Arc<CameraFrame>>;

struct AvFoundation;

pub trait Backend: Any + Send + Sync {
    fn all_devices(&self) -> Vec<CameraDevice>;
}

pub trait Device: Any + Send + Sync {
    fn all_streams(&self) -> Vec<CameraStream>;
    fn get_smallest_nv21_video_stream(&self) -> CameraStream;
    fn start(&self, stream: &CameraStream);
    fn stop(&self);
    fn frames(&self) -> CameraFrameStream;
}

pub trait Stream: Any + Send + Sync {
    fn format(&self) -> (u32, u32, String);
}

pub trait Frame: Any + Send + Sync {
    fn into_arc(self) -> Arc<dyn Frame>;
    fn size_and_pixel_format(&self) -> (u32, u32, String);
    fn data(&self) -> &[u8];
}

/*****************************************************************************/

impl CameraBackend {
    fn new(backend: impl Backend) -> Self {
        Self {
            inner: Arc::new(backend),
        }
    }
}

impl Backend for CameraBackend {
    fn all_devices(&self) -> Vec<CameraDevice> {
        self.inner.all_devices()
    }
}

impl CameraDevice {
    fn new(device: impl Device) -> Self {
        Self {
            inner: Arc::new(device),
        }
    }
}

impl Device for CameraDevice {
    fn all_streams(&self) -> Vec<CameraStream> {
        self.inner.all_streams()
    }

    fn get_smallest_nv21_video_stream(&self) -> CameraStream {
        self.inner.get_smallest_nv21_video_stream()
    }

    fn start(&self, stream: &CameraStream) {
        self.inner.start(stream)
    }

    fn stop(&self) {
        self.inner.stop()
    }

    fn frames(&self) -> CameraFrameStream {
        self.inner.frames()
    }
}

impl CameraStream {
    fn new(stream: impl Stream) -> Self {
        Self {
            inner: Arc::new(stream),
        }
    }
}

impl Stream for CameraStream {
    fn format(&self) -> (u32, u32, String) {
        self.inner.format()
    }
}

impl CameraFrame {
    fn new(frame: impl Frame) -> Self {
        Self {
            inner: frame.into_arc(),
        }
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        self.inner.size_and_pixel_format()
    }

    fn data(&self) -> &[u8] {
        self.inner.data()
    }
}

impl Frame for CameraFrame {
    fn into_arc(self) -> Arc<dyn Frame> {
        Arc::new(self)
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        self.size_and_pixel_format()
    }

    fn data(&self) -> &[u8] {
        self.data()
    }
}

/*****************************************************************************/

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
        // todo!()
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
