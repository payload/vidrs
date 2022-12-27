use tokio::sync::broadcast;

mod camera;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (exit_tx, mut exit) = broadcast::channel::<()>(1);
    tokio::spawn(exit_on_ctrl_c(exit_tx.clone()));
    // tokio::spawn(exit_on_camera_end(exit_tx.clone()));

    tokio::spawn(run_camera(exit_tx.clone()));

    let _ = exit.recv().await;
    Ok(())
}

async fn exit_on_ctrl_c(exit_tx: broadcast::Sender<()>) {
    tokio::signal::ctrl_c().await.expect("ctrl_c");
    let _ = exit_tx.send(());
}

async fn run_camera(exit_tx: broadcast::Sender<()>) {
    let mut cam = camera::create_camera();
    camera::start_camera(&mut cam);

    let _ = exit_tx.send(());
}
