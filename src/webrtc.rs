use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;

pub use webrtc::api::interceptor_registry::register_default_interceptors;
pub use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_VP8};
pub use webrtc::api::APIBuilder;
pub use webrtc::api::API;
pub use webrtc::ice_transport::ice_server::RTCIceServer;
pub use webrtc::interceptor::registry::Registry;
pub use webrtc::media::Sample;
pub use webrtc::peer_connection::configuration::RTCConfiguration;
pub use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
pub use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
pub use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtcp::header::{PacketType, FORMAT_PLI};
pub use webrtc::rtcp::payload_feedbacks::picture_loss_indication::PictureLossIndication;
pub use webrtc::rtp;
pub use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
pub use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
pub use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
pub use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
pub use webrtc::track::track_local::{TrackLocal, TrackLocalWriter};
pub use webrtc::Error;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::EncodedFrame;

type OfferAnswerExchange = (RTCSessionDescription, mpsc::Sender<RTCSessionDescription>);

pub async fn http_testapp(
    port: u16,
    exchange_tx: mpsc::Sender<OfferAnswerExchange>,
    mut exit: broadcast::Receiver<()>,
) {
    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port)).unwrap();
    let service = make_service_fn(move |_| {
        let exchange_tx = exchange_tx.clone();

        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let exchange_tx = exchange_tx.clone();

                remote_handler(req, exchange_tx)
            }))
        }
    });
    let shutdown = async move {
        let _ = exit.recv().await;
    };

    let server = Server::bind(&addr).serve(service);

    println!("http://localhost:{}", server.local_addr().port());

    if let Err(err) = server.with_graceful_shutdown(shutdown).await {
        eprintln!("Server error: {}", err);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RtcError {
    #[error("No offer received")]
    NoOffer,
    #[error("Cant send answer")]
    AnswerSendError,
    #[error("WebRTC error")]
    WebRtc(#[from] webrtc::Error),
}

pub async fn webrtc_tasks(
    mut exchange_rx: mpsc::Receiver<OfferAnswerExchange>,
    mut encoded_frames_rx: mpsc::Receiver<EncodedFrame>,
    picture_loss_indicator: Arc<AtomicBool>,
) -> webrtc::error::Result<()> {
    let api = create_webrtc_api().expect("webrtc api");
    let config = rtc_configuration();

    let peer_connection = Arc::new(api.new_peer_connection(config).await?);
    let _peer_connection_state_change = PeerConnectionStateChange::new(&peer_connection);
    let output_track = create_vp8_track();
    let output_track_pc = Arc::clone(&output_track);

    // let peer_connection2 = peer_connection.clone();
    let on_setup = (|| async {
        let rtp_sender = peer_connection.add_track(output_track_pc).await?;
        tokio::spawn(process_rtcp(rtp_sender.clone(), picture_loss_indicator));
        let (offer, answer_tx) = exchange_rx.recv().await.ok_or(RtcError::NoOffer)?;
        peer_connection.set_remote_description(offer).await?;
        let answer = peer_connection.create_answer(None).await?;
        let mut gather_complete = peer_connection.gathering_complete_promise().await;
        peer_connection.set_local_description(answer).await?;
        let _ = gather_complete.recv().await; // no trickle ICE
        let answer = peer_connection.local_description().await.unwrap();
        answer_tx
            .send(answer)
            .await
            .map_err(|_| RtcError::AnswerSendError)?;
        Ok::<(), RtcError>(())
    })()
    .await;

    if let Err(err) = on_setup {
        log::debug!("WebRTC setup failed. End. ({})", err);
        peer_connection
            .close()
            .await
            .unwrap_or_else(|err| log::error!("{:?}", err));
        return Ok(());
    }

    let ms33 = Duration::from_millis(33);
    while let Some(frame) = encoded_frames_rx.recv().await {
        let sample = Sample {
            data: frame.bytes,
            duration: ms33,
            ..Default::default()
        };
        if let Err(err) = output_track.write_sample(&sample).await {
            log::warn!("{}", err);
        }
    }

    // TODO kind of tell others that PC is connected and listen to PC is done
    Ok(())
}

const INDEX_HTML: &'static str = include_str!("./index.html");

#[derive(thiserror::Error, Debug)]
pub enum HttpTestappError {
    #[error("HTTP handling error")]
    Hyper(#[from] hyper::Error),
    #[error("Offer SDP parsing error")]
    OfferSdp(serde_json::Error),
    #[error("Answer SDP serializing error")]
    AnswerSdp(serde_json::Error),
}

async fn remote_handler(
    req: Request<Body>,
    exchange_tx: mpsc::Sender<OfferAnswerExchange>,
) -> Result<Response<Body>, HttpTestappError> {
    match (req.method(), req.uri().path()) {
        // A HTTP handler that processes a SessionDescription given to us from the other WebRTC-rs or Pion process
        (&Method::POST, "/sdp") => {
            let sdp_str = match std::str::from_utf8(&hyper::body::to_bytes(req.into_body()).await?)
            {
                Ok(s) => s.to_owned(),
                Err(err) => panic!("sdp from utf8: {}", err),
            };
            let sdp = serde_json::from_str::<RTCSessionDescription>(&sdp_str)
                .map_err(HttpTestappError::OfferSdp)?;

            let (answer_tx, mut answer_rx) = mpsc::channel(1);

            let _ = exchange_tx.send((sdp, answer_tx)).await;
            println!("exchange sent");

            if let Some(answer) = answer_rx.recv().await {
                println!("answer received");
                let answer_str =
                    serde_json::to_string(&answer).map_err(HttpTestappError::AnswerSdp)?;
                let mut response = Response::new(answer_str.into());
                *response.status_mut() = StatusCode::OK;
                println!("answer response");
                Ok(response)
            } else {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                println!("answer error");
                Ok(response)
            }
        }
        (&Method::GET, "/") => {
            let mut response = Response::new(INDEX_HTML.into());
            *response.status_mut() = StatusCode::OK;
            Ok(response)
        }
        (&Method::GET, "/favicon.ico") => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::OK;
            Ok(response)
        }
        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn create_webrtc_api() -> webrtc::error::Result<API> {
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m)?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();
    Ok(api)
}

fn rtc_configuration() -> RTCConfiguration {
    RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[allow(unused)]
struct PeerConnectionStateChange {
    connected: tokio::sync::mpsc::Receiver<()>,
    done: tokio::sync::mpsc::Receiver<()>,
}

impl PeerConnectionStateChange {
    fn new(peer_connection: &RTCPeerConnection) -> Self {
        let (connected_tx, connected) = tokio::sync::mpsc::channel(1);
        let (done_tx, done) = tokio::sync::mpsc::channel(1);
        peer_connection.on_peer_connection_state_change(Box::new(
            move |s: RTCPeerConnectionState| {
                log::debug!("PeerConnectionStateChange: {}", s);
                if s == RTCPeerConnectionState::Connected {
                    let _ = connected_tx.try_send(());
                } else if s == RTCPeerConnectionState::Failed {
                    let _ = done_tx.try_send(());
                }
                Box::pin(async move {})
            },
        ));
        Self { connected, done }
    }
}

fn create_vp8_track() -> Arc<TrackLocalStaticSample> {
    Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_VP8.to_owned(),
            ..Default::default()
        },
        "video".to_owned(),     // id
        "webrtc-rs".to_owned(), // stream_id
    ))
}

/// Need to read rtcp to run the internal logic of webrtc-rs of processing rtcp.
async fn process_rtcp(rtp_sender: Arc<RTCRtpSender>, picture_loss_indicator: Arc<AtomicBool>) {
    while let Ok((packets, _)) = rtp_sender.read_rtcp().await {
        for packet in packets {
            if packet.header().packet_type == PacketType::PayloadSpecificFeedback
                && packet.header().length == FORMAT_PLI as u16
            {
                if picture_loss_indicator.swap(true, Ordering::Relaxed) == false {
                    log::debug!("Picture loss indicator set.");
                }
            }
        }
    }
}
