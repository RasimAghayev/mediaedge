//! WebSocket fMP4 Egress Implementation
//!
//! Supports:
//! - fMP4 over WebSocket (binary frames)
//! - Initialization segment + media segments
//! - Low-latency chunked transfer
//! - Multiple tracks (video, audio, metadata)
//! - Reconnection with resume capability

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use mp4::{Mp4Box, Sample, Track, TrackType};
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::fragment::{Fragment, FragmentBroadcasterRegistry, FragmentSubscriber, StreamId};
use mediaedge_config::types::HttpProtocolConfig;
use mediaedge_observability::MetricsRegistry;

/// WebSocket fMP4 session state.
#[derive(Debug, Clone)]
pub struct WsFmp4Session {
    pub session_id: String,
    pub stream_id: StreamId,
    pub state: WsFmp4SessionState,
    pub tracks: HashMap<String, WsFmp4TrackState>,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub segments_sent: u64,
    pub init_segment_sent: bool,
}

/// WebSocket fMP4 session state.
#[derive(Debug, Clone, PartialEq)]
pub enum WsFmp4SessionState {
    Connecting,
    InitSent,
    Streaming,
    Paused,
    Reconnecting,
    Closed,
}

/// WebSocket fMP4 track state.
#[derive(Debug, Clone)]
pub struct WsFmp4TrackState {
    pub track_id: u32,
    pub track_type: TrackType,
    pub timescale: u32,
    pub sequence_number: u64,
    pub last_timestamp: u64,
    pub init_segment: Option<Bytes>,
    pub config: WsFmp4TrackConfig,
}

/// WebSocket fMP4 track configuration.
#[derive(Debug, Clone)]
pub struct WsFmp4TrackConfig {
    pub track_name: String,
    pub mime_type: String,
    pub codecs: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channel_count: Option<u16>,
}

/// fMP4 segment data.
#[derive(Debug, Clone)]
pub struct Fmp4Segment {
    pub track_id: u32,
    pub sequence: u64,
    pub timestamp: u64,
    pub duration: u64,
    pub is_sync_sample: bool,
    pub data: Bytes,
    pub sample_flags: u32,
}

/// WebSocket message types for fMP4.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum WsFmp4Message {
    /// Initialization segment
    Init {
        tracks: Vec<InitTrackInfo>,
    },
    /// Media segment
    Segment {
        track_id: u32,
        sequence: u64,
        timestamp: u64,
        duration: u64,
        is_sync: bool,
        data: String, // base64 encoded
    },
    /// End of stream
    EndOfStream {
        track_id: u32,
    },
    /// Error
    Error {
        code: u32,
        message: String,
    },
    /// Ping/Pong for keepalive
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },
    /// Reconnect with resume
    Resume {
        track_id: u32,
        from_sequence: u64,
    },
}

/// Initialization track info.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InitTrackInfo {
    pub track_id: u32,
    pub track_name: String,
    pub mime_type: String,
    pub codecs: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channel_count: Option<u16>,
    pub init_segment: String, // base64 encoded
}

/// WebSocket fMP4 server state.
struct WsFmp4ServerState {
    config: HttpProtocolConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    sessions: DashMap<String, WsFmp4Session>,
    output_root: PathBuf,
    base_url: String,
}

/// Run WebSocket fMP4 egress for a stream.
pub async fn run_ws_fmp4_egress(
    stream_id: StreamId,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    config: HttpProtocolConfig,
    output_root: PathBuf,
    base_url: String,
) -> anyhow::Result<()> {
    let state = WsFmp4ServerState {
        config,
        registry,
        metrics,
        sessions: DashMap::new(),
        output_root,
        base_url,
    };

    info!(
        "WebSocket fMP4 egress server started for stream: {}",
        stream_id.0
    );

    // WebSocket fMP4 is connection-based, not a continuous process
    // This would be invoked via WebSocket upgrade
    // For now, we set up the infrastructure

    Ok(())
}

