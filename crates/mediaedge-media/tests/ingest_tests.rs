//! MediaEdge Media Tests
//!
//! Integration tests for ingest protocols.

use mediaedge_media::ingest::{start_all_ingest_servers, IngestManager, IngestProtocol, IngestStatus};
use mediaedge_media::fragment::FragmentBroadcasterRegistry;
use mediaedge_config::types::GatewayConfig;
use mediaedge_observability::MetricsRegistry;
use std::sync::Arc;

#[tokio::test]
async fn test_ingest_manager() {
    let registry = Arc::new(FragmentBroadcasterRegistry::new());
    let metrics = Arc::new(MetricsRegistry::new(&Default::default()).unwrap());
    let manager = IngestManager::new(registry.clone(), metrics.clone());

    // Register streams for each protocol
    manager.register_stream("rtmp-stream".to_string(), IngestProtocol::Rtmp, Some("192.168.1.1:1234".to_string()));
    manager.register_stream("srt-stream".to_string(), IngestProtocol::Srt, Some("192.168.1.2:5678".to_string()));
    manager.register_stream("rist-stream".to_string(), IngestProtocol::Rist, Some("192.168.1.3:9012".to_string()));
    manager.register_stream("whip-stream".to_string(), IngestProtocol::Whip, Some("192.168.1.4:3456".to_string()));
    manager.register_stream("rtsp-stream".to_string(), IngestProtocol::Rtsp, Some("192.168.1.5:7890".to_string()));

    assert_eq!(manager.stream_count(), 5);

    // Check all streams registered
    for protocol in [IngestProtocol::Rtmp, IngestProtocol::Srt, IngestProtocol::Rist, IngestProtocol::Whip, IngestProtocol::Rtsp] {
        let stream_id = format!("{:?}-stream", protocol).to_lowercase();
        let info = manager.get_stream(&stream_id).unwrap();
        assert_eq!(info.protocol, protocol);
        assert_eq!(info.status, IngestStatus::Connecting);
    }

    // Update statuses
    manager.update_status("rtmp-stream", IngestStatus::Streaming);
    assert_eq!(manager.get_stream("rtmp-stream").unwrap().status, IngestStatus::Streaming);

    // Update stats
    manager.update_stats("rtmp-stream", 1024 * 1024, 5_000_000);
    let info = manager.get_stream("rtmp-stream").unwrap();
    assert_eq!(info.bytes_received, 1024 * 1024);
    assert_eq!(info.bitrate_bps, 5_000_000);

    // Remove stream
    manager.remove_stream("rtmp-stream");
    assert_eq!(manager.stream_count(), 4);
    assert!(manager.get_stream("rtmp-stream").is_none());
}

#[tokio::test]
async fn test_fragment_registry_ingest() {
    let registry = Arc::new(FragmentBroadcasterRegistry::new());
    let metrics = Arc::new(MetricsRegistry::new(&Default::default()).unwrap());
    
    let stream_id = crate::fragment::StreamId::new("test-stream");
    
    // Ingest fragments from different "protocols"
    for i in 0..10 {
        let fragment = create_test_fragment("test-stream", i, i % 5 == 0);
        registry.ingest(stream_id.clone(), fragment);
    }

    // Subscribe and verify
    let mut subscriber = registry.subscribe(stream_id.clone()).unwrap();
    let fragment = subscriber.try_next().unwrap();
    assert_eq!(fragment.meta.sequence, 9); // Latest fragment

    // Check history
    let broadcaster = registry.get(&stream_id).unwrap();
    let history = broadcaster.history();
    assert_eq!(history.len(), 10);
}

#[tokio::test]
async fn test_multiple_protocols_same_registry() {
    let registry = Arc::new(FragmentBroadcasterRegistry::new());
    let metrics = Arc::new(MetricsRegistry::new(&Default::default()).unwrap());
    
    // Simulate ingest from multiple protocols
    let rtmp_stream = crate::fragment::StreamId::new("rtmp-stream");
    let srt_stream = crate::fragment::StreamId::new("srt-stream");
    let rist_stream = crate::fragment::StreamId::new("rist-stream");
    
    for i in 0..5 {
        registry.ingest(rtmp_stream.clone(), create_test_fragment("rtmp-stream", i, false));
        registry.ingest(srt_stream.clone(), create_test_fragment("srt-stream", i, false));
        registry.ingest(rist_stream.clone(), create_test_fragment("rist-stream", i, false));
    }

    assert_eq!(registry.stream_count(), 3);
    assert!(registry.get(&rtmp_stream).is_some());
    assert!(registry.get(&srt_stream).is_some());
    assert!(registry.get(&rist_stream).is_some());
}

fn create_test_fragment(stream_id: &str, seq: u64, is_keyframe: bool) -> crate::fragment::Fragment {
    crate::fragment::Fragment {
        meta: crate::fragment::FragmentMeta {
            stream_id: crate::fragment::StreamId::new(stream_id),
            sequence: seq,
            timestamp: std::time::Duration::from_millis(seq * 100),
            duration: std::time::Duration::from_millis(100),
            is_keyframe,
            size: 1024,
            ingest_time: std::time::Instant::now(),
        },
        payload: bytes::Bytes::from(vec![0u8; 1024]),
        siblings: std::collections::HashMap::new(),
    }
}

#[test]
fn test_ingest_protocol_defaults() {
    assert_eq!(IngestProtocol::Rtmp.default_port(), 1935);
    assert_eq!(IngestProtocol::Srt.default_port(), 8888);
    assert_eq!(IngestProtocol::Rist.default_port(), 5000);
    assert_eq!(IngestProtocol::Whip.default_port(), 443);
    assert_eq!(IngestProtocol::Rtsp.default_port(), 554);
}

#[test]
fn test_ingest_protocol_names() {
    assert_eq!(IngestProtocol::Rtmp.name(), "RTMP");
    assert_eq!(IngestProtocol::Srt.name(), "SRT");
    assert_eq!(IngestProtocol::Rist.name(), "RIST");
    assert_eq!(IngestProtocol::Whip.name(), "WHIP");
    assert_eq!(IngestProtocol::Rtsp.name(), "RTSP");
}