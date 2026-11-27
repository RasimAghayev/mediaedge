//! FAST Channel Scheduler
//!
//! Linear scheduler with EPG generation, SCTE-35 ad breaks,
//! AI-assisted scheduling (Amazon Bedrock AgentCore).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, NaiveTime, TimeZone, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// FAST channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastChannel {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub status: FastChannelStatus,
    pub schedule: ChannelSchedule,
    pub ad_breaks: AdBreakTemplate,
    pub drm_policy: Option<String>,
    pub output_formats: Vec<OutputFormat>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// FAST channel status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FastChannelStatus {
    Draft,
    Active,
    Paused,
    Archived,
}

/// Channel schedule (recurring programming blocks).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSchedule {
    pub timezone: String,
    pub blocks: Vec<ScheduleBlock>,
    pub default_block: Option<ScheduleBlock>,
}

/// Schedule block (recurring time slot).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleBlock {
    pub id: Uuid,
    pub name: String,
    pub days_of_week: Vec<u8>, // 0=Sunday, 6=Saturday
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub content_type: ScheduleContentType,
    pub content_ids: Vec<Uuid>, // VOD assets, live streams
    pub playout_mode: PlayoutMode,
    pub priority: i32,
}

/// Schedule content type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleContentType {
    Vod,
    Live,
    Slate,
    Filler,
}

/// Playout mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayoutMode {
    Sequential,     // Play in order
    Random,         // Shuffle
    Loop,           // Loop single item
    Dynamic,        // AI-selected
}

/// Ad break template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBreakTemplate {
    pub enabled: bool,
    pub frequency_minutes: u32,
    pub duration_seconds: u32,
    pub scte35_enabled: bool,
    pub ad_server: AdServerConfig,
    pub pre_roll: Option<AdBreakConfig>,
    pub mid_roll: Option<AdBreakConfig>,
    pub post_roll: Option<AdBreakConfig>,
}

/// Ad break configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdBreakConfig {
    pub duration_seconds: u32,
    pub max_ads: u32,
    pub targeting: AdTargeting,
}

/// Ad targeting parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdTargeting {
    pub content_genre: Option<String>,
    pub content_rating: Option<String>,
    pub viewer_demographics: HashMap<String, String>,
    pub custom: HashMap<String, String>,
}

/// Ad server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdServerConfig {
    pub provider: AdServerProvider,
    pub endpoint: String,
    pub api_key: String,
    pub zone_id: Option<String>,
    pub vast_version: VastVersion,
}

/// Ad server providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdServerProvider {
    GoogleAdManager,
    Yospace,
    Broadpeak,
    AwsMediaTailor,
    Prebid,
    Custom,
}

/// VAST version.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VastVersion {
    Vast3,
    Vast4,
}

/// Output format for channel.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Hls,
    Dash,
    HlsFmp4,
    Moq,
}

/// EPG entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgEntry {
    pub channel_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub title: String,
    pub description: Option<String>,
    pub content_id: Option<Uuid>,
    pub content_type: ScheduleContentType,
    pub episode_number: Option<u32>,
    pub season_number: Option<u32>,
    pub genre: Option<String>,
    pub rating: Option<String>,
    pub is_live: bool,
    pub ad_breaks: Vec<EpgAdBreak>,
}

/// EPG ad break.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpgAdBreak {
    pub start_offset_seconds: u32,
    pub duration_seconds: u32,
    pub scte35_signal: Option<String>,
}

/// SCTE-35 ad break signal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scte35Signal {
    pub splice_insert: SpliceInsert,
    pub segmentation_descriptor: Option<SegmentationDescriptor>,
}

/// Splice insert command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpliceInsert {
    pub splice_event_id: u32,
    pub splice_event_cancel_indicator: bool,
    pub out_of_network_indicator: bool,
    pub program_splice_flag: bool,
    pub duration_flag: bool,
    pub splice_immediate_flag: bool,
    pub break_duration: Option<u64>, // 90kHz ticks
    pub unique_program_id: u16,
    pub avail_num: u8,
    pub avails_expected: u8,
}

