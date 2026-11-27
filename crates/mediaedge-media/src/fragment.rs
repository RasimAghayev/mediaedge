//! Fragment Broadcaster Registry
//!
//! Unified fragment model - all ingest protocols funnel through this registry.
//! Similar to LVQR's FragmentBroadcasterRegistry.
//!
//! Features:
//! - Single fragment model for all protocols
//! - Sibling tracks (captions, SCTE-35, timing, catalog)
//! - Subscriber management with backpressure
//! - Metrics integration

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info, warn};

use mediaedge_observability::MetricsRegistry;

/// Unique stream identifier.
#[derive(Debug, Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StreamId(pub String);

impl StreamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Fragment metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FragmentMeta {
    pub stream_id: StreamId,
    pub sequence: u64,
    pub timestamp: Duration,
    pub duration: Duration,
    pub is_keyframe: bool,
    pub size: usize,
    pub ingest_time: Instant,
}

/// Media fragment with payload and metadata.
#[derive(Debug, Clone)]
pub struct Fragment {
    pub meta: FragmentMeta,
    pub payload: Bytes,
    /// Sibling tracks (captions, SCTE-35, timing, catalog)
    pub siblings: HashMap<String, Bytes>,
}

/// Subscriber handle for receiving fragments.
#[derive(Debug)]
pub struct FragmentSubscriber {
    stream_id: StreamId,
    sender: mpsc::Sender<Fragment>,
    receiver: mpsc::Receiver<Fragment>,
    subscription_time: Instant,
}

impl FragmentSubscriber {
    /// Create a new subscriber.
    pub fn new(stream_id: StreamId, buffer_size: usize) -> Self {
        let (sender, receiver) = mpsc::channel(buffer_size);
        Self {
            stream_id,
            sender,
            receiver,
            subscription_time: Instant::now(),
        }
    }

    /// Get the stream ID.
    pub fn stream_id(&self) -> &StreamId {
        &self.stream_id
    }

    /// Receive next fragment.
    pub async fn next(&mut self) -> Option<Fragment> {
        self.receiver.recv().await
    }

    /// Try to receive next fragment (non-blocking).
    pub fn try_next(&mut self) -> Option<Fragment> {
        self.receiver.try_recv().ok()
    }

    /// Get subscription duration.
    pub fn subscription_duration(&self) -> Duration {
        self.subscription_time.elapsed()
    }
}

/// Fragment broadcaster for a single stream.
struct FragmentBroadcaster {
    stream_id: StreamId,
    subscribers: RwLock<HashMap<u64, mpsc::Sender<Fragment>>>,
    next_subscriber_id: std::sync::atomic::AtomicU64,
    latest_fragment: RwLock<Option<Fragment>>,
    fragment_history: RwLock<Vec<Fragment>>,
    max_history: usize,
    metrics: Arc<MetricsRegistry>,
}

impl FragmentBroadcaster {
    fn new(stream_id: StreamId, max_history: usize, metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            stream_id,
            subscribers: RwLock::new(HashMap::new()),
            next_subscriber_id: std::sync::atomic::AtomicU64::new(0),
            latest_fragment: RwLock::new(None),
            fragment_history: RwLock::new(Vec::new()),
            max_history,
            metrics,
        }
    }

    /// Broadcast a fragment to all subscribers.
    fn broadcast(&self, fragment: Fragment) {
        // Update latest fragment
        *self.latest_fragment.write() = Some(fragment.clone());

        // Add to history
        {
            let mut history = self.fragment_history.write();
            history.push(fragment.clone());
            if history.len() > self.max_history {
                history.drain(0..history.len() - self.max_history);
            }
        }

        // Broadcast to all subscribers
        let subscribers = self.subscribers.read();
        let mut dead_subscribers = Vec::new();

        for (id, sender) in subscribers.iter() {
            if sender.try_send(fragment.clone()).is_err() {
                dead_subscribers.push(*id);
            }
        }

        // Remove dead subscribers
        if !dead_subscribers.is_empty() {
            let mut subscribers = self.subscribers.write();
            for id in dead_subscribers {
                subscribers.remove(&id);
            }
        }

        // Record metrics
        let active = subscribers.len() as u64;
        self.metrics
            .gauge("ingest_active_streams", "Active ingest streams")
            .set(active as f64);
        self.metrics
            .counter("ingest_fragments_total", "Total fragments ingested")
            .increment(1);
    }

    /// Subscribe to fragments.
    fn subscribe(&self, buffer_size: usize) -> FragmentSubscriber {
        let id = self
            .next_subscriber_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let subscriber = FragmentSubscriber::new(self.stream_id.clone(), buffer_size);

        self.subscribers
            .write()
            .insert(id, subscriber.sender.clone());

        // Send latest fragment immediately if available
        if let Some(latest) = self.latest_fragment.read().as_ref() {
            let _ = subscriber.sender.try_send(latest.clone());
        }

        subscriber
    }

    /// Get subscriber count.
    fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    /// Get latest fragment.
    fn latest(&self) -> Option<Fragment> {
        self.latest_fragment.read().clone()
    }

    /// Get fragment history.
    fn history(&self) -> Vec<Fragment> {
        self.fragment_history.read().clone()
    }
}