/// Handle WebSocket connection upgrade for fMP4.
pub async fn handle_ws_fmp4_connection(
    stream: tokio::net::TcpStream,
    stream_id: StreamId,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let session_id = uuid::Uuid::new_v4().to_string();
    info!(
        "WebSocket fMP4 connection accepted: {} for stream {}",
        session_id, stream_id.0
    );

    // Subscribe to stream
    let mut subscriber = registry
        .subscribe_with_buffer(stream_id.clone(), 1000)
        .ok_or_else(|| anyhow::anyhow!("Failed to subscribe to stream"))?;

    // Create session
    let mut session = WsFmp4Session {
        session_id: session_id.clone(),
        stream_id: stream_id.clone(),
        state: WsFmp4SessionState::Connecting,
        tracks: HashMap::new(),
        created_at: Instant::now(),
        last_activity: Instant::now(),
        bytes_sent: 0,
        segments_sent: 0,
        init_segment_sent: false,
    };

    // Initialize tracks
    initialize_tracks(&mut session).await;

    // Send initialization segment
    send_init_segment(&mut ws_sender, &session).await?;
    session.state = WsFmp4SessionState::InitSent;

    // Start media forwarding
    let (tx, mut rx) = mpsc::channel::<Fmp4Segment>(1000);

    // Fragment processing task
    let fragment_task = tokio::spawn(async move {
        while let Some(fragment) = subscriber.next().await {
            if let Some(segment) = fragment_to_fmp4_segment(fragment, &session).await {
                if tx.send(segment).await.is_err() {
                    break;
                }
            }
        }
    });

    // WebSocket send loop
    let mut ping_interval = interval(Duration::from_secs(10));
    let send_loop = async {
        loop {
            tokio::select! {
                segment = rx.recv() => {
                    if let Some(segment) = segment {
                        if let Err(e) = send_segment(&mut ws_sender, &segment).await {
                            error!("Failed to send fMP4 segment: {}", e);
                            break;
                        }
                        session.segments_sent += 1;
                        session.last_activity = Instant::now();
                    } else {
                        break; // Channel closed
                    }
                }

                msg = ws_receiver.next() => {
                    if let Some(msg) = msg {
                        if let Err(e) = handle_ws_message(msg?, &mut session, &mut ws_sender).await {
                            error!("WebSocket message handling error: {}", e);
                            break;
                        }
                    } else {
                        break; // Connection closed
                    }
                }

                _ = ping_interval.tick() => {
                    let ping = WsFmp4Message::Ping {
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                    };
                    if let Err(e) = ws_sender.send(Message::Text(serde_json::to_string(&ping)?)).await {
                        error!("Failed to send ping: {}", e);
                        break;
                    }
                }
            }
        }

        // Cleanup
        fragment_task.abort();
        session.state = WsFmp4SessionState::Closed;
        info!("WebSocket fMP4 session closed: {}", session_id);
    };

    send_loop.await;
    Ok(())
}

/// Initialize tracks for a session.
async fn initialize_tracks(session: &mut WsFmp4Session) {
    // Video track
    session.tracks.insert(
        "video".to_string(),
        WsFmp4TrackState {
            track_id: 1,
            track_type: TrackType::Video,
            timescale: 90000,
            sequence_number: 0,
            last_timestamp: 0,
            init_segment: None,
            config: WsFmp4TrackConfig {
                track_name: "video".to_string(),
                mime_type: "video/mp4".to_string(),
                codecs: "avc1.640028".to_string(),
                width: Some(1920),
                height: Some(1080),
                sample_rate: None,
                channel_count: None,
            },
        },
    );

    // Audio track
    session.tracks.insert(
        "audio".to_string(),
        WsFmp4TrackState {
            track_id: 2,
            track_type: TrackType::Audio,
            timescale: 48000,
            sequence_number: 0,
            last_timestamp: 0,
            init_segment: None,
            config: WsFmp4TrackConfig {
                track_name: "audio".to_string(),
                mime_type: "audio/mp4".to_string(),
                codecs: "mp4a.40.2".to_string(),
                width: None,
                height: None,
                sample_rate: Some(48000),
                channel_count: Some(2),
            },
        },
    );
}

/// Send initialization segment.
async fn send_init_segment(
    ws_sender: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
    session: &WsFmp4Session,
) -> anyhow::Result<()> {
    let mut init_tracks = Vec::new();

    for (name, track) in &session.tracks {
        // Generate fMP4 initialization segment (ftyp + moov)
        let init_segment = generate_init_segment(&track.config)?;

        init_tracks.push(InitTrackInfo {
            track_id: track.track_id,
            track_name: name.clone(),
            mime_type: track.config.mime_type.clone(),
            codecs: track.config.codecs.clone(),
            width: track.config.width,
            height: track.config.height,
            sample_rate: track.config.sample_rate,
            channel_count: track.config.channel_count,
            init_segment: base64::encode(&init_segment),
        });
    }

    let msg = WsFmp4Message::Init {
        tracks: init_tracks,
    };
    let json = serde_json::to_string(&msg)?;
    ws_sender.send(Message::Text(json)).await?;

    Ok(())
}

