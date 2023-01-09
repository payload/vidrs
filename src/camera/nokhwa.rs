use super::camera::*;
use ::nokhwa::*;
use tokio_stream::wrappers::WatchStream;

pub struct Nokhwa;

impl Backend for Nokhwa {
    fn all_devices(&self) -> Vec<CameraDeviceDescriptor> {
        vec![CameraDeviceDescriptor::Default]
    }

    fn open_device(&self, desc: CameraDeviceDescriptor) -> CameraDevice {
        dbg!(nokhwa_check());
        dbg!(nokhwa_initialize(|permission| {
            dbg!(permission);
        }));
        let backend = nokhwa::native_api_backend().unwrap();
        let infos = nokhwa::query(backend).unwrap();
        dbg!(infos);
        use ::nokhwa::pixel_format::*;
        use ::nokhwa::utils::*;
        let index = CameraIndex::Index(0);
        let format = CameraFormat::new_from(1280, 720, FrameFormat::NV12, 30);
        let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(format));
        let camera = Camera::new(index, requested).unwrap();

        match desc {
            CameraDeviceDescriptor::Default => CameraDevice::new(NokhwaCamera::new(camera)),
            CameraDeviceDescriptor::Name(_) => todo!(),
        }
    }
}

use std::sync::Arc;
use std::sync::Mutex;

struct NokhwaCamera {
    inner: Arc<Mutex<Option<nokhwa::Camera>>>,
    tx: Arc<Mutex<Option<CameraFrameSender>>>,
    rx: CameraFrameReceiver,
}

impl NokhwaCamera {
    fn new(camera: nokhwa::Camera) -> Self {
        let (tx, rx) = tokio::sync::watch::channel(None);
        Self {
            inner: Arc::new(Mutex::new(Some(camera))),
            tx: Arc::new(Mutex::new(Some(tx))),
            rx,
        }
    }
}

// TODO those traits dont need to be Send and Sync, but well they are for now
unsafe impl Send for NokhwaCamera {}
unsafe impl Sync for NokhwaCamera {}

impl Device for NokhwaCamera {
    fn all_streams(&self) -> Vec<CameraStream> {
        todo!()
    }

    fn get_smallest_nv21_video_stream(&self) -> CameraStream {
        let camera_guard = self.inner.lock().unwrap();
        let camera = camera_guard.as_ref().unwrap();
        let format = camera.camera_format();
        let width = format.width();
        let height = format.height();
        use utils::FrameFormat::*;
        let format = match format.format() {
            f @ YUYV => format!("{}", f),
            f @ NV12 => format!("{}", f),
            MJPEG => todo!(),
            GRAY => todo!(),
            RAWRGB => "rgb".to_string(),
        };
        CameraStream::new(StreamFormat(width, height, format))
    }

    fn start(&self, _stream: &CameraStream) {
        let mut camera_guard = self.inner.lock().unwrap();
        let camera = camera_guard.as_mut().unwrap();
        camera.open_stream().unwrap();
        dbg!(camera.compatible_fourcc().unwrap());

        let tx = self.tx.lock().unwrap().take().expect("only start once");
        let camera = unsafe { std::mem::transmute::<_, Arc<()>>(self.inner.clone()) };
        std::thread::spawn(move || {
            let camera = unsafe { std::mem::transmute::<_, Arc<Mutex<Option<Camera>>>>(camera) };
            while let Some(camera) = camera.lock().unwrap().as_mut() {
                let buffer = camera.frame().unwrap();

                let width = buffer.resolution().width();
                let height = buffer.resolution().height();
                let data = buffer.buffer();

                let data = if data.len() == (width * height * 3) as usize {
                    rgb2yuv420::convert_rgb_to_yuv420sp_nv12(data, width, height, 3)
                } else {
                    data.to_vec()
                };

                let format = "420v".to_string();
                let frame = NFrame {
                    width,
                    height,
                    format,
                    data,
                };

                tx.send(Some(Arc::new(CameraFrame::new(frame)))).unwrap();
            }
        });
    }

    fn stop(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.as_mut().unwrap().stop_stream().unwrap();
    }

    fn frames(&self) -> CameraFrameStream {
        Box::pin(WatchStream::new(self.rx.clone()))
    }
}

struct StreamFormat(u32, u32, String);

impl Stream for StreamFormat {
    fn format(&self) -> (u32, u32, String) {
        (self.0, self.1, self.2.clone())
    }
}

struct NFrame {
    width: u32,
    height: u32,
    data: Vec<u8>,
    format: String,
}

impl Frame for NFrame {
    fn into_arc(self) -> Arc<dyn Frame> {
        Arc::new(self)
    }

    fn size_and_pixel_format(&self) -> (u32, u32, String) {
        (self.width, self.height, self.format.clone())
    }

    fn data(&self) -> &[u8] {
        &self.data
    }
}