/// Segmentation descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationDescriptor {
    pub segmentation_event_id: u32,
    pub segmentation_type_id: u8,
    pub segment_num: u8,
    pub segments_expected: u8,
    pub sub_segment_num: u8,
    pub sub_segments_expected: u8,
}

/// FAST scheduler service.
pub struct FastScheduler {
    channels: Arc<DashMap<Uuid, FastChannel>>,
    epg_cache: Arc<DashMap<Uuid, Vec<EpgEntry>>>, // channel_id -> EPG entries
    config: FastSchedulerConfig,
}

#[derive(Debug, Clone)]
pub struct FastSchedulerConfig {
    pub epg_lookahead_hours: u32,
    pub epg_refresh_interval: Duration,
    pub ai_scheduling_enabled: bool,
    pub ai_scheduling_endpoint: Option<String>,
}

impl Default for FastSchedulerConfig {
    fn default() -> Self {
        Self {
            epg_lookahead_hours: 168, // 7 days
            epg_refresh_interval: Duration::from_secs(3600), // 1 hour
            ai_scheduling_enabled: false,
            ai_scheduling_endpoint: None,
        }
    }
}

impl FastScheduler {
    /// Create a new FAST scheduler.
    pub fn new(config: FastSchedulerConfig) -> Arc<Self> {
        Arc::new(Self {
            channels: Arc::new(DashMap::new()),
            epg_cache: Arc::new(DashMap::new()),
            config,
        })
    }

    /// Create a new FAST channel.
    pub async fn create_channel(&self, mut channel: FastChannel) -> Result<FastChannel> {
        channel.id = Uuid::new_v4();
        channel.created_at = Utc::now();
        channel.updated_at = Utc::now();

        // Generate initial EPG
        let epg = self.generate_epg(&channel).await?;
        self.epg_cache.insert(channel.id, epg);

        self.channels.insert(channel.id, channel.clone());
        Ok(channel)
    }

    /// Update a FAST channel.
    pub async fn update_channel(&self, channel_id: Uuid, updates: FastChannelUpdate) -> Result<FastChannel> {
        let mut channel = self.channels.get_mut(&channel_id)
            .ok_or_else(|| anyhow::anyhow!("Channel not found"))?;

        if let Some(name) = updates.name {
            channel.name = name;
        }
        if let Some(description) = updates.description {
            channel.description = Some(description);
        }
        if let Some(logo_url) = updates.logo_url {
            channel.logo_url = Some(logo_url);
        }
        if let Some(status) = updates.status {
            channel.status = status;
        }
        if let Some(schedule) = updates.schedule {
            channel.schedule = schedule;
        }
        if let Some(ad_breaks) = updates.ad_breaks {
            channel.ad_breaks = ad_breaks;
        }
        if let Some(output_formats) = updates.output_formats {
            channel.output_formats = output_formats;
        }

        channel.updated_at = Utc::now();

        // Regenerate EPG
        let epg = self.generate_epg(&channel).await?;
        self.epg_cache.insert(channel.id, epg);

        Ok(channel.clone())
    }

    /// Get a FAST channel.
    pub fn get_channel(&self, channel_id: Uuid) -> Option<FastChannel> {
        self.channels.get(&channel_id).map(|c| c.clone())
    }

    /// List FAST channels for a tenant.
    pub fn list_channels(&self, tenant_id: Uuid) -> Vec<FastChannel> {
        self.channels.iter()
            .filter(|c| c.tenant_id == tenant_id)
            .map(|c| c.clone())
            .collect()
    }

    /// Delete a FAST channel.
    pub async fn delete_channel(&self, channel_id: Uuid) -> Result<()> {
        self.channels.remove(&channel_id);
        self.epg_cache.remove(&channel_id);
        Ok(())
    }

