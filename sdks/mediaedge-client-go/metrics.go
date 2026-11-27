package mediaedge

import (
	"context"
	"fmt"
	"net/url"
)

// MetricsService handles metrics-related API calls
type MetricsService struct {
	client *Client
}

// Metrics returns the metrics service
func (c *Client) Metrics() *MetricsService {
	return &MetricsService{client: c}
}

// QueryMetrics queries Prometheus metrics
func (s *MetricsService) QueryMetrics(ctx context.Context, query string, time *time.Time) (map[string]any, error) {
	params := url.Values{}
	params.Set("query", query)
	if time != nil {
		params.Set("time", time.Format(time.RFC3339))
	}

	var result map[string]any
	err := s.client.GET(ctx, "/api/v1/metrics/query", params, &result)
	return result, err
}

// MetricsQuery represents a metrics range query
type MetricsQuery struct {
	Metric string            `json:"metric"`
	Start  time.Time        `json:"start"`
	End    time.Time        `json:"end"`
	Step   string           `json:"step,omitempty"`
	Labels map[string]any   `json:"labels,omitempty"`
}

// MetricsResult represents a metrics range query result
type MetricsResult struct {
	Metric string            `json:"metric"`
	Data   []MetricsDataPoint `json:"data"`
}

// MetricsDataPoint represents a metrics data point
type MetricsDataPoint struct {
	Timestamp time.Time       `json:"timestamp"`
	Value     float64         `json:"value"`
	Labels    map[string]any  `json:"labels"`
}

// QueryRange queries metrics range
func (s *MetricsService) QueryRange(ctx context.Context, req MetricsQuery) (*MetricsResult, error) {
	params := url.Values{}
	params.Set("metric", req.Metric)
	params.Set("start", req.Start.Format(time.RFC3339))
	params.Set("end", req.End.Format(time.RFC3339))
	if req.Step != "" {
		params.Set("step", req.Step)
	}
	if req.Labels != nil {
		labelsJSON, _ := json.Marshal(req.Labels)
		params.Set("labels", string(labelsJSON))
	}

	var result MetricsResult
	err := s.client.GET(ctx, "/api/v1/metrics/query_range", params, &result)
	return &result, err
}

// StreamMetrics gets stream metrics
func (s *MetricsService) StreamMetrics(ctx context.Context, streamID uuid.UUID, start, end *time.Time, interval string) ([]StreamMetrics, error) {
	params := url.Values{}
	if start != nil {
		params.Set("start", start.Format(time.RFC3339))
	}
	if end != nil {
		params.Set("end", end.Format(time.RFC3339))
	}
	if interval != "" {
		params.Set("interval", interval)
	}

	var result []StreamMetrics
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/streams/%s/metrics", streamID), params, &result)
	return result, err
}

// ClusterMetrics represents cluster metrics
type ClusterMetrics struct {
	TotalStreams       int     `json:"total_streams"`
	ActiveStreams      int     `json:"active_streams"`
	TotalViewers       int64   `json:"total_viewers"`
	TotalBandwidthMbps float64 `json:"total_bandwidth_mbps"`
	TotalStorageGB     float64 `json:"total_storage_gb"`
	TranscodingHours   float64 `json:"transcoding_hours"`
	NodesHealthy       int     `json:"nodes_healthy"`
	NodesTotal         int     `json:"nodes_total"`
}

// ClusterMetrics gets cluster metrics
func (s *MetricsService) ClusterMetrics(ctx context.Context, start, end *time.Time, interval string) (*ClusterMetrics, error) {
	params := url.Values{}
	if start != nil {
		params.Set("start", start.Format(time.RFC3339))
	}
	if end != nil {
		params.Set("end", end.Format(time.RFC3339))
	}
	if interval != "" {
		params.Set("interval", interval)
	}

	var result ClusterMetrics
	err := s.client.GET(ctx, "/api/v1/cluster/metrics", params, &result)
	return &result, err
}

// TenantMetrics represents tenant metrics
type TenantMetrics struct {
	TenantID           uuid.UUID `json:"tenant_id"`
	Streams            int       `json:"streams"`
	Viewers            int64     `json:"viewers"`
	BandwidthMbps      float64   `json:"bandwidth_mbps"`
	StorageGB          float64   `json:"storage_gb"`
	TranscodingHours   float64   `json:"transcoding_hours"`
	RevenueCents       int64     `json:"revenue_cents"`
}

// TenantMetrics gets tenant metrics
func (s *MetricsService) TenantMetrics(ctx context.Context, tenantID uuid.UUID, start, end *time.Time, interval string) (*TenantMetrics, error) {
	params := url.Values{}
	if start != nil {
		params.Set("start", start.Format(time.RFC3339))
	}
	if end != nil {
		params.Set("end", end.Format(time.RFC3339))
	}
	if interval != "" {
		params.Set("interval", interval)
	}

	var result TenantMetrics
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/tenants/%s/metrics", tenantID), params, &result)
	return &result, err
}

// AnomalyAlert represents an anomaly alert
type AnomalyAlert struct {
	ID           uuid.UUID `json:"id"`
	StreamID     *uuid.UUID `json:"stream_id,omitempty"`
	TenantID     uuid.UUID `json:"tenant_id"`
	AnomalyType  string    `json:"anomaly_type"`
	Severity     string    `json:"severity"`
	Message      string    `json:"message"`
	MetricValue  float64   `json:"metric_value"`
	Threshold    float64   `json:"threshold"`
	DetectedAt   time.Time `json:"detected_at"`
	Acknowledged bool      `json:"acknowledged"`
}

// Anomalies lists anomaly alerts
func (s *MetricsService) Anomalies(ctx context.Context, params ListParams) (*PaginatedResponse[AnomalyAlert], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[AnomalyAlert]
	err := s.client.GET(ctx, "/api/v1/observability/anomalies", query, &result)
	return &result, err
}