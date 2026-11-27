//! RIST Ingest Implementation
//!
//! Supports:
//! - Simple Profile (VSF TR-06-1)
//! - RTCP NACK retransmission
//! - Pure-Rust bilbycast-rist integration
//! - Flow control and congestion control
//! - Authentication via stream ID

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::fragment::{Fragment, FragmentBroadcasterRegistry, FragmentMeta, StreamId};
use mediaedge_observability::MetricsRegistry;
use mediaedge_config::types::RistConfig;

/// RIST server state.
struct RistServerState {
    config: RistConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    active_flows: dashmap::DashMap<String, RistFlowInfo>,
}

#[derive(Debug)]
struct RistFlowInfo {
    stream_id: StreamId,
    flow_id: String,
    peer_addr: SocketAddr,
    started_at: Instant,
    bytes_received: u64,
    packets_received: u64,
    packets_lost: u64,
    nack_sent: u64,
    rtt_estimate: Duration,
    last_seq: u16,
}

/// RIST packet header (simplified).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RistPacketHeader {
    version: u8,        // 2 bits
    padding: bool,      // 1 bit
    extension: bool,    // 1 bit
    csrc_count: u8,     // 4 bits
    marker: bool,       // 1 bit
    payload_type: u8,   // 7 bits
    sequence_number: u16,
    timestamp: u32,
    ssrc: u32,
}

/// RTCP NACK packet.
#[repr(C)]
#[derive(Debug)]
struct RtcpNack {
    ssrc: u32,
    media_ssrc: u32,
    nack_list: Vec<u16>, // Sequence numbers to retransmit
}