/// Fragment broadcaster registry - central registry for all streams.
pub struct FragmentBroadcasterRegistry {
    broadcasters: DashMap<StreamId, Arc<FragmentBroadcaster>>,
    metrics: Arc<MetricsRegistry>,
    max_history_per_stream: usize,
    default_buffer_size: usize,
}

impl FragmentBroadcasterRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self::with_config(1000, 100)
    }

    /// Create with custom config.
    pub fn with_config(max_history_per_stream: usize, default_buffer_size: usize) -> Self {
        Self {
            broadcasters: DashMap::new(),
            metrics: Arc::new(MetricsRegistry::new(&Default::default()).unwrap()),
            max_history_per_stream,
            default_buffer_size,
        }
    }

    /// Create with metrics registry.
    pub fn with_metrics(
        metrics: Arc<MetricsRegistry>,
        max_history_per_stream: usize,
        default_buffer_size: usize,
    ) -> Self {
        Self {
            broadcasters: DashMap::new(),
            metrics,
            max_history_per_stream,
            default_buffer_size,
        }
    }

    /// Get or create a broadcaster for a stream.
    pub fn get_or_create(&self, stream_id: StreamId) -> Arc<FragmentBroadcaster> {
        self.broadcasters
            .entry(stream_id.clone())
            .or_insert_with(|| {
                info!(
                    "Created new fragment broadcaster for stream: {}",
                    stream_id.0
                );
                self.metrics
                    .counter("ingest_streams_total", "Total ingest streams")
                    .increment(1);
                Arc::new(FragmentBroadcaster::new(
                    stream_id,
                    self.max_history_per_stream,
                    self.metrics.clone(),
                ))
            })
            .clone()
    }

    /// Get a broadcaster for a stream (if exists).
    pub fn get(&self, stream_id: &StreamId) -> Option<Arc<FragmentBroadcaster>> {
        self.broadcasters.get(stream_id).map(|b| b.clone())
    }

    /// Remove a broadcaster (when stream ends).
    pub fn remove(&self, stream_id: &StreamId) -> bool {
        if self.broadcasters.remove(stream_id).is_some() {
            info!("Removed fragment broadcaster for stream: {}", stream_id.0);
            true
        } else {
            false
        }
    }

    /// Ingest a fragment for a stream.
    pub fn ingest(&self, stream_id: StreamId, fragment: Fragment) {
        let broadcaster = self.get_or_create(stream_id);
        broadcaster.broadcast(fragment);
    }

    /// Subscribe to a stream.
    pub fn subscribe(&self, stream_id: StreamId) -> Option<FragmentSubscriber> {
        self.get(&stream_id)
            .map(|b| b.subscribe(self.default_buffer_size))
    }

    /// Subscribe with custom buffer size.
    pub fn subscribe_with_buffer(
        &self,
        stream_id: StreamId,
        buffer_size: usize,
    ) -> Option<FragmentSubscriber> {
        self.get(&stream_id).map(|b| b.subscribe(buffer_size))
    }

    /// Get active stream count.
    pub fn stream_count(&self) -> usize {
        self.broadcasters.len()
    }

    /// Get total subscriber count across all streams.
    pub fn total_subscriber_count(&self) -> usize {
        self.broadcasters.iter().map(|b| b.subscriber_count()).sum()
    }

    /// Get all active stream IDs.
    pub fn stream_ids(&self) -> Vec<StreamId> {
        self.broadcasters.iter().map(|b| b.key().clone()).collect()
    }

    /// Get metrics registry.
    pub fn metrics(&self) -> Arc<MetricsRegistry> {
        self.metrics.clone()
    }
}

impl Default for FragmentBroadcasterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Ingest stats for a stream.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StreamIngestStats {
    pub stream_id: StreamId,
    pub fragment_count: u64,
    pub subscriber_count: usize,
    pub latest_fragment_sequence: Option<u64>,
    pub bitrate_bps: u64,
    pub uptime: Duration,
}

/// Get ingest stats for all streams.
pub fn get_all_ingest_stats(registry: &FragmentBroadcasterRegistry) -> Vec<StreamIngestStats> {
    registry
        .broadcasters
        .iter()
        .map(|entry| {
            let broadcaster = entry.value();
            let history = broadcaster.history();
            let latest = history.last().map(|f| f.meta.sequence);
            let fragment_count = history.len() as u64;

            // Calculate bitrate (simplified)
            let bitrate = if fragment_count > 1 {
                let first = &history[0];
                let last = &history.last().unwrap();
                let duration =
                    last.meta.timestamp.as_secs_f64() - first.meta.timestamp.as_secs_f64();
                let total_bytes: usize = history.iter().map(|f| f.payload.len()).sum();
                if duration > 0.0 {
                    ((total_bytes as f64 * 8.0) / duration) as u64
                } else {
                    0
                }
            } else {
                0
            };

            StreamIngestStats {
                stream_id: entry.key().clone(),
                fragment_count,
                subscriber_count: broadcaster.subscriber_count(),
                latest_fragment_sequence: latest,
                bitrate_bps: bitrate,
                uptime: history
                    .first()
                    .map(|f| f.meta.ingest_time.elapsed())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    fn create_test_fragment(stream_id: &str, seq: u64, is_keyframe: bool) -> Fragment {
        Fragment {
            meta: FragmentMeta {
                stream_id: StreamId::new(stream_id),
                sequence: seq,
                timestamp: Duration::from_millis(seq * 100),
                duration: Duration::from_millis(100),
                is_keyframe,
                size: 1024,
                ingest_time: Instant::now(),
            },
            payload: Bytes::from(vec![0u8; 1024]),
            siblings: HashMap::new(),
        }
    }

    #[test]
    fn test_fragment_registry_basic() {
        let registry = FragmentBroadcasterRegistry::new();
        let stream_id = StreamId::new("test-stream");

        // Ingest some fragments
        for i in 0..10 {
            registry.ingest(
                stream_id.clone(),
                create_test_fragment("test-stream", i, i % 5 == 0),
            );
        }

        // Subscribe
        let mut subscriber = registry.subscribe(stream_id.clone()).unwrap();

        // Should receive latest fragment
        let fragment = subscriber.try_next();
        assert!(fragment.is_some());
        assert_eq!(fragment.unwrap().meta.sequence, 9); // Latest
    }

    #[test]
    fn test_fragment_registry_history() {
        let registry = FragmentBroadcasterRegistry::with_config(5, 100);
        let stream_id = StreamId::new("test-stream");

        // Ingest 10 fragments (max history is 5)
        for i in 0..10 {
            registry.ingest(
                stream_id.clone(),
                create_test_fragment("test-stream", i, false),
            );
        }

        let broadcaster = registry.get(&stream_id).unwrap();
        let history = broadcaster.history();
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].meta.sequence, 5); // Oldest kept
        assert_eq!(history[4].meta.sequence, 9); // Latest
    }

    #[test]
    fn test_multiple_subscribers() {
        let registry = FragmentBroadcasterRegistry::new();
        let stream_id = StreamId::new("test-stream");

        registry.ingest(
            stream_id.clone(),
            create_test_fragment("test-stream", 0, true),
        );

        let mut sub1 = registry.subscribe(stream_id.clone()).unwrap();
        let mut sub2 = registry.subscribe(stream_id.clone()).unwrap();

        assert_eq!(sub1.try_next().unwrap().meta.sequence, 0);
        assert_eq!(sub2.try_next().unwrap().meta.sequence, 0);

        // New fragment should go to both
        registry.ingest(
            stream_id.clone(),
            create_test_fragment("test-stream", 1, false),
        );

        assert_eq!(sub1.try_next().unwrap().meta.sequence, 1);
        assert_eq!(sub2.try_next().unwrap().meta.sequence, 1);
    }

    #[test]
    fn test_stream_removal() {
        let registry = FragmentBroadcasterRegistry::new();
        let stream_id = StreamId::new("test-stream");

        registry.ingest(
            stream_id.clone(),
            create_test_fragment("test-stream", 0, true),
        );
        assert!(registry.get(&stream_id).is_some());

        registry.remove(&stream_id);
        assert!(registry.get(&stream_id).is_none());
    }

    #[test]
    fn test_sibling_tracks() {
        let mut fragment = create_test_fragment("test-stream", 0, true);
        fragment
            .siblings
            .insert("captions".to_string(), Bytes::from("caption data"));
        fragment
            .siblings
            .insert("scte35".to_string(), Bytes::from("scte35 data"));
        fragment
            .siblings
            .insert("timing".to_string(), Bytes::from("timing data"));

        let registry = FragmentBroadcasterRegistry::new();
        let stream_id = StreamId::new("test-stream");
        registry.ingest(stream_id.clone(), fragment);

        let mut subscriber = registry.subscribe(stream_id).unwrap();
        let received = subscriber.try_next().unwrap();

        assert!(received.siblings.contains_key("captions"));
        assert!(received.siblings.contains_key("scte35"));
        assert!(received.siblings.contains_key("timing"));
    }
}