    /// Generate EPG for a channel.
    pub async fn generate_epg(&self, channel: &FastChannel) -> Result<Vec<EpgEntry>> {
        let mut epg = Vec::new();
        let tz = channel.schedule.timezone.parse::<chrono_tz::Tz>()
            .unwrap_or(chrono_tz::UTC);

        let now = Utc::now();
        let lookahead = ChronoDuration::hours(self.config.epg_lookahead_hours as i64);
        let end_time = now + lookahead;

        // Generate entries for each day in range
        let mut current_day = now.date_naive();
        let end_day = end_time.date_naive();

        while current_day <= end_day {
            let weekday = current_day.weekday().number_from_sunday() as u8; // 0=Sunday

            // Find matching schedule blocks
            for block in &channel.schedule.blocks {
                if block.days_of_week.contains(&weekday) {
                    let start_dt = tz.from_local_datetime(&current_day.and_time(block.start_time))
                        .single()
                        .unwrap_or_else(|| Utc.from_utc_datetime(&current_day.and_time(block.start_time)));

                    let end_dt = tz.from_local_datetime(&current_day.and_time(block.end_time))
                        .single()
                        .unwrap_or_else(|| Utc.from_utc_datetime(&current_day.and_time(block.end_time)));

                    if start_dt >= now && start_dt <= end_time {
                        let entries = self.generate_block_entries(&channel, &block, start_dt, end_dt).await?;
                        epg.extend(entries);
                    }
                }
            }

            // Handle default block (filler)
            if let Some(default) = &channel.schedule.default_block {
                if !block_covers_time(&channel.schedule.blocks, weekday, default.start_time) {
                    let start_dt = tz.from_local_datetime(&current_day.and_time(default.start_time))
                        .single()
                        .unwrap_or_else(|| Utc.from_utc_datetime(&current_day.and_time(default.start_time)));

                    let end_dt = tz.from_local_datetime(&current_day.and_time(default.end_time))
                        .single()
                        .unwrap_or_else(|| Utc.from_utc_datetime(&current_day.and_time(default.end_time)));

                    if start_dt >= now && start_dt <= end_time {
                        let entries = self.generate_block_entries(&channel, default, start_dt, end_dt).await?;
                        epg.extend(entries);
                    }
                }
            }

            current_day = current_day.succ_opt().unwrap();
        }

        // Sort by start time
        epg.sort_by_key(|e| e.start_time);

        Ok(epg)
    }