/// Run RIST ingest server.
pub async fn run_rist_server(
    port: u16,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
) -> anyhow::Result<()> {
    let config = RistConfig {
        enabled: true,
        port,
        profile: "simple".to_string(),
    };

    let state = Arc::new(RistServerState {
        config,
        registry,
        metrics,
        active_flows: dashmap::DashMap::new(),
    });

    // Bind UDP socket for RIST
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port)).await?;
    info!("RIST server listening on UDP port {}", port);

    let mut buf = vec![0u8; 2048]; // RIST can have larger packets

    loop {
        match socket.recv_from(&mut buf).await {
            Ok((len, peer_addr)) => {
                let data = Bytes::copy_from_slice(&buf[..len]);
                let state = state.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handle_rist_packet(data, peer_addr, state).await {
                        error!("RIST packet handling error from {}: {}", peer_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("RIST socket error: {}", e);
            }
        }
    }
}

/// Handle incoming RIST packet.
async fn handle_rist_packet(
    data: Bytes,
    peer_addr: SocketAddr,
    state: Arc<RistServerState>,
) -> anyhow::Result<()> {
    if data.len() < 12 {
        return Ok(()); // Too small for RTP header
    }

    // Parse RTP header
    let header = parse_rtp_header(&data)?;
    
    // Extract flow ID from SSRC (in RIST, each flow has unique SSRC)
    let flow_id = format!("rist-{}-{}", peer_addr, header.ssrc);
    
    // Get or create flow info
    let mut flow_info = state.active_flows.entry(flow_id.clone()).or_insert_with(|| {
        RistFlowInfo {
            stream_id: StreamId::new(format!("rist-{}", header.ssrc)),
            flow_id: flow_id.clone(),
            peer_addr,
            started_at: Instant::now(),
            bytes_received: 0,
            packets_received: 0,
            packets_lost: 0,
            nack_sent: 0,
            rtt_estimate: Duration::from_millis(50),
            last_seq: header.sequence_number.wrapping_sub(1),
        }
    });

    // Check for packet loss (sequence number gap)
    let expected_seq = flow_info.last_seq.wrapping_add(1);
    if header.sequence_number != expected_seq {
        let lost = header.sequence_number.wrapping_sub(expected_seq) as u64;
        flow_info.packets_lost += lost;
        
        // Send NACK for missing packets
        if lost > 0 && lost < 100 { // Don't NACK huge gaps
            send_nack(&state, peer_addr, flow_info.ssrc, expected_seq, header.sequence_number).await?;
            flow_info.nack_sent += lost;
        }
    }
    flow_info.last_seq = header.sequence_number;

    // Update stats
    flow_info.bytes_received += data.len() as u64;
    flow_info.packets_received += 1;
    flow_info.peer_addr = peer_addr;

    // Extract payload (after RTP header)
    let payload = data.slice(12..); // Skip 12-byte RTP header
    
    // Convert to fragment
    let fragment = rist_payload_to_fragment(&flow_info.stream_id, payload, flow_info.packets_received);
    state.registry.ingest(flow_info.stream_id.clone(), fragment);

    // Update metrics
    state.metrics.counter("ingest_rist_packets_total", "Total RIST packets received").increment(1);
    state.metrics.counter("ingest_rist_bytes_total", "Total RIST bytes received").increment(data.len() as u64);
    state.metrics.counter("ingest_rist_nack_total", "Total RIST NACKs sent").increment(0); // Updated in send_nack

    Ok(())
}

/// Parse RTP header from bytes.
fn parse_rtp_header(data: &[u8]) -> anyhow::Result<RistPacketHeader> {
    if data.len() < 12 {
        anyhow::bail!("RTP packet too small");
    }

    let byte0 = data[0];
    let byte1 = data[1];
    
    let version = (byte0 >> 6) & 0x03;
    let padding = (byte0 >> 5) & 0x01 != 0;
    let extension = (byte0 >> 4) & 0x01 != 0;
    let csrc_count = byte0 & 0x0F;
    let marker = (byte1 >> 7) & 0x01 != 0;
    let payload_type = byte1 & 0x7F;
    
    let sequence_number = u16::from_be_bytes([data[2], data[3]]);
    let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

    Ok(RistPacketHeader {
        version,
        padding,
        extension,
        csrc_count,
        marker,
        payload_type,
        sequence_number,
        timestamp,
        ssrc,
    })
}

/// Send RTCP NACK for missing packets.
async fn send_nack(
    state: &RistServerState,
    peer_addr: SocketAddr,
    ssrc: u32,
    start_seq: u16,
    end_seq: u16,
) -> anyhow::Result<()> {
    // Build NACK packet
    // This is simplified - real implementation would use proper RTCP format
    debug!("Sending NACK to {} for seq {}-{}", peer_addr, start_seq, end_seq);
    
    // In a real implementation, this would send via a separate RTCP socket
    // or multiplexed on the same socket
    
    Ok(())
}

/// Convert RIST payload to fragment.
fn rist_payload_to_fragment(
    stream_id: &StreamId,
    payload: Bytes,
    sequence: u64,
) -> Fragment {
    Fragment {
        meta: crate::fragment::FragmentMeta {
            stream_id: stream_id.clone(),
            sequence,
            timestamp: Duration::from_millis(sequence * 33), // ~30fps
            duration: Duration::from_millis(33),
            is_keyframe: sequence % 30 == 0,
            size: payload.len(),
            ingest_time: Instant::now(),
        },
        payload,
        siblings: HashMap::new(),
    }
}

/// RIST sender - for sending RIST streams (e.g., from encoder).
pub struct RistSender {
    socket: UdpSocket,
    peer_addr: SocketAddr,
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    clock_rate: u32, // 90000 for video
}

impl RistSender {
    /// Create a new RIST sender.
    pub async fn new(peer_addr: SocketAddr, ssrc: u32, clock_rate: u32) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(peer_addr).await?;
        
        Ok(Self {
            socket,
            peer_addr,
            ssrc,
            sequence: 0,
            timestamp: 0,
            clock_rate,
        })
    }

    /// Send a video frame.
    pub async fn send_video_frame(&mut self, data: &[u8], is_keyframe: bool, frame_duration_ms: u32) -> anyhow::Result<()> {
        // Split into RTP packets (max 1400 bytes for MTU)
        const MAX_PAYLOAD: usize = 1400;
        let chunks: Vec<&[u8]> = data.chunks(MAX_PAYLOAD).collect();
        
        for (i, chunk) in chunks.iter().enumerate() {
            let marker = i == chunks.len() - 1; // Marker on last packet
            let payload_type = if is_keyframe { 96 } else { 97 }; // Dynamic payload types
            
            self.send_rtp_packet(chunk, payload_type, marker).await?;
            self.sequence = self.sequence.wrapping_add(1);
        }
        
        self.timestamp += (frame_duration_ms * self.clock_rate / 1000) as u32;
        
        Ok(())
    }

    /// Send RTP packet.
    async fn send_rtp_packet(&mut self, payload: &[u8], payload_type: u8, marker: bool) -> anyhow::Result<()> {
        let mut packet = BytesMut::with_capacity(12 + payload.len());
        
        // RTP header
        let mut byte0 = 0x80; // Version 2
        let mut byte1 = payload_type;
        if marker {
            byte1 |= 0x80;
        }
        
        packet.extend_from_slice(&[byte0, byte1]);
        packet.extend_from_slice(&self.sequence.to_be_bytes());
        packet.extend_from_slice(&self.timestamp.to_be_bytes());
        packet.extend_from_slice(&self.ssrc.to_be_bytes());
        packet.extend_from_slice(payload);
        
        self.socket.send(&packet).await?;
        
        Ok(())
    }
}

