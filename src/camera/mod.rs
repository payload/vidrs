use std::{any::*, pin::Pin, sync::Arc};

// mod eye;

mod mac_avfoundation;
use futures::stream::BoxStream;
use mac_avfoundation as av;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(AvFoundation));
    // backends.push(CameraBackend::new(Eye));
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

pub type CameraFrameSender = tokio::sync::watch::Sender<Option<Arc<CameraFrame>>>;
pub type CameraFrameReceiver = tokio::sync::watch::Receiver<Option<Arc<CameraFrame>>>;

struct AvFoundation;
struct Eye;

pub trait Backend: Any + Send + Sync {
    fn all_devices(&self) -> Vec<CameraDevice>;
}

// type FrameStream = Arc<Pin<Box<dyn futures::Stream<Item = Option<Arc<CameraFrame>>>>>>;
// type FrameStream = Pin<Box<dyn futures::Stream<Item = SomeFrame> + Unpin + Send>>;
type FrameStream = BoxStream<'static, u32>;
type SomeFrame = Option<Arc<CameraFrame>>;


pub trait Device: Any + Send + Sync {
    fn all_streams(&self) -> Vec<CameraStream>;
    fn get_smallest_nv21_video_stream(&self) -> CameraStream;
    fn start(&mut self, stream: &CameraStream);
    fn stop(&mut self);
    fn frames(&self) -> FrameStream;
}

pub trait Stream: Any + Send + Sync {
    fn as_any(&self) -> &(dyn Any + '_) {
        &self
    }
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

    fn start(&mut self, stream: &CameraStream) {
        self.inner.start(stream)
    }

    fn stop(&mut self) {
        self.inner.stop()
    }

    fn frames(&self) -> FrameStream {
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

impl Stream for CameraStream {}

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
            vec![CameraDevice::new(device)]
        } else {
            Vec::new()
        }
    }
}

impl Device for av::Camera {
    fn all_streams(&self) -> Vec<CameraStream> {
        self.formats().into_iter().map(CameraStream::new).collect()
    }

    fn get_smallest_nv21_video_stream(&self) -> CameraStream {
        CameraStream::new(
            self.formats()
                .into_iter()
                .filter(|f| &f.pixel_format == "420v")
                .max_by_key(|f| f.height)
                .expect("420v format"),
        )
    }

    fn start(&mut self, stream: &CameraStream) {
        let format = stream.inner.as_any().downcast_ref::<av::DeviceFormat>();
        let format = Some(format.expect("is AVFoundation type").clone());
        self.set_preferred_format(format);
        self.start();
    }

    fn stop(&mut self) {
        self.stop()
    }

    fn frames(&self) -> FrameStream {
        use tokio_stream::StreamExt;
        Box::pin(
            self.frames()
                .map(|f| f.map(|f| Arc::new(CameraFrame::new(f)))),
        )
        // todo!()
    }
}

impl Stream for av::DeviceFormat {}

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
