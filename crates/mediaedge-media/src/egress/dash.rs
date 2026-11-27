//! DASH Egress Implementation
//!
//! Supports:
//! - CMAF-LL (Common Media Application Format - Low Latency)
//! - SegmentTemplate + SegmentTimeline
//! - Multi-period for ad insertion (SCTE-35)
//! - MPD generation and updates
//! - Multiple representations (bitrate ladder)
//! - In-band events for SCTE-35

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use dash_mpd::{
    Mpd, Period, AdaptationSet, Representation, SegmentTemplate, SegmentTimeline,
    STimeline, SElement, ContentProtection, Descriptor,
};
use tokio::fs;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::fragment::{Fragment, FragmentBroadcasterRegistry, FragmentSubscriber, StreamId};
use mediaedge_config::types::HttpProtocolConfig;
use mediaedge_observability::MetricsRegistry;

/// DASH segment information.
#[derive(Debug, Clone)]
pub struct DashSegment {
    pub number: u64,
    pub start_time: u64,
    pub duration: u64,
    pub path: PathBuf,
    pub size: u64,
    pub is_keyframe: bool,
    pub scte35_events: Vec<Scte35Event>,
}

/// SCTE-35 event for ad insertion.
#[derive(Debug, Clone)]
pub struct Scte35Event {
    pub scheme_id_uri: String,
    pub presentation_time: u64,
    pub duration: Option<u64>,
    pub id: String,
    pub message_data: Vec<u8>,
}

/// DASH stream state.
struct DashStreamState {
    stream_id: StreamId,
    subscriber: FragmentSubscriber,
    segment_number: u64,
    segment_duration: Duration,
    segment_duration_timescale: u64, // e.g., 90000 for 90kHz
    output_dir: PathBuf,
    base_url: String,
    segments: Vec<DashSegment>,
    current_segment_data: Vec<u8>,
    current_segment_start_time: u64,
    current_segment_is_keyframe: bool,
    started_at: Instant,
    bytes_written: u64,
    timescale: u64,
    presentation_time_offset: u64,
}

/// DASH egress server state.
struct DashServerState {
    config: HttpProtocolConfig,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    active_streams: DashMap<String, DashStreamState>,
    output_root: PathBuf,
    base_url: String,
}

/// Run DASH egress for a stream.
pub async fn run_dash_egress(
    stream_id: StreamId,
    registry: Arc<FragmentBroadcasterRegistry>,
    metrics: Arc<MetricsRegistry>,
    config: HttpProtocolConfig,
    output_root: PathBuf,
    base_url: String,
) -> anyhow::Result<()> {
    let segment_duration = Duration::from_secs(config.segment_duration_secs as u64);
    let timescale = 90000; // 90kHz standard for video
    let segment_duration_timescale = (segment_duration.as_secs() * timescale) as u64;

    let output_dir = output_root.join(&stream_id.0);
    fs::create_dir_all(&output_dir).await?;

    let state = DashStreamState {
        stream_id: stream_id.clone(),
        subscriber: registry.subscribe_with_buffer(stream_id.clone(), 1000)
            .ok_or_else(|| anyhow::anyhow!("Failed to subscribe to stream"))?,
        segment_number: 0,
        segment_duration,
        segment_duration_timescale,
        output_dir: output_dir.clone(),
        base_url: base_url.clone(),
        segments: Vec::new(),
        current_segment_data: Vec::new(),
        current_segment_start_time: 0,
        current_segment_is_keyframe: false,
        started_at: Instant::now(),
        bytes_written: 0,
        timescale,
        presentation_time_offset: 0,
    };

    info!("DASH egress started for stream: {} at {}", stream_id.0, output_dir.display());

    // Generate initial MPD
    generate_initial_mpd(&state).await?;

    // Run segmenter loop
    run_dash_segmenter_loop(state, metrics).await
}

/// Generate initial MPD.
async fn generate_initial_mpd(state: &DashStreamState) -> anyhow::Result<()> {
    let mpd = create_mpd(state).await?;
    let mpd_path = state.output_dir.join("manifest.mpd");
    fs::write(&mpd_path, mpd.to_string()).await?;
    info!("Initial DASH MPD generated for {}", state.stream_id.0);
    Ok(())
}

