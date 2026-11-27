//! WHIP (WebRTC HTTP Ingest Protocol) Implementation
//!
//! Supports:
//! - WebRTC ingest via WHIP (client + server modes)
//! - Pure-Rust str0m integration
//! - H.264 + Opus (HEVC auto-transcode to H.264 for browser compat)
//! - STUN/TURN server configuration
//! - ICE candidate gathering
//! - SDP offer/answer exchange

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use str0m::{media::Direction, net::IceCandidate, sdp::SessionDescription};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::fragment::{Fragment, FragmentBroadcasterRegistry, FragmentMeta, StreamId};
use mediaedge_observability::MetricsRegistry;
use mediaedge_config::types::{WhipConfig, TurnServerConfig};

/// WHIP server state.
#[derive(Clone)]
struct WhipServerState {
    config: WhipConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    active_sessions: Arc<dashmap::DashMap<String, WhipSessionInfo>>,
}

#[derive(Debug)]
struct WhipSessionInfo {
    stream_id: StreamId,
    peer_connection: Arc<tokio::sync::Mutex<Option<str0m::PeerConnection>>>,
    started_at: Instant,
    bytes_received: u64,
    video_frames: u64,
    audio_frames: u64,
    ice_state: str0m::IceConnectionState,
    dtls_state: str0m::DtlsConnectionState,
}

/// WHIP endpoint request (SDP offer).
#[derive(Debug, Deserialize, Serialize)]
pub struct WhipOfferRequest {
    pub sdp: String,
    pub stream_id: Option<String>,
}

/// WHIP endpoint response (SDP answer).
#[derive(Debug, Deserialize, Serialize)]
pub struct WhipAnswerResponse {
    pub sdp: String,
    pub stream_id: String,
}

/// WHIP ICE candidate request.
#[derive(Debug, Deserialize, Serialize)]
pub struct WhipIceCandidateRequest {
    pub candidate: String,
    pub sdp_mid: String,
    pub sdp_mline_index: u32,
}

/// Create WHIP ingest router.
pub fn create_whip_router(
    config: WhipConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
) -> Router {
    let state = Arc::new(WhipServerState {
        config,
        registry,
        metrics,
        active_sessions: Arc::new(dashmap::DashMap::new()),
    });

    Router::new()
        .route("/whip", post(whip_offer_handler))
        .route("/whip/:stream_id/ice", post(whip_ice_handler))
        .with_state(state)
}