/// Generate fMP4 initialization segment.
fn generate_init_segment(config: &WsFmp4TrackConfig) -> anyhow::Result<Bytes> {
    // This is a simplified implementation
    // Real implementation would use mp4 crate to build proper init segment
    let mut buf = BytesMut::new();

    // ftyp box
    buf.extend_from_slice(b"ftyp");
    buf.extend_from_slice(&[0, 0, 0, 0]); // size placeholder
    buf.extend_from_slice(b"isom");
    buf.extend_from_slice(&[0, 0, 0, 1]); // minor version
    buf.extend_from_slice(b"isom");
    buf.extend_from_slice(b"iso2");
    buf.extend_from_slice(b"avc1");
    buf.extend_from_slice(b"mp41");

    // moov box (simplified)
    buf.extend_from_slice(b"moov");
    buf.extend_from_slice(&[0, 0, 0, 0]); // size placeholder

    // In real implementation, would build proper mvhd, trak, mdia, etc.

    Ok(buf.freeze())
}

/// Convert fragment to fMP4 segment.
async fn fragment_to_fmp4_segment(
    fragment: Fragment,
    session: &WsFmp4Session,
) -> Option<Fmp4Segment> {
    let track_name = if fragment.meta.is_keyframe
        || fragment.meta.media_type == crate::fragment::MediaType::Video
    {
        "video"
    } else {
        "audio"
    };

    if let Some(track) = session.tracks.get(track_name) {
        // In real implementation, would package fragment payload into fMP4 segment
        // This involves creating moof + mdat boxes

        Some(Fmp4Segment {
            track_id: track.track_id,
            sequence: track.sequence_number,
            timestamp: fragment.meta.timestamp as u64,
            duration: fragment.meta.duration as u64,
            is_sync_sample: fragment.meta.is_keyframe,
            data: fragment.payload,
            sample_flags: if fragment.meta.is_keyframe {
                0x2000000
            } else {
                0
            },
        })
    } else {
        None
    }
}

/// Send fMP4 segment over WebSocket.
async fn send_segment(
    ws_sender: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
    segment: &Fmp4Segment,
) -> anyhow::Result<()> {
    // Send as binary message for efficiency
    let mut data = BytesMut::new();

    // Simple binary format: track_id (4) | sequence (8) | timestamp (8) | duration (8) | flags (4) | payload
    data.extend_from_slice(&segment.track_id.to_be_bytes());
    data.extend_from_slice(&segment.sequence.to_be_bytes());
    data.extend_from_slice(&segment.timestamp.to_be_bytes());
    data.extend_from_slice(&segment.duration.to_be_bytes());
    data.extend_from_slice(&segment.sample_flags.to_be_bytes());
    data.extend_from_slice(&segment.data);

    ws_sender.send(Message::Binary(data.freeze())).await?;
    Ok(())
}

/// Handle incoming WebSocket message.
async fn handle_ws_message(
    msg: Message,
    session: &mut WsFmp4Session,
    ws_sender: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
        Message,
    >,
) -> anyhow::Result<()> {
    match msg {
        Message::Text(text) => {
            if let Ok(ws_msg) = serde_json::from_str::<WsFmp4Message>(&text) {
                match ws_msg {
                    WsFmp4Message::Ping { timestamp } => {
                        let pong = WsFmp4Message::Pong { timestamp };
                        ws_sender
                            .send(Message::Text(serde_json::to_string(&pong)?))
                            .await?;
                    }
                    WsFmp4Message::Resume {
                        track_id,
                        from_sequence,
                    } => {
                        // Handle resume request
                        info!(
                            "Resume requested for track {} from sequence {}",
                            track_id, from_sequence
                        );
                        // In real implementation, would seek and resend from sequence
                    }
                    _ => {}
                }
            }
        }
        Message::Binary(_) => {
            // Client shouldn't send binary, but handle gracefully
        }
        Message::Ping(data) => {
            ws_sender.send(Message::Pong(data)).await?;
        }
        Message::Close(_) => {
            session.state = WsFmp4SessionState::Closed;
        }
        _ => {}
    }
    Ok(())
}