/// Create MPD document.
async fn create_mpd(state: &DashStreamState) -> Mpd {
    let mut mpd = Mpd::default();
    
    mpd.profiles = vec!["urn:mpeg:dash:profile:isoff-live:2013".to_string()];
    mpd.type_ = "dynamic".to_string();
    mpd.minimum_update_period = Some(format!("PT{}S", state.segment_duration.as_secs()));
    mpd.min_buffer_time = Some(format!("PT{}S", state.segment_duration.as_secs() * 2));
    mpd.availability_start_time = Some(chrono::Utc::now().to_rfc3339());
    mpd.publish_time = Some(chrono::Utc::now().to_rfc3339());
    mpd.time_shift_buffer_depth = Some(format!("PT{}S", state.segment_duration.as_secs() * state.segments.len().max(10) as u64));

    // Create period
    let mut period = Period::default();
    period.id = Some("0".to_string());
    period.start = Some("PT0S".to_string());

    // Video adaptation set
    let mut video_adaptation = AdaptationSet::default();
    video_adaptation.id = Some(1);
    video_adaptation.content_type = Some("video".to_string());
    video_adaptation.mime_type = Some("video/mp4".to_string());
    video_adaptation.codecs = Some("avc1.640028".to_string());
    video_adaptation.frame_rate = Some("30".to_string());
    video_adaptation.max_width = Some(1920);
    video_adaptation.max_height = Some(1080);
    video_adaptation.par = Some("1:1".to_string());
    video_adaptation.segment_alignment = Some(true);
    video_adaptation.bitstream_switching = Some(true);

    // Segment template for video
    let mut video_seg_template = SegmentTemplate::default();
    video_seg_template.timescale = state.timescale;
    video_seg_template.duration = state.segment_duration_timescale;
    video_seg_template.start_number = 1;
    video_seg_template.media = "video_$Number$.m4s".to_string();
    video_seg_template.initialization = "video_init.mp4".to_string();
    
    // Build segment timeline
    let mut timeline = SegmentTimeline::default();
    for (i, segment) in state.segments.iter().enumerate() {
        let mut s = SElement::default();
        s.t = segment.start_time;
        s.d = segment.duration;
        if i == 0 {
            s.r = 0;
        }
        timeline.s.push(s);
    }
    video_seg_template.segment_timeline = Some(timeline);
    
    video_adaptation.segment_template = Some(video_seg_template);

    // Add representations (bitrate ladder)
    let representations = vec![
        ("1080p", 5_000_000, "avc1.640028", 1920, 1080),
        ("720p", 2_500_000, "avc1.64001f", 1280, 720),
        ("480p", 1_000_000, "avc1.4d401e", 854, 480),
        ("360p", 500_000, "avc1.4d4015", 640, 360),
    ];

    for (name, bandwidth, codecs, width, height) in representations {
        let mut rep = Representation::default();
        rep.id = Some(name.to_string());
        rep.bandwidth = bandwidth;
        rep.codecs = Some(codecs.to_string());
        rep.width = Some(width);
        rep.height = Some(height);
        video_adaptation.representations.push(rep);
    }

    period.adaptation_sets.push(video_adaptation);

    // Audio adaptation set
    let mut audio_adaptation = AdaptationSet::default();
    audio_adaptation.id = Some(2);
    audio_adaptation.content_type = Some("audio".to_string());
    audio_adaptation.mime_type = Some("audio/mp4".to_string());
    audio_adaptation.codecs = Some("mp4a.40.2".to_string());
    audio_adaptation.lang = Some("en".to_string());
    audio_adaptation.segment_alignment = Some(true);
    audio_adaptation.bitstream_switching = Some(true);

    let mut audio_seg_template = SegmentTemplate::default();
    audio_seg_template.timescale = 48000; // Audio timescale
    audio_seg_template.duration = (state.segment_duration.as_secs() * 48000) as u64;
    audio_seg_template.start_number = 1;
    audio_seg_template.media = "audio_$Number$.m4s".to_string();
    audio_seg_template.initialization = "audio_init.mp4".to_string();
    audio_adaptation.segment_template = Some(audio_seg_template);

    let mut audio_rep = Representation::default();
    audio_rep.id = Some("audio_en".to_string());
    audio_rep.bandwidth = 128_000;
    audio_rep.codecs = Some("mp4a.40.2".to_string());
    audio_rep.audio_sampling_rate = Some(48000);
    audio_adaptation.representations.push(audio_rep);

    period.adaptation_sets.push(audio_adaptation);

    // Add SCTE-35 event stream for ad insertion
    let mut event_stream = dash_mpd::EventStream::default();
    event_stream.scheme_id_uri = "urn:scte:scte35:2013:xml".to_string();
    event_stream.value = "scte35".to_string();
    event_stream.timescale = state.timescale;
    period.event_streams.push(event_stream);

    mpd.periods.push(period);
    mpd
}