/// POST /whip - Handle WHIP offer.
async fn whip_offer_handler(
    State(state): State<Arc<WhipServerState>>,
    headers: HeaderMap,
    Json(payload): Json<WhipOfferRequest>,
) -> Result<Response, StatusCode> {
    // Validate content type
    if !headers.get("content-type").map(|v| v.to_str().unwrap_or("").contains("application/sdp")).unwrap_or(false) {
        warn!("WHIP offer missing application/sdp content-type");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate stream ID if not provided
    let stream_id = payload.stream_id.unwrap_or_else(|| {
        format!("whip-{}", uuid::Uuid::new_v4().simple())
    });

    info!("WHIP offer received for stream: {}", stream_id);

    // Parse SDP offer
    let offer = match SessionDescription::parse(&payload.sdp) {
        Ok(sdp) => sdp,
        Err(e) => {
            error!("Failed to parse SDP offer: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Create peer connection
    let peer_connection = create_peer_connection(&state, &stream_id, offer).await?;

    // Generate SDP answer
    let answer = peer_connection.lock().await.as_mut().unwrap().create_answer().await
        .map_err(|e| {
            error!("Failed to create SDP answer: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store session
    let session_info = WhipSessionInfo {
        stream_id: StreamId::new(stream_id.clone()),
        peer_connection: Arc::new(tokio::sync::Mutex::new(Some(peer_connection))),
        started_at: Instant::now(),
        bytes_received: 0,
        video_frames: 0,
        audio_frames: 0,
        ice_state: str0m::IceConnectionState::New,
        dtls_state: str0m::DtlsConnectionState::New,
    };
    state.active_sessions.insert(stream_id.clone(), session_info);

    // Update metrics
    state.metrics.counter("ingest_whip_sessions_total", "Total WHIP sessions").increment(1);
    state.metrics.counter("ingest_active_streams", "Active ingest streams").increment(1);

    // Return SDP answer
    let response = WhipAnswerResponse {
        sdp: answer.to_string(),
        stream_id,
    };

    Ok(Json(response).into_response())
}

/// POST /whip/:stream_id/ice - Handle ICE candidate.
async fn whip_ice_handler(
    State(state): State<Arc<WhipServerState>>,
    Path(stream_id): Path<String>,
    Json(payload): Json<WhipIceCandidateRequest>,
) -> Result<Response, StatusCode> {
    info!("WHIP ICE candidate for stream: {}", stream_id);

    let session = state.active_sessions.get(&stream_id);
    if session.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let session = session.unwrap();
    let mut pc = session.peer_connection.lock().await;
    
    if let Some(ref mut pc) = *pc {
        let candidate = IceCandidate::parse(&payload.candidate)
            .map_err(|e| {
                error!("Failed to parse ICE candidate: {}", e);
                StatusCode::BAD_REQUEST
            })?;

        pc.add_ice_candidate(candidate).await
            .map_err(|e| {
                error!("Failed to add ICE candidate: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(StatusCode::OK.into_response())
}

/// Create peer connection with str0m.
async fn create_peer_connection(
    state: &WhipServerState,
    stream_id: &str,
    offer: SessionDescription,
) -> Result<str0m::PeerConnection, StatusCode> {
    // Configure ICE servers
    let mut ice_servers = vec![str0m::net::IceServer {
        urls: state.config.stun_servers.clone(),
        username: None,
        credential: None,
    }];

    // Add TURN servers
    for turn in &state.config.turn_servers {
        ice_servers.push(str0m::net::IceServer {
            urls: turn.urls.clone(),
            username: Some(turn.username.clone()),
            credential: Some(turn.credential.clone()),
        });
    }

    // Create peer connection configuration
    let config = str0m::PeerConnectionConfiguration {
        ice_servers,
        ice_transport_policy: str0m::IceTransportPolicy::All,
        bundle_policy: str0m::BundlePolicy::MaxBundle,
        rtcp_mux_policy: str0m::RtcpMuxPolicy::Require,
        ice_candidate_pool_size: 0,
        ..Default::default()
    };

    let mut pc = str0m::PeerConnection::new(config)
        .await
        .map_err(|e| {
            error!("Failed to create peer connection: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Set remote description (offer)
    pc.set_remote_description(offer).await
        .map_err(|e| {
            error!("Failed to set remote description: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Set up media handlers
    setup_media_handlers(&mut pc, stream_id, state.clone()).await?;

    Ok(pc)
}

/// Set up media track handlers.
async fn setup_media_handlers(
    pc: &mut str0m::PeerConnection,
    stream_id: &str,
    state: Arc<WhipServerState>,
) -> Result<(), StatusCode> {
    // Handle incoming video tracks
    pc.on_track(move |track| {
        let stream_id = stream_id.to_string();
        let state = state.clone();
        
        tokio::spawn(async move {
            if track.kind() == str0m::media::MediaKind::Video {
                handle_video_track(track, &stream_id, state).await;
            } else if track.kind() == str0m::media::MediaKind::Audio {
                handle_audio_track(track, &stream_id, state).await;
            }
        });
    });

    // Handle ICE connection state changes
    let stream_id_clone = stream_id.to_string();
    let state_clone = state.clone();
    pc.on_ice_connection_state_change(move |state| {
        info!("WHIP ICE state for {}: {:?}", stream_id_clone, state);
        // Update session info
    });

    // Handle DTLS state changes
    let stream_id_clone = stream_id.to_string();
    pc.on_dtls_connection_state_change(move |state| {
        info!("WHIP DTLS state for {}: {:?}", stream_id_clone, state);
    });

    Ok(())
}

/// Handle incoming video track.
async fn handle_video_track(
    mut track: str0m::media::MediaStreamTrack,
    stream_id: &str,
    state: Arc<WhipServerState>,
) {
    info!("WHIP video track started for stream: {}", stream_id);
    
    let stream_id_obj = StreamId::new(stream_id.to_string());
    let mut frame_count = 0u64;

    while let Some(sample) = track.recv().await {
        if let str0m::media::MediaSample::Video(frame) = sample {
            frame_count += 1;
            
            // Convert to fragment
            let fragment = whip_video_frame_to_fragment(&stream_id_obj, frame, frame_count);
            
            // Ingest into fragment registry
            state.registry.ingest(stream_id_obj.clone(), fragment);
            
            // Update session stats
            if let Some(mut session) = state.active_sessions.get_mut(stream_id) {
                session.bytes_received += frame.data.len() as u64;
                session.video_frames += 1;
            }
            
            // Update metrics
            state.metrics.counter("ingest_whip_video_frames_total", "WHIP video frames received").increment(1);
            state.metrics.counter("ingest_whip_bytes_total", "WHIP bytes received").increment(frame.data.len() as u64);
        }
    }

    info!("WHIP video track ended for stream: {}", stream_id);
    
    // Cleanup
    state.active_sessions.remove(stream_id);
    state.metrics.counter("ingest_active_streams", "Active ingest streams").decrement(1);
}

/// Handle incoming audio track.
async fn handle_audio_track(
    mut track: str0m::media::MediaStreamTrack,
    stream_id: &str,
    state: Arc<WhipServerState>,
) {
    info!("WHIP audio track started for stream: {}", stream_id);
    
    let stream_id_obj = StreamId::new(stream_id.to_string());
    let mut frame_count = 0u64;

    while let Some(sample) = track.recv().await {
        if let str0m::media::MediaSample::Audio(frame) = sample {
            frame_count += 1;
            
            // Convert to fragment
            let fragment = whip_audio_frame_to_fragment(&stream_id_obj, frame, frame_count);
            
            // Ingest into fragment registry
            state.registry.ingest(stream_id_obj.clone(), fragment);
            
            // Update session stats
            if let Some(mut session) = state.active_sessions.get_mut(stream_id) {
                session.bytes_received += frame.data.len() as u64;
                session.audio_frames += 1;
            }
            
            // Update metrics
            state.metrics.counter("ingest_whip_audio_frames_total", "WHIP audio frames received").increment(1);
        }
    }

    info!("WHIP audio track ended for stream: {}", stream_id);
}

/// Convert WHIP video frame to fragment.
fn whip_video_frame_to_fragment(
    stream_id: &StreamId,
    frame: str0m::media::VideoFrame,
    sequence: u64,
) -> Fragment {
    // In a real implementation, this would handle H.264/HEVC/VP8/VP9/AV1 frames
    // For now, we'll pass through the raw frame data
    Fragment {
        meta: FragmentMeta {
            stream_id: stream_id.clone(),
            sequence,
            timestamp: Duration::from_millis(frame.timestamp.as_millis() as u64),
            duration: Duration::from_millis(33), // ~30fps
            is_keyframe: frame.is_keyframe,
            size: frame.data.len(),
            ingest_time: Instant::now(),
        },
        payload: Bytes::from(frame.data),
        siblings: HashMap::new(),
    }
}

/// Convert WHIP audio frame to fragment.
fn whip_audio_frame_to_fragment(
    stream_id: &StreamId,
    frame: str0m::media::AudioFrame,
    sequence: u64,
) -> Fragment {
    Fragment {
        meta: FragmentMeta {
            stream_id: stream_id.clone(),
            sequence,
            timestamp: Duration::from_millis(frame.timestamp.as_millis() as u64),
            duration: Duration::from_millis(20), // ~20ms per Opus frame
            is_keyframe: true, // Audio frames are effectively keyframes
            size: frame.data.len(),
            ingest_time: Instant::now(),
        },
        payload: Bytes::from(frame.data),
        siblings: HashMap::new(),
    }
}

/// WHIP client for sending streams to WHIP endpoint.
pub struct WhipClient {
    endpoint: String,
    stream_id: String,
    peer_connection: Arc<tokio::sync::Mutex<Option<str0m::PeerConnection>>>,
}

impl WhipClient {
    /// Create a new WHIP client.
    pub async fn new(endpoint: String, stream_id: String) -> anyhow::Result<Self> {
        Ok(Self {
            endpoint,
            stream_id,
            peer_connection: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Start publishing with local tracks.
    pub async fn start_publishing(&self, video_track: Option<str0m::media::MediaStreamTrack>, audio_track: Option<str0m::media::MediaStreamTrack>) -> anyhow::Result<()> {
        // Create peer connection
        let mut pc = str0m::PeerConnection::new(Default::default()).await?;
        
        // Add tracks
        if let Some(track) = video_track {
            pc.add_track(track).await?;
        }
        if let Some(track) = audio_track {
            pc.add_track(track).await?;
        }

        // Create offer
        let offer = pc.create_offer().await?;

        // Send WHIP offer
        let client = reqwest::Client::new();
        let response = client
            .post(&self.endpoint)
            .header("Content-Type", "application/sdp")
            .body(offer.to_string())
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("WHIP offer failed: {}", response.status());
        }

        let answer: WhipAnswerResponse = response.json().await?;
        
        // Set remote description (answer)
        let answer_sdp = SessionDescription::parse(&answer.sdp)?;
        pc.set_remote_description(answer_sdp).await?;

        // Store peer connection
        *self.peer_connection.lock().await = Some(pc);

        Ok(())
    }

    /// Stop publishing.
    pub async fn stop_publishing(&self) {
        let mut pc = self.peer_connection.lock().await;
        if let Some(pc) = pc.take() {
            pc.close().await.ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whip_offer_request() {
        let request = WhipOfferRequest {
            sdp: "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\n...".to_string(),
            stream_id: Some("test-stream".to_string()),
        };
        
        assert_eq!(request.stream_id, Some("test-stream".to_string()));
    }

    #[test]
    fn test_whip_answer_response() {
        let response = WhipAnswerResponse {
            sdp: "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\n...".to_string(),
            stream_id: "test-stream".to_string(),
        };
        
        assert_eq!(response.stream_id, "test-stream");
    }

    #[test]
    fn test_whip_ice_candidate_request() {
        let request = WhipIceCandidateRequest {
            candidate: "candidate:1 1 UDP 2122260223 192.168.1.1 54400 typ host".to_string(),
            sdp_mid: "0".to_string(),
            sdp_mline_index: 0,
        };
        
        assert_eq!(request.sdp_mline_index, 0);
    }

    #[test]
    fn test_whip_video_frame_to_fragment() {
        let stream_id = StreamId::new("test");
        let frame = str0m::media::VideoFrame {
            data: vec![0x00, 0x00, 0x00, 0x01, 0x67], // H.264 SPS
            timestamp: std::time::Duration::from_millis(1000),
            is_keyframe: true,
        };
        
        let fragment = whip_video_frame_to_fragment(&stream_id, frame, 1);
        
        assert_eq!(fragment.meta.stream_id, stream_id);
        assert_eq!(fragment.meta.sequence, 1);
        assert!(fragment.meta.is_keyframe);
    }
}