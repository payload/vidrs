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

const DEBUG: bool = true;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if DEBUG {
        init_logging()
    }

    let (exit_tx, exit) = broadcast::channel(1);
    tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));

    let (frames_tx, frames) = mpsc::channel(1);
    let (packets_tx, packets) = mpsc::channel(3);
    let run_camera_task = tokio::spawn(run_camera(exit_tx.clone(), exit.resubscribe(), frames_tx));

    let picture_loss_indicator = Arc::new(AtomicBool::new(false));
    let encode_frames_task = tokio::spawn(encode_frames(
        frames,
        packets_tx,
        picture_loss_indicator.clone(),
    ));

    let (exchange_tx, exchange_rx) = mpsc::channel(1);
    let http_testapp_task =
        tokio::spawn(webrtc::http_testapp(8080, exchange_tx, exit.resubscribe()));
    let webrtc_task = tokio::spawn(webrtc::webrtc_tasks(
        exchange_rx,
        packets,
        picture_loss_indicator.clone(),
    ));

    let _ = tokio::join!(
        run_camera_task,
        encode_frames_task,
        http_testapp_task,
        webrtc_task
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

        let Some(encoder) = encoder.as_mut() else { panic!("no enocder"); };
        let start_time = start_time.get_or_insert_with(|| Instant::now());

        let pts = start_time.elapsed().as_millis() as _;
        let data = frame.pixels().data;

        let frames: Vec<_> = encoder
            .encode(pts, data, picture_loss_indicator.load(Ordering::Relaxed))
            .expect("encoded packets")
            .map(|frame| {
                if frame.key {
                    log::debug!("Encoded key frame: {:?}", format)
                }
                EncodedFrame {
                    bytes: bytes::Bytes::copy_from_slice(frame.data),
                    keyframe: frame.key,
                }
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
    encoder: Option<codec::Encoder>,
    format: &camera::SampleFormat,
) -> Option<codec::Encoder> {
    if let Some(encoder) = encoder {
        if encoder.width == format.width as usize && encoder.height == format.height as usize {
            return Some(encoder);
        }
    }

    let config = codec::Config {
        width: format.width as u32,
        height: format.height as u32,
        codec: codec::VideoCodecId::VP8,
        timebase: [1, 1000],
        bitrate: 5000,
    };
    Some(codec::Encoder::new(config).expect("encoder"))
}

fn init_logging() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Trace)
        .init();
}