    async fn generate_block_entries(
        &self,
        channel: &FastChannel,
        block: &ScheduleBlock,
        block_start: DateTime<Utc>,
        block_end: DateTime<Utc>,
    ) -> Result<Vec<EpgEntry>> {
        let mut entries = Vec::new();
        let mut current_time = block_start;
        let block_duration = block_end - block_start;

        match block.playout_mode {
            PlayoutMode::Sequential | PlayoutMode::Loop => {
                if block.content_ids.is_empty() {
                    // Slate/filler
                    entries.push(EpgEntry {
                        channel_id: channel.id,
                        start_time: current_time,
                        end_time: block_end,
                        title: block.name.clone(),
                        description: None,
                        content_id: None,
                        content_type: ScheduleContentType::Slate,
                        episode_number: None,
                        season_number: None,
                        genre: None,
                        rating: None,
                        is_live: false,
                        ad_breaks: self.generate_ad_breaks(channel, current_time, block_duration).await,
                    });
                } else {
                    let mut content_idx = 0;
                    while current_time < block_end {
                        let content_id = block.content_ids[content_idx % block.content_ids.len()];
                        // In production, fetch actual content duration
                        let content_duration = ChronoDuration::minutes(30); // Placeholder
                        let entry_end = (current_time + content_duration).min(block_end);

                        entries.push(EpgEntry {
                            channel_id: channel.id,
                            start_time: current_time,
                            end_time: entry_end,
                            title: format!("{} - Content {}", block.name, content_idx + 1),
                            description: None,
                            content_id: Some(content_id),
                            content_type: block.content_type,
                            episode_number: None,
                            season_number: None,
                            genre: None,
                            rating: None,
                            is_live: block.content_type == ScheduleContentType::Live,
                            ad_breaks: self.generate_ad_breaks(channel, current_time, entry_end - current_time).await,
                        });

                        current_time = entry_end;
                        content_idx += 1;

                        if block.playout_mode == PlayoutMode::Loop && content_idx >= block.content_ids.len() {
                            content_idx = 0;
                        }
                    }
                }
            }
            PlayoutMode::Random => {
                // Shuffle content
                let mut shuffled = block.content_ids.clone();
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                shuffled.shuffle(&mut rng);

                for (idx, content_id) in shuffled.iter().enumerate() {
                    if current_time >= block_end { break; }
                    let content_duration = ChronoDuration::minutes(30);
                    let entry_end = (current_time + content_duration).min(block_end);

                    entries.push(EpgEntry {
                        channel_id: channel.id,
                        start_time: current_time,
                        end_time: entry_end,
                        title: format!("{} - Content {}", block.name, idx + 1),
                        description: None,
                        content_id: Some(*content_id),
                        content_type: block.content_type,
                        episode_number: None,
                        season_number: None,
                        genre: None,
                        rating: None,
                        is_live: false,
                        ad_breaks: self.generate_ad_breaks(channel, current_time, entry_end - current_time).await,
                    });

                    current_time = entry_end;
                }
            }
            PlayoutMode::Dynamic => {
                // AI-assisted scheduling
                if self.config.ai_scheduling_enabled {
                    entries.extend(self.ai_generate_schedule(channel, block, block_start, block_end).await?);
                } else {
                    // Fallback to sequential
                    entries.extend(self.generate_block_entries(channel, block, block_start, block_end).await?);
                }
            }
        }

        Ok(entries)
    }

    /// Generate ad breaks for a time range.
    async fn generate_ad_breaks(&self, channel: &FastChannel, start: DateTime<Utc>, duration: ChronoDuration) -> Vec<EpgAdBreak> {
        let mut breaks = Vec::new();

        if !channel.ad_breaks.enabled {
            return breaks;
        }

        let frequency = ChronoDuration::minutes(channel.ad_breaks.frequency_minutes as i64);
        let ad_duration = ChronoDuration::seconds(channel.ad_breaks.duration_seconds as i64);

        let mut break_time = start + frequency;
        while break_time < start + duration - ad_duration {
            breaks.push(EpgAdBreak {
                start_offset_seconds: (break_time - start).num_seconds() as u32,
                duration_seconds: channel.ad_breaks.duration_seconds,
                scte35_signal: if channel.ad_breaks.scte35_enabled {
                    Some(self.generate_scte35_signal(channel, break_time, ad_duration).await)
                } else {
                    None
                },
            });
            break_time += frequency;
        }

        breaks
    }

    /// Generate SCTE-35 signal for ad break.
    async fn generate_scte35_signal(&self, channel: &FastChannel, break_time: DateTime<Utc>, duration: ChronoDuration) -> String {
        // Generate SCTE-35 splice_insert command
        let signal = Scte35Signal {
            splice_insert: SpliceInsert {
                splice_event_id: rand::random(),
                splice_event_cancel_indicator: false,
                out_of_network_indicator: true,
                program_splice_flag: true,
                duration_flag: true,
                splice_immediate_flag: false,
                break_duration: Some((duration.num_milliseconds() * 90) as u64), // Convert to 90kHz ticks
                unique_program_id: 1,
                avail_num: 1,
                avails_expected: 1,
            },
            segmentation_descriptor: Some(SegmentationDescriptor {
                segmentation_event_id: rand::random(),
                segmentation_type_id: 0x30, // Provider advertisement
                segment_num: 1,
                segments_expected: 1,
                sub_segment_num: 0,
                sub_segments_expected: 0,
            }),
        };

        // Encode to base64 (simplified)
        base64::encode(serde_json::to_vec(&signal).unwrap_or_default())
    }