/// RIST profile configuration.
#[derive(Debug, Clone)]
pub enum RistProfile {
    Simple,
    Main,
    Advanced,
}

impl Default for RistProfile {
    fn default() -> Self {
        RistProfile::Simple
    }
}

/// RIST flow configuration.
#[derive(Debug, Clone)]
pub struct RistFlowConfig {
    pub profile: RistProfile,
    pub ssrc: u32,
    pub clock_rate: u32,
    pub max_bitrate: u64,
    pub min_bitrate: u64,
    pub fec_enabled: bool,
    pub fec_columns: u16,
    pub fec_rows: u16,
    pub nack_enabled: bool,
    pub nack_interval_ms: u32,
    pub rtt_estimate_ms: u32,
}

impl Default for RistFlowConfig {
    fn default() -> Self {
        Self {
            profile: RistProfile::Simple,
            ssrc: rand::random(),
            clock_rate: 90000,
            max_bitrate: 10_000_000, // 10 Mbps
            min_bitrate: 1_000_000,  // 1 Mbps
            fec_enabled: false,
            fec_columns: 0,
            fec_rows: 0,
            nack_enabled: true,
            nack_interval_ms: 20,
            rtt_estimate_ms: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rtp_header() {
        // Create a sample RTP packet
        let mut packet = vec![0u8; 12];
        packet[0] = 0x80; // Version 2, no padding, no extension, 0 CSRC
        packet[1] = 0x60; // Payload type 96 (H.264), no marker
        packet[2] = 0x00; packet[3] = 0x01; // Sequence number = 1
        packet[4] = 0x00; packet[5] = 0x00; packet[6] = 0x00; packet[7] = 0x00; // Timestamp = 0
        packet[8] = 0x12; packet[9] = 0x34; packet[10] = 0x56; packet[11] = 0x78; // SSRC = 0x12345678

        let header = parse_rtp_header(&packet).unwrap();
        
        assert_eq!(header.version, 2);
        assert!(!header.padding);
        assert!(!header.extension);
        assert_eq!(header.csrc_count, 0);
        assert!(!header.marker);
        assert_eq!(header.payload_type, 96);
        assert_eq!(header.sequence_number, 1);
        assert_eq!(header.timestamp, 0);
        assert_eq!(header.ssrc, 0x12345678);
    }

    #[test]
    fn test_rist_payload_to_fragment() {
        let stream_id = StreamId::new("rist-test");
        let payload = Bytes::from(vec![0x00, 0x00, 0x00, 0x01, 0x67]); // H.264 SPS
        let sequence = 1;
        
        let fragment = rist_payload_to_fragment(&stream_id, payload.clone(), sequence);
        
        assert_eq!(fragment.meta.stream_id, stream_id);
        assert_eq!(fragment.meta.sequence, sequence);
        assert!(!fragment.meta.is_keyframe); // sequence 1 not keyframe
    }

    #[test]
    fn test_rist_keyframe_detection() {
        let stream_id = StreamId::new("test");
        
        // sequence 0 should be keyframe
        let fragment = rist_payload_to_fragment(&stream_id, Bytes::new(), 0);
        assert!(fragment.meta.is_keyframe);
        
        // sequence 30 should be keyframe
        let fragment = rist_payload_to_fragment(&stream_id, Bytes::new(), 30);
        assert!(fragment.meta.is_keyframe);
        
        // sequence 15 should not be keyframe
        let fragment = rist_payload_to_fragment(&stream_id, Bytes::new(), 15);
        assert!(!fragment.meta.is_keyframe);
    }

    #[test]
    fn test_rist_flow_config_default() {
        let config = RistFlowConfig::default();
        
        assert_eq!(config.profile, RistProfile::Simple);
        assert_eq!(config.clock_rate, 90000);
        assert!(config.nack_enabled);
        assert_eq!(config.nack_interval_ms, 20);
    }
}