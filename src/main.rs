use std::time::*;
use tokio::sync::{broadcast, mpsc};

mod camera;
mod codec;

const DEBUG: bool = true;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if DEBUG {
        init_logging()
    }

    let (exit_tx, exit) = broadcast::channel(1);
    tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));

    let (frames_tx, frames) = mpsc::channel(1);
    let run_camera_task = tokio::spawn(run_camera(exit_tx.clone(), exit.resubscribe(), frames_tx));
    let encode_frames_task = tokio::spawn(encode_frames(frames));

    let _ = tokio::join!(run_camera_task, encode_frames_task);
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

async fn encode_frames(mut frames: mpsc::Receiver<camera::Frame>) {
    let mut start_time = None;
    let mut encoder = None;

    while let Some(frame) = frames.recv().await {
        let format = frame.format();
        encoder = reconfigure_encoder(encoder, format);

        let Some(encoder) = encoder.as_mut() else { panic!("no enocder"); };
        let start_time = start_time.get_or_insert_with(|| Instant::now());

        let pts = start_time.elapsed().as_millis() as _;
        let data = frame.pixels().data;
        for packet in encoder.encode(pts, data, false).expect("encoded packets") {
            log::debug!(
                "{} {} Bytes {}",
                packet.pts,
                packet.data.len(),
                ["", "KEY"][packet.key as usize]
            );
        }
    }
}

fn reconfigure_encoder(
    encoder: Option<codec::Encoder>,
    format: camera::SampleFormat,
) -> Option<codec::Encoder> {
    if let Some(encoder) = encoder {
        if encoder.width == format.width as _ && encoder.height == format.height as _ {
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