    /// AI-assisted schedule generation.
    async fn ai_generate_schedule(
        &self,
        channel: &FastChannel,
        block: &ScheduleBlock,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<EpgEntry>> {
        // Call AI scheduling service (Amazon Bedrock AgentCore)
        // For now, return empty - would integrate with external AI service
        Ok(Vec::new())
    }

    /// Get EPG for a channel (cached).
    pub async fn get_epg(&self, channel_id: Uuid) -> Result<Vec<EpgEntry>> {
        if let Some(epg) = self.epg_cache.get(&channel_id) {
            // Filter to current/future
            let now = Utc::now();
            let filtered: Vec<EpgEntry> = epg.iter()
                .filter(|e| e.end_time > now)
                .cloned()
                .collect();
            return Ok(filtered);
        }

        // Generate if not cached
        let channel = self.get_channel(channel_id)
            .ok_or_else(|| anyhow::anyhow!("Channel not found"))?;
        let epg = self.generate_epg(&channel).await?;
        self.epg_cache.insert(channel_id, epg.clone());
        Ok(epg)
    }

    /// Get current/now playing for a channel.
    pub async fn get_now_playing(&self, channel_id: Uuid) -> Result<Option<EpgEntry>> {
        let epg = self.get_epg(channel_id).await?;
        let now = Utc::now();
        Ok(epg.into_iter().find(|e| e.start_time <= now && e.end_time > now))
    }

    /// Get upcoming schedule for a channel.
    pub async fn get_upcoming(&self, channel_id: Uuid, hours: u32) -> Result<Vec<EpgEntry>> {
        let epg = self.get_epg(channel_id).await?;
        let now = Utc::now();
        let end = now + ChronoDuration::hours(hours as i64);
        Ok(epg.into_iter()
            .filter(|e| e.start_time >= now && e.start_time <= end)
            .collect())
    }
}

/// FastChannel update struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FastChannelUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub status: Option<FastChannelStatus>,
    pub schedule: Option<ChannelSchedule>,
    pub ad_breaks: Option<AdBreakTemplate>,
    pub output_formats: Option<Vec<OutputFormat>>,
}

fn block_covers_time(blocks: &[ScheduleBlock], weekday: u8, time: NaiveTime) -> bool {
    for block in blocks {
        if block.days_of_week.contains(&weekday) {
            if block.start_time <= time && time < block.end_time {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_channel_status() {
        assert_eq!(FastChannelStatus::Active as i32, 1);
        assert_eq!(FastChannelStatus::Draft as i32, 0);
    }

    #[test]
    fn test_schedule_block() {
        let block = ScheduleBlock {
            id: Uuid::new_v4(),
            name: "Prime Time".to_string(),
            days_of_week: vec![1,2,3,4,5], // Mon-Fri
            start_time: NaiveTime::from_hms_opt(19, 0, 0).unwrap(),
            end_time: NaiveTime::from_hms_opt(23, 0, 0).unwrap(),
            content_type: ScheduleContentType::Vod,
            content_ids: vec![Uuid::new_v4()],
            playout_mode: PlayoutMode::Sequential,
            priority: 10,
        };

        assert!(block.days_of_week.contains(&2)); // Tuesday
        assert!(!block.days_of_week.contains(&0)); // Sunday
    }

    #[test]
    fn test_ad_break_template() {
        let template = AdBreakTemplate {
            enabled: true,
            frequency_minutes: 15,
            duration_seconds: 120,
            scte35_enabled: true,
            ad_server: AdServerConfig {
                provider: AdServerProvider::GoogleAdManager,
                endpoint: "https://admanager.google.com".to_string(),
                api_key: "test".to_string(),
                zone_id: Some("12345".to_string()),
                vast_version: VastVersion::Vast4,
            },
            pre_roll: None,
            mid_roll: None,
            post_roll: None,
        };

        assert!(template.enabled);
        assert_eq!(template.frequency_minutes, 15);
    }
}