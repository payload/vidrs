mod camera;
mod mac_avfoundation;

pub use camera::*;
use eye::hal::format::PixelFormat;

use std::sync::Arc;
use std::sync::Mutex;

use mac_avfoundation as av;

pub fn all_backends() -> Vec<CameraBackend> {
    let mut backends = Vec::new();
    backends.push(CameraBackend::new(AvFoundation));
    backends.push(CameraBackend::new(Eye));
    backends
}

/*****************************************************************************/

struct AvFoundation;

impl Backend for AvFoundation {
    fn all_devices(&self) -> Vec<CameraDeviceDescriptor> {
        vec![CameraDeviceDescriptor::Default]
    }

    fn open_device(&self, desc: CameraDeviceDescriptor) -> CameraDevice {
        match desc {
            CameraDeviceDescriptor::Default => {
                CameraDevice::new(std::sync::Mutex::new(av::Camera::default().unwrap()))
            }
            CameraDeviceDescriptor::Name(_) => todo!(),
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
                .min_by_key(|f| f.height)
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

struct Eye;

use eye::hal::traits::Context as EyeContextTrait;
use eye::hal::traits::Device as EyeDeviceTrait;
use eye::hal::PlatformContext;

impl Eye {
    fn ctx() -> PlatformContext<'static> {
        PlatformContext::all().next().unwrap()
    }
}

impl Backend for Eye {
    fn all_devices(&self) -> Vec<CameraDeviceDescriptor> {
        Self::ctx()
            .devices()
            .unwrap()
            .into_iter()
            .map(|d| CameraDeviceDescriptor::Name(d.uri))
            .collect()
    }

    fn open_device(&self, desc: CameraDeviceDescriptor) -> CameraDevice {
        match desc {
            CameraDeviceDescriptor::Default => todo!(),
            CameraDeviceDescriptor::Name(uri) => {
                CameraDevice::new(EyeDevice::new(Self::ctx().open_device(&uri).unwrap()))
            }
        }
    }
}

struct EyeDevice {
    device: Arc<Mutex<Option<eye::hal::platform::Device<'static>>>>,
}

impl EyeDevice {
    fn new(device: eye::hal::platform::Device<'static>) -> Self {
        Self {
            device: Arc::new(Mutex::new(Some(device))),
        }
    }
}

impl Device for EyeDevice {
    fn all_streams(&self) -> Vec<CameraStream> {
        let device = self.device.lock().unwrap();
        device
            .as_ref()
            .expect("device not stopped")
            .streams()
            .unwrap()
            .into_iter()
            .map(|d| CameraStream::new(d))
            .collect()
    }

    fn get_smallest_nv21_video_stream(&self) -> CameraStream {
        let device = self.device.lock().unwrap();
        let format =  PixelFormat::Custom("v024".to_string());
        device
            .as_ref()
            .expect("device not stopped")
            .streams()
            .unwrap()
            .into_iter()
            .filter(|d| &d.pixfmt == &format)
            .min_by_key(|d| d.height)
            .map(|d| CameraStream::new(d))
            .expect("420v stream found")
    }

    fn start(&self, stream: &CameraStream) {
        let device = self.device.lock().unwrap();
        let stream_descriptor = device
            .as_ref()
            .expect("device not stopped")
            .streams()
            .unwrap()
            .into_iter()
            .find(|d| stream.format() == CameraStream::new(d.clone()).format())
            .unwrap();
        device
            .as_ref()
            .expect("device not stopped")
            .start_stream(&stream_descriptor)
            .unwrap();
    }

    fn stop(&self) {
        let mut device = self.device.lock().unwrap();
        *device = None;
    }

    fn frames(&self) -> CameraFrameStream {
        todo!()
    }
}

impl Stream for eye::hal::stream::Descriptor {
    fn format(&self) -> (u32, u32, String) {
        (self.width, self.height, self.pixfmt.to_string())
    }
}
