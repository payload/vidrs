use std::time::*;
use tokio::sync::{broadcast, mpsc};

mod camera;
mod codec;
mod webrtc;

const DEBUG: bool = false;

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
    let encode_frames_task = tokio::spawn(encode_frames(frames, packets_tx));

    let (exchange_tx, exchange_rx) = mpsc::channel(1);
    let http_testapp_task =
        tokio::spawn(webrtc::http_testapp(8080, exchange_tx, exit.resubscribe()));
    let webrtc_task = tokio::spawn(webrtc::webrtc_tasks(
        exchange_rx,
        packets,
        exit.resubscribe(),
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
    tokio::signal::ctrl_c().await.expect("ctrl_c");
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

    let start_time = Instant::now();
    while exit.is_empty() {
        let frame = cam.frames().next().unwrap();
        log::debug!("{} {:?}", start_time.elapsed().as_millis(), frame.format());

        let frame_tx = frames_tx.clone();
        tokio::spawn(async move {
            frame_tx.send(frame).await.unwrap();
        });
    }

    cam.stop();

    let _ = exit_tx.send(());
}

pub struct EncodedFrame {
    pub bytes: bytes::Bytes,
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
) {
    let mut start_time = None;
    let mut encoder = None;

    while let Some(frame) = frames.recv().await {
        let format = frame.format();
        encoder = reconfigure_encoder(encoder, format);

        let Some(encoder) = encoder.as_mut() else { panic!("no enocder"); };
        let start_time = start_time.get_or_insert_with(|| Instant::now());

        let pts = start_time.elapsed().as_millis() as _;
        let data = frame.pixels().data;
        for frame in encoder.encode(pts, data, false).expect("encoded packets") {
            log::debug!(
                "{} {} Bytes {}",
                frame.pts,
                frame.data.len(),
                ["", "KEY"][frame.key as usize]
            );
            let bytes = bytes::Bytes::copy_from_slice(frame.data);
            let packets = packets.clone();
            tokio::spawn(async move {
                packets
                    .send(EncodedFrame { bytes })
                    .await
                    .expect("send encoded packet")
            });
        }
    }
}

fn reconfigure_encoder(
    encoder: Option<codec::Encoder>,
    format: camera::SampleFormat,
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
