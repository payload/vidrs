use futures::stream::BoxStream;
use std::any::Any;
use std::sync::Arc;

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
    pub(crate) fn new(backend: impl Backend) -> Self {
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
    pub(crate) fn new(device: impl Device) -> Self {
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
    pub(crate) fn new(stream: impl Stream) -> Self {
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
    pub(crate) fn new(frame: impl Frame) -> Self {
        Self {
            inner: frame.into_arc(),
        }
    }

    pub(crate) fn size_and_pixel_format(&self) -> (u32, u32, String) {
        self.inner.size_and_pixel_format()
    }

    pub(crate) fn data(&self) -> &[u8] {
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