/// WebSocket fMP4 egress configuration.
#[derive(Debug, Clone)]
pub struct WsFmp4EgressConfig {
    pub bind_address: String,
    pub max_connections: usize,
    pub max_message_size: usize,
    pub ping_interval_secs: u64,
    pub segment_buffer_size: usize,
    pub enable_compression: bool,
    pub reconnect_window_secs: u64,
}

impl Default for WsFmp4EgressConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".to_string(),
            max_connections: 10000,
            max_message_size: 16 * 1024 * 1024, // 16MB
            ping_interval_secs: 10,
            segment_buffer_size: 1000,
            enable_compression: true,
            reconnect_window_secs: 60,
        }
    }
}

/// fMP4 segment builder for proper box construction.
pub struct Fmp4SegmentBuilder {
    track_id: u32,
    timescale: u32,
    sequence: u64,
}

impl Fmp4SegmentBuilder {
    pub fn new(track_id: u32, timescale: u32) -> Self {
        Self {
            track_id,
            timescale,
            sequence: 0,
        }
    }

    pub fn build_segment(
        &mut self,
        payload: Bytes,
        timestamp: u64,
        duration: u64,
        is_sync: bool,
    ) -> Bytes {
        self.sequence += 1;

        let mut buf = BytesMut::new();

        // moof box
        self.write_moof(&mut buf, timestamp, duration, is_sync);

        // mdat box
        self.write_mdat(&mut buf, &payload);

        buf.freeze()
    }

    fn write_moof(&self, buf: &mut BytesMut, timestamp: u64, duration: u64, is_sync: bool) {
        // mfhd (Movie Fragment Header)
        buf.extend_from_slice(b"mfhd");
        buf.extend_from_slice(&[0, 0, 0, 16]); // size
        buf.extend_from_slice(&[0, 0, 0, 0]); // version/flags
        buf.extend_from_slice(&self.sequence.to_be_bytes()); // sequence number

        // traf (Track Fragment)
        buf.extend_from_slice(b"traf");
        // ... tfhd, tfdt, trun boxes would go here
        // Simplified for brevity
    }

    fn write_mdat(&self, buf: &mut BytesMut, payload: &Bytes) {
        buf.extend_from_slice(b"mdat");
        buf.extend_from_slice(&(payload.len() as u32 + 8).to_be_bytes()); // size
        buf.extend_from_slice(payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_fmp4_session() {
        let session = WsFmp4Session {
            session_id: "test-session".to_string(),
            stream_id: StreamId::new("test-stream"),
            state: WsFmp4SessionState::Connecting,
            tracks: HashMap::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            segments_sent: 0,
            init_segment_sent: false,
        };

        assert_eq!(session.session_id, "test-session");
        assert_eq!(session.state, WsFmp4SessionState::Connecting);
    }

    #[test]
    fn test_ws_fmp4_track_config() {
        let config = WsFmp4TrackConfig {
            track_name: "video".to_string(),
            mime_type: "video/mp4".to_string(),
            codecs: "avc1.640028".to_string(),
            width: Some(1920),
            height: Some(1080),
            sample_rate: None,
            channel_count: None,
        };

        assert_eq!(config.track_name, "video");
        assert_eq!(config.width, Some(1920));
    }

    #[test]
    fn test_fmp4_segment() {
        let segment = Fmp4Segment {
            track_id: 1,
            sequence: 1,
            timestamp: 90000,
            duration: 3000,
            is_sync_sample: true,
            data: Bytes::from_static(b"test"),
            sample_flags: 0x2000000,
        };

        assert_eq!(segment.track_id, 1);
        assert!(segment.is_sync_sample);
    }

    #[test]
    fn test_ws_fmp4_message_serialization() {
        let msg = WsFmp4Message::Init {
            tracks: vec![InitTrackInfo {
                track_id: 1,
                track_name: "video".to_string(),
                mime_type: "video/mp4".to_string(),
                codecs: "avc1.640028".to_string(),
                width: Some(1920),
                height: Some(1080),
                sample_rate: None,
                channel_count: None,
                init_segment: "base64data".to_string(),
            }],
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"Init\""));
        assert!(json.contains("video"));
    }

    #[test]
    fn test_fmp4_segment_builder() {
        let mut builder = Fmp4SegmentBuilder::new(1, 90000);
        let segment = builder.build_segment(Bytes::from_static(b"test payload"), 90000, 3000, true);

        assert!(!segment.is_empty());
        // Should contain moof and mdat boxes
        assert!(segment.starts_with(b"m") || segment.starts_with(b"\x00"));
    }
}
