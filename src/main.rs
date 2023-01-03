use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::*,
};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_stream::{wrappers::WatchStream, StreamExt};

mod camera;
mod codec;
mod gui;
mod webrtc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging();

    /*

    Several tasks get spawned communicating with each other through channels.
    * run_camera_task gets frames from the default camera with some 4:2:0 pixel format
    * encode_frames_task throws frames into libvpx VP8 encoder and get `EncodedFrame`s out
    * http_testapp_task is a HTTP server serving an index.html testapp on usuall http://localhost:8080
    * http_testapp_task also provides a SDP offer answer exchange endpoint, for a single exchange though
    * the SDP offer exchange request goes into the webrtc_testapp_task which eventually produces an SDP answer as a response
    * webrtc_testapp_task is setting up a peer connection, an output track and takes additionally encoded frames and writes them on the output track

    On pressing Ctrl-C the camera stops.
    When the camera task ends, the corresponding channel gets closed to, which will close the encode frames task.
    Because every channel closes when the task ends, this closing and ending eventually propagetes through all tasks.
    Every task can so deal with closing and shutting down.
     */

    let (exit_tx, exit) = broadcast::channel(1);

    let (camera_frame_tx, camera_frame) = watch::channel(None);

    let (encoded_frames_tx, encoded_frames) = mpsc::channel(3);
    let picture_loss_indicator = Arc::new(AtomicBool::new(false));
    let (exchange_tx, exchange_rx) = mpsc::channel(1);

    let _ = tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));

    let run_camera_task = tokio::spawn(run_camera(
        exit_tx.clone(),
        exit.resubscribe(),
        camera_frame_tx,
    ));

    let encode_frames_task = tokio::spawn(encode_frames(
        camera_frame.clone(),
        encoded_frames_tx,
        picture_loss_indicator.clone(),
    ));

    tokio::spawn(write_frame(camera_frame.clone()));

    let http_testapp_task =
        tokio::spawn(webrtc::http_testapp(8080, exchange_tx, exit.resubscribe()));

    let webrtc_testapp_task = tokio::spawn(webrtc::webrtc_testapp(
        exchange_rx,
        encoded_frames,
        picture_loss_indicator.clone(),
    ));

    // let gui_thread = std::thread::spawn(|| {
    gui::run_gui(camera_frame.clone());
    // });

    let _ = tokio::join!(
        run_camera_task,
        encode_frames_task,
        http_testapp_task,
        webrtc_testapp_task
    );

    // gui_thread.join().expect("gui thread");
    Ok(())
}

async fn write_frame(mut frame: camera::ReceiverSharedFrame) {
    for _ in 0..10 {
        let _ = frame.changed().await;
    }

    let (path, data) = {
        let frame_borrow = frame.borrow();
        let Some(frame) = frame_borrow.as_ref() else { return };

        let format = frame.format();
        let path = format!(
            "camera_frame.{}.{}.{}",
            format.pixel_format.as_str(),
            format.width,
            format.height
        );
        let data = frame.pixels().data.to_vec();
        (path, data)
    };

    let _ = tokio::fs::write(path, &data).await;
}

async fn exit_on_ctrl_c(exit_tx: broadcast::Sender<()>) {
    if let Err(err) = tokio::signal::ctrl_c().await {
        log::debug!("Ctrl-C signal handler broke. Exit. ({})", err);
    }
    // ignore err, because this means everybody who can exit, exitted already.
    let _ = exit_tx.send(());
}

async fn run_camera(
    exit_tx: broadcast::Sender<()>,
    exit: broadcast::Receiver<()>,
    frames_tx: camera::SenderSharedFrame,
) {
    let mut cam = camera::Camera::default().unwrap();
    // searching for the biggest compatible NV21 video range format, on Mac this is 420v
    let format = cam
        .formats()
        .into_iter()
        .filter(|f| &f.pixel_format == "420v")
        .max_by_key(|f| f.height)
        .expect("420v format");
    cam.set_preferred_format(Some(format));

    cam.start().unwrap();
    let mut frames = cam.frames();
    let mut first_frame = true;

    loop {
        if !exit.is_empty() {
            break;
        }

        if let Some(frame) = frames.next().await {
            let Some(frame) = frame else { continue };

            if first_frame {
                first_frame = false;
                log::debug!(
                    "run_camera: Started receiving camera frames. {:?}",
                    frame.format()
                );
            }

            match frames_tx.send(Some(frame)) {
                Ok(_) => log::trace!("run_camera: send frame"),
                Err(_) => {
                    log::debug!("run_camera: No camera frame receiver. End.");
                    break;
                }
            }
        } else {
            log::debug!("run_camera: Camera frames ended. End.");
            break;
        }
    }

    cam.stop();

    exit_tx.send(()).expect("exit");
}

pub struct EncodedFrame {
    pub bytes: bytes::Bytes,
    pub keyframe: bool,
}

impl std::fmt::Debug for EncodedFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncodedFrame")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

async fn encode_frames(
    frame: camera::ReceiverSharedFrame,
    packets: mpsc::Sender<EncodedFrame>,
    picture_loss_indicator: Arc<AtomicBool>,
) {
    let mut start_time = None;
    let mut encoder = None;
    let mut frames = WatchStream::new(frame);

    while let Some(frame) = frames.next().await {
        let Some(frame) = frame else { continue };
        log::trace!("encode_frames: recv frame");

        let format = frame.format();
        encoder = reconfigure_encoder(encoder, &format);
        let Some(encoder) = encoder.as_mut() else { panic!("no encoder"); };
        let start_time = start_time.get_or_insert_with(Instant::now);
        let pts = start_time.elapsed().as_millis() as _;
        let force_keyframe = picture_loss_indicator.load(Ordering::Relaxed);

        let mut encoded_data = {
            let image = encoder
                .wrap_image(frame.pixels().data, codec::ImageFormat::NV12)
                .expect("wrap image");
            encoder
                .encode(pts, image, force_keyframe)
                .expect("encoded data")
        };

        // Copy each frame so we can asynchronously send them one after the other without risking getting an invalidated buffer.
        // TODO This copy can be skipped when we check that the packets sender is not full.
        // TODO This copy can be skipped when we control the data buffer by using vpx_codec_set_cx_data_buf.
        let frames: Vec<_> = encoded_data
            .frames()
            .inspect(|frame| {
                if frame.keyframe() {
                    log::debug!("encode_frames: Encoded key frame: {:?}", format)
                }
            })
            .map(|frame| EncodedFrame {
                bytes: bytes::Bytes::copy_from_slice(frame.data),
                keyframe: frame.keyframe(),
            })
            .collect();

        for frame in frames {
            log::trace!("encode_frames: sending frame");
            match packets.send(frame).await {
                Ok(_) => log::trace!("encode_frames: sent frame"),
                Err(err) => {
                    log::debug!(
                        "encode_frames: No encoded frame receiver. End encoding frames. {}",
                        err
                    );
                    return;
                }
            }
        }
    }

    log::debug!("encode_frames: End.");
}

fn reconfigure_encoder(
    encoder: Option<codec::Vp8Encoder>,
    format: &camera::SampleFormat,
) -> Option<codec::Vp8Encoder> {
    let config = codec::Vp8Config::new(format.width as u32, format.height as u32, [1, 1000], 5000)
        .expect("config");

    if let Some(encoder) = encoder {
        if encoder.config() == &config {
            return Some(encoder);
        }
    }

    Some(codec::Vp8Encoder::new(&config).expect("encoder"))
}

fn init_logging() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Error)
        .parse_default_env()
        .init();
}
