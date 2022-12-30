use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::*,
};
use tokio::sync::{broadcast, mpsc};

mod camera;
mod codec;
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
    let (frames_tx, frames) = mpsc::channel(1);
    let (encoded_frames_tx, encoded_frames) = mpsc::channel(3);
    let picture_loss_indicator = Arc::new(AtomicBool::new(false));
    let (exchange_tx, exchange_rx) = mpsc::channel(1);

    let _ = tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));

    let run_camera_task = tokio::spawn(run_camera(exit_tx.clone(), exit.resubscribe(), frames_tx));

    let encode_frames_task = tokio::spawn(encode_frames(
        frames,
        encoded_frames_tx,
        picture_loss_indicator.clone(),
    ));

    let http_testapp_task =
        tokio::spawn(webrtc::http_testapp(8080, exchange_tx, exit.resubscribe()));

    let webrtc_testapp_task = tokio::spawn(webrtc::webrtc_testapp(
        exchange_rx,
        encoded_frames,
        picture_loss_indicator.clone(),
    ));

    let _ = tokio::join!(
        run_camera_task,
        encode_frames_task,
        http_testapp_task,
        webrtc_testapp_task
    );
    Ok(())
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
    frames_tx: mpsc::Sender<camera::Frame>,
) {
    let mut cam = camera::create_camera();

    cam.start().unwrap();
    let frame = cam.frames().next().unwrap();
    let format = frame.format();
    log::debug!("start_camera: first frame format: {:?}", format);

    while exit.is_empty() {
        let frame = cam.frames().next().unwrap();
        let frame_tx = frames_tx.clone();
        if frame_tx.send(frame).await.is_err() {
            log::debug!("No camera frame receiver. End.");
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
    mut frames: mpsc::Receiver<camera::Frame>,
    packets: mpsc::Sender<EncodedFrame>,
    picture_loss_indicator: Arc<AtomicBool>,
) {
    let mut start_time = None;
    let mut encoder = None;

    while let Some(frame) = frames.recv().await {
        let format = frame.format();
        encoder = reconfigure_encoder(encoder, &format);
        let Some(encoder) = encoder.as_mut() else { panic!("no encoder"); };
        let start_time = start_time.get_or_insert_with(Instant::now);
        let pts = start_time.elapsed().as_millis() as _;
        let data = frame.pixels().data;
        let force_keyframe = picture_loss_indicator.load(Ordering::Relaxed);

        let mut encoded_data = encoder
            .encode(pts, data, force_keyframe)
            .expect("encoded data");

        // Copy each frame so we can asynchronously send them one after the other without risking getting an invalidated buffer.
        // TODO This copy can be skipped when we check that the packets sender is not full.
        // TODO This copy can be skipped when we control the data buffer by using vpx_codec_set_cx_data_buf.
        let frames: Vec<_> = encoded_data
            .frames()
            .inspect(|frame| {
                if frame.keyframe() {
                    log::debug!("Encoded key frame: {:?}", format)
                }
            })
            .map(|frame| EncodedFrame {
                bytes: bytes::Bytes::copy_from_slice(frame.data),
                keyframe: frame.keyframe(),
            })
            .collect();

        for frame in frames {
            if let Err(err) = packets.send(frame).await {
                log::debug!("No encoded frame receiver. End encoding frames. {}", err);
                return;
            }
        }
    }
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