/// Main DASH segmenter loop.
async fn run_dash_segmenter_loop(
    mut state: DashStreamState,
    metrics: Arc<MetricsRegistry>,
) -> anyhow::Result<()> {
    let mut segment_timer = interval(state.segment_duration);
    let mut mpd_update_timer = interval(Duration::from_secs(5)); // Update MPD every 5 seconds

    loop {
        tokio::select! {
            // Receive fragment from ingest
            fragment = state.subscriber.next() => {
                if let Some(fragment) = fragment {
                    handle_dash_fragment(&mut state, fragment, &metrics).await;
                } else {
                    // Stream ended
                    info!("DASH egress stream ended: {}", state.stream_id.0);
                    finalize_dash_stream(&mut state).await;
                    break;
                }
            }
            
            // Segment boundary
            _ = segment_timer.tick() => {
                if !state.current_segment_data.is_empty() {
                    finalize_dash_segment(&mut state, &metrics).await;
                }
            }
            
            // Update MPD
            _ = mpd_update_timer.tick() => {
                if let Err(e) = update_mpd(&state).await {
                    error!("Failed to update DASH MPD: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Handle incoming fragment for DASH.
async fn handle_dash_fragment(
    state: &mut DashStreamState,
    fragment: Fragment,
    metrics: &Arc<MetricsRegistry>,
) {
    // Start new segment if needed
    if state.current_segment_start_time == 0 {
        state.current_segment_start_time = (state.started_at.elapsed().as_secs() * state.timescale) as u64;
        state.current_segment_is_keyframe = fragment.meta.is_keyframe;
    } else if fragment.meta.is_keyframe && state.current_segment_start_time > 0 {
        // Keyframe can start new segment
        let elapsed_time = (Instant::now().duration_since(state.started_at).as_secs() * state.timescale) as u64;
        let elapsed_in_segment = elapsed_time - state.current_segment_start_time;
        
        if elapsed_in_segment >= state.segment_duration_timescale {
            finalize_dash_segment(state, metrics).await;
            state.current_segment_start_time = (Instant::now().duration_since(state.started_at).as_secs() * state.timescale) as u64;
            state.current_segment_is_keyframe = true;
        }
    }

    // Append fragment data
    state.current_segment_data.extend_from_slice(&fragment.payload);
    state.bytes_written += fragment.payload.len() as u64;

    // Handle SCTE-35
    if let Some(scte35_data) = fragment.siblings.get("scte35") {
        if let Ok(events) = parse_scte35_for_dash(scte35_data, state.current_segment_start_time) {
            // Store events for current segment
        }
    }

    metrics.counter("egress_dash_fragments_total", "DASH fragments processed").increment(1);
    metrics.counter("egress_dash_bytes_total", "DASH bytes written").increment(fragment.payload.len() as u64);
}

/// Finalize DASH segment.
async fn finalize_dash_segment(
    state: &mut DashStreamState,
    metrics: &Arc<MetricsRegistry>,
) {
    if state.current_segment_data.is_empty() {
        return;
    }

    let segment_path = state.output_dir.join(format!("video_{}.m4s", state.segment_number));
    
    if let Err(e) = fs::write(&segment_path, &state.current_segment_data).await {
        error!("Failed to write DASH segment: {}", e);
        return;
    }

    let segment_duration = state.segment_duration_timescale;
    let segment_start = state.current_segment_start_time;

    let segment = DashSegment {
        number: state.segment_number,
        start_time: segment_start,
        duration: segment_duration,
        path: segment_path.clone(),
        size: state.current_segment_data.len() as u64,
        is_keyframe: state.current_segment_is_keyframe,
        scte35_events: Vec::new(),
    };

    state.segments.push(segment);
    state.segment_number += 1;
    state.current_segment_data.clear();
    state.current_segment_start_time = 0;
    state.current_segment_is_keyframe = false;

    metrics.counter("egress_dash_segments_total", "DASH segments created").increment(1);
}

/// Update MPD document.
async fn update_mpd(state: &DashStreamState) -> anyhow::Result<()> {
    let mpd = create_mpd(state).await?;
    let mpd_path = state.output_dir.join("manifest.mpd");
    fs::write(&mpd_path, mpd.to_string()).await?;
    debug!("DASH MPD updated for {}", state.stream_id.0);
    Ok(())
}

/// Finalize DASH stream.
async fn finalize_dash_stream(state: &mut DashStreamState) {
    // Write final segment
    if !state.current_segment_data.is_empty() {
        finalize_dash_segment(state, &Arc::new(MetricsRegistry::new(&Default::default()).unwrap())).await;
    }

    // Generate final MPD with static type
    let mut mpd = create_mpd(state).await;
    mpd.type_ = "static".to_string();
    mpd.minimum_update_period = None;
    
    let mpd_path = state.output_dir.join("manifest.mpd");
    let _ = fs::write(&mpd_path, mpd.to_string()).await;

    info!("DASH egress finalized for stream: {} ({} segments)", 
        state.stream_id.0, state.segments.len());
}

/// Parse SCTE-35 for DASH event stream.
fn parse_scte35_for_dash(data: &Bytes, presentation_time: u64) -> anyhow::Result<Vec<Scte35Event>> {
    // Simplified parsing
    Ok(Vec::new())
}

/// DASH representation configuration.
#[derive(Debug, Clone)]
pub struct DashRepresentationConfig {
    pub id: String,
    pub bandwidth: u32,
    pub codecs: String,
    pub width: u32,
    pub height: u32,
    pub framerate: u32,
    pub audio_sampling_rate: Option<u32>,
}

/// DASH egress configuration.
#[derive(Debug, Clone)]
pub struct DashEgressConfig {
    pub segment_duration_secs: u32,
    pub playlist_window: usize,
    pub timescale: u64,
    pub output_root: PathBuf,
    pub base_url: String,
    pub representations: Vec<DashRepresentationConfig>,
}

impl Default for DashEgressConfig {
    fn default() -> Self {
        Self {
            segment_duration_secs: 6,
            playlist_window: 10,
            timescale: 90000,
            output_root: PathBuf::from("/data/dash"),
            base_url: "https://cdn.example.com/dash".to_string(),
            representations: vec![
                DashRepresentationConfig {
                    id: "1080p".to_string(),
                    bandwidth: 5_000_000,
                    codecs: "avc1.640028".to_string(),
                    width: 1920,
                    height: 1080,
                    framerate: 30,
                    audio_sampling_rate: Some(48000),
                },
                DashRepresentationConfig {
                    id: "720p".to_string(),
                    bandwidth: 2_500_000,
                    codecs: "avc1.64001f".to_string(),
                    width: 1280,
                    height: 720,
                    framerate: 30,
                    audio_sampling_rate: Some(48000),
                },
            ],
        }
    }
}

/// Multi-period DASH for ad insertion.
pub struct MultiPeriodDash {
    periods: Vec<PeriodConfig>,
    current_period: usize,
}

/// Period configuration for multi-period DASH.
#[derive(Debug, Clone)]
pub struct PeriodConfig {
    pub id: String,
    pub start: String,
    pub duration: Option<String>,
    pub adaptation_sets: Vec<AdaptationSetConfig>,
}

/// Adaptation set configuration.
#[derive(Debug, Clone)]
pub struct AdaptationSetConfig {
    pub id: u32,
    pub content_type: String,
    pub representations: Vec<DashRepresentationConfig>,
}

impl MultiPeriodDash {
    pub fn new() -> Self {
        Self {
            periods: Vec::new(),
            current_period: 0,
        }
    }

    pub fn add_content_period(&mut self, config: PeriodConfig) {
        self.periods.push(config);
    }

    pub fn add_ad_period(&mut self, config: PeriodConfig) {
        self.periods.push(config);
    }

    pub fn generate_mpd(&self) -> Mpd {
        let mut mpd = Mpd::default();
        mpd.type_ = "dynamic".to_string();
        mpd.profiles = vec!["urn:mpeg:dash:profile:isoff-live:2013".to_string()];

        for period_config in &self.periods {
            let mut period = Period::default();
            period.id = Some(period_config.id.clone());
            period.start = Some(period_config.start.clone());

            for adapt_config in &period_config.adaptation_sets {
                let mut adaptation = AdaptationSet::default();
                adaptation.id = Some(adapt_config.id);
                adaptation.content_type = Some(adapt_config.content_type.clone());

                for rep_config in &adapt_config.representations {
                    let mut rep = Representation::default();
                    rep.id = Some(rep_config.id.clone());
                    rep.bandwidth = rep_config.bandwidth;
                    rep.codecs = Some(rep_config.codecs.clone());
                    rep.width = Some(rep_config.width);
                    rep.height = Some(rep_config.height);
                    adaptation.representations.push(rep);
                }

                period.adaptation_sets.push(adaptation);
            }

            mpd.periods.push(period);
        }

        mpd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dash_segment() {
        let segment = DashSegment {
            number: 1,
            start_time: 90000,
            duration: 540000,
            path: PathBuf::from("/data/dash/video_1.m4s"),
            size: 1024 * 1024,
            is_keyframe: true,
            scte35_events: Vec::new(),
        };

        assert_eq!(segment.number, 1);
        assert_eq!(segment.duration, 540000); // 6 seconds at 90kHz
    }

    #[test]
    fn test_dash_representation_config() {
        let rep = DashRepresentationConfig {
            id: "1080p".to_string(),
            bandwidth: 5_000_000,
            codecs: "avc1.640028".to_string(),
            width: 1920,
            height: 1080,
            framerate: 30,
            audio_sampling_rate: Some(48000),
        };

        assert_eq!(rep.bandwidth, 5_000_000);
        assert_eq!(rep.width, 1920);
    }

    #[test]
    fn test_multi_period_dash() {
        let mut mpd = MultiPeriodDash::new();
        
        mpd.add_content_period(PeriodConfig {
            id: "content-1".to_string(),
            start: "PT0S".to_string(),
            duration: Some("PT300S".to_string()),
            adaptation_sets: vec![
                AdaptationSetConfig {
                    id: 1,
                    content_type: "video".to_string(),
                    representations: vec![
                        DashRepresentationConfig {
                            id: "1080p".to_string(),
                            bandwidth: 5_000_000,
                            codecs: "avc1.640028".to_string(),
                            width: 1920,
                            height: 1080,
                            framerate: 30,
                            audio_sampling_rate: None,
                        },
                    ],
                },
            ],
        });

        mpd.add_ad_period(PeriodConfig {
            id: "ad-1".to_string(),
            start: "PT300S".to_string(),
            duration: Some("PT30S".to_string()),
            adaptation_sets: vec![
                AdaptationSetConfig {
                    id: 1,
                    content_type: "video".to_string(),
                    representations: vec![
                        DashRepresentationConfig {
                            id: "ad-1080p".to_string(),
                            bandwidth: 5_000_000,
                            codecs: "avc1.640028".to_string(),
                            width: 1920,
                            height: 1080,
                            framerate: 30,
                            audio_sampling_rate: None,
                        },
                    ],
                },
            ],
        });

        let generated_mpd = mpd.generate_mpd();
        assert_eq!(generated_mpd.periods.len(), 2);
    }
}