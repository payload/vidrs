use std::time::*;
use tokio::sync::broadcast;

mod camera;

const DEBUG: bool = true;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if DEBUG {
        init_logging()
    }

    let (exit_tx, exit) = broadcast::channel::<()>(1);
    tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));

    let (_,) = tokio::join! {
        tokio::spawn(run_camera(
        exit_tx.clone(),
        exit.resubscribe()
    ))};

    println!("EXIT");
    Ok(())
}

async fn exit_on_ctrl_c(exit_tx: broadcast::Sender<()>) {
    tokio::signal::ctrl_c().await.expect("ctrl_c");
    println!("\nCtrl-C");
    let _ = exit_tx.send(());
}

async fn run_camera(exit_tx: broadcast::Sender<()>, exit: broadcast::Receiver<()>) {
    let mut cam = camera::create_camera();

    cam.start().unwrap();
    let frame = cam.frames().next().unwrap();
    let format = frame.format();
    log::debug!("start_camera: first frame format: {:?}", format);

    let start_time = Instant::now();
    while exit.is_empty() && start_time.elapsed() < Duration::from_secs(1) {
        let frame = cam.frames().next().unwrap();
        log::debug!("{} {:?}", start_time.elapsed().as_millis(), frame.format());
    }

    cam.stop();

    let _ = exit_tx.send(());
}

fn init_logging() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Trace)
        .init();
}
