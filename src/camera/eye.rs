use super::*;

use std::sync::Arc;
use std::sync::Mutex;

pub struct Eye;

use ::eye::hal;
use hal::format::PixelFormat;
use hal::traits::Context as EyeContextTrait;
use hal::traits::Device as EyeDeviceTrait;
use hal::traits::Stream as EyeStreamTrait;
use hal::PlatformContext;
use tokio_stream::wrappers::WatchStream;

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
    tx: Arc<Mutex<Option<CameraFrameSender>>>,
    rx: CameraFrameReceiver,
}

impl EyeDevice {
    fn new(device: eye::hal::platform::Device<'static>) -> Self {
        let (tx, rx) = tokio::sync::watch::channel(None);

        Self {
            device: Arc::new(Mutex::new(Some(device))),
            tx: Arc::new(Mutex::new(Some(tx))),
            rx,
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
        let format = PixelFormat::Custom("v024".to_string());
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
        let mut stream = device
            .as_ref()
            .expect("device not stopped")
            .start_stream(&stream_descriptor)
            .unwrap();
        let device = self.device.clone();
        let tx = self.tx.lock().unwrap().take().expect("only start once");
        std::thread::spawn(move || {
            while device.lock().unwrap().is_some() {
                let data = stream
                    .next()
                    .expect("next frame")
                    .expect("next frame, no error");

                let width = stream_descriptor.width;
                let height = stream_descriptor.height;

                let data = if data.len() == (width * height * 3) as usize {
                    // eye openpnp backend produces only rgb24 m(
                    // and since vp8 encoding and video view only support yuv I need to convert it
                    // again
                    // TODO support different color formats, find 420v support for linux and windows
                    rgb2yuv420::convert_rgb_to_yuv420sp_nv12(data, width, height, 3)
                } else {
                    data.to_vec()
                };

                let frame = EyeFrame {
                    width,
                    height,
                    pixel_format: "420v".to_string(),
                    data,
                };

                tx.send(Some(Arc::new(CameraFrame::new(frame))))
                    .expect("send frame");
            }
        });
    }

    fn stop(&self) {
        let mut device = self.device.lock().unwrap();
        *device = None;
    }

    fn frames(&self) -> CameraFrameStream {
        Box::pin(WatchStream::new(self.rx.clone()))
    }
}

impl Stream for eye::hal::stream::Descriptor {
    fn format(&self) -> (u32, u32, String) {
        (self.width, self.height, self.pixfmt.to_string())
    }
}

struct EyeFrame {
    width: u32,
    height: u32,
    pixel_format: String,
    data: Vec<u8>,
}

impl Frame for EyeFrame {
    fn into_arc(self) -> Arc<dyn Frame> {
        Arc::new(self)
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        (self.width, self.height, self.pixel_format.clone())
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}
