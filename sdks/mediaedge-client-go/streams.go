package mediaedge

import (
	"context"
	"net/url"
	"time"
)

// StreamsService handles stream-related API calls
type StreamsService struct {
	client *Client
}

// Streams returns the streams service
func (c *Client) Streams() *StreamsService {
	return &StreamsService{client: c}
}

// List lists streams with pagination
func (s *StreamsService) List(ctx context.Context, params ListParams) (*PaginatedResponse[Stream], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}
	if params.Search != "" {
		query.Set("search", params.Search)
	}
	if params.Sort != "" {
		query.Set("sort", params.Sort)
	}
	if params.Order != "" {
		query.Set("order", params.Order)
	}

	var result PaginatedResponse[Stream]
	err := s.client.GET(ctx, "/api/v1/streams", query, &result)
	return &result, err
}

// Get gets a stream by ID
func (s *StreamsService) Get(ctx context.Context, id uuid.UUID) (*Stream, error) {
	var result Stream
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/streams/%s", id), nil, &result)
	return &result, err
}

// Create creates a new stream
func (s *StreamsService) Create(ctx context.Context, req CreateStreamRequest) (*Stream, error) {
	var result Stream
	err := s.client.POST(ctx, "/api/v1/streams", req, &result)
	return &result, err
}

// Update updates a stream
func (s *StreamsService) Update(ctx context.Context, id uuid.UUID, req UpdateStreamRequest) (*Stream, error) {
	var result Stream
	err := s.client.PATCH(ctx, fmt.Sprintf("/api/v1/streams/%s", id), req, &result)
	return &result, err
}

// Delete deletes a stream
func (s *StreamsService) Delete(ctx context.Context, id uuid.UUID) error {
	return s.client.DELETE(ctx, fmt.Sprintf("/api/v1/streams/%s", id), nil)
}

// Start starts a stream
func (s *StreamsService) Start(ctx context.Context, id uuid.UUID) (*Stream, error) {
	var result Stream
	err := s.client.POST(ctx, fmt.Sprintf("/api/v1/streams/%s/start", id), nil, &result)
	return &result, err
}

// Stop stops a stream
func (s *StreamsService) Stop(ctx context.Context, id uuid.UUID) (*Stream, error) {
	var result Stream
	err := s.client.POST(ctx, fmt.Sprintf("/api/v1/streams/%s/stop", id), nil, &result)
	return &result, err
}

// Metrics gets stream metrics
func (s *StreamsService) Metrics(ctx context.Context, id uuid.UUID, start, end *time.Time, interval string) ([]StreamMetrics, error) {
	query := url.Values{}
	if start != nil {
		query.Set("start", start.Format(time.RFC3339))
	}
	if end != nil {
		query.Set("end", end.Format(time.RFC3339))
	}
	if interval != "" {
		query.Set("interval", interval)
	}

	var result []StreamMetrics
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/streams/%s/metrics", id), query, &result)
	return result, err
}

// PlaybackURLs gets stream playback URLs
func (s *StreamsService) PlaybackURLs(ctx context.Context, id uuid.UUID) (*PlaybackUrls, error) {
	var result PlaybackUrls
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/streams/%s/playback", id), nil, &result)
	return &result, err
}

// IngestURL gets stream ingest URL
func (s *StreamsService) IngestURL(ctx context.Context, id uuid.UUID) (string, error) {
	var result struct {
		URL string `json:"url"`
	}
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/streams/%s/ingest", id), nil, &result)
	return result.URL, err
}