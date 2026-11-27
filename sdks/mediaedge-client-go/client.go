package mediaedge

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"time"

	"github.com/google/uuid"
)

// Client is the MediaEdge API client
type Client struct {
	baseURL      *url.URL
	httpClient   *http.Client
	accessToken  string
	refreshToken string
}

// Config holds client configuration
type Config struct {
	BaseURL      string
	AccessToken  string
	RefreshToken string
	Timeout      time.Duration
}

// NewClient creates a new MediaEdge client
func NewClient(config Config) (*Client, error) {
	baseURL, err := url.Parse(config.BaseURL)
	if err != nil {
		return nil, fmt.Errorf("invalid base URL: %w", err)
	}

	if baseURL.Scheme == "" {
		baseURL.Scheme = "http"
	}

	if config.Timeout == 0 {
		config.Timeout = 30 * time.Second
	}

	return &Client{
		baseURL: baseURL,
		httpClient: &http.Client{
			Timeout: config.Timeout,
		},
		accessToken:  config.AccessToken,
		refreshToken: config.RefreshToken,
	}, nil
}

// SetAccessToken sets the access token
func (c *Client) SetAccessToken(token string) {
	c.accessToken = token
}

// SetRefreshToken sets the refresh token
func (c *Client) SetRefreshToken(token string) {
	c.refreshToken = token
}

// GetAccessToken returns the current access token
func (c *Client) GetAccessToken() string {
	return c.accessToken
}

// doRequest performs an HTTP request
func (c *Client) doRequest(ctx context.Context, method, path string, body, result interface{}) error {
	var reqBody io.Reader
	if body != nil {
		jsonBody, err := json.Marshal(body)
		if err != nil {
			return fmt.Errorf("marshal request body: %w", err)
		}
		reqBody = bytes.NewReader(jsonBody)
	}

	fullURL := c.baseURL.ResolveReference(&url.URL{Path: path})
	req, err := http.NewRequestWithContext(ctx, method, fullURL.String(), reqBody)
	if err != nil {
		return fmt.Errorf("create request: %w", err)
	}

	req.Header.Set("Content-Type", "application/json")
	if c.accessToken != "" {
		req.Header.Set("Authorization", "Bearer "+c.accessToken)
	}

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("do request: %w", err)
	}
	defer resp.Body.Close()

	respBody, err := io.ReadAll(resp.Body)
	if err != nil {
		return fmt.Errorf("read response: %w", err)
	}

	if resp.StatusCode >= 400 {
		var apiErr APIError
		if err := json.Unmarshal(respBody, &apiErr); err != nil {
			apiErr.Message = string(respBody)
		}
		apiErr.Code = resp.StatusCode
		return &apiErr
	}

	if result != nil && len(respBody) > 0 {
		if err := json.Unmarshal(respBody, result); err != nil {
			return fmt.Errorf("unmarshal response: %w", err)
		}
	}

	return nil
}

// GET performs a GET request
func (c *Client) GET(ctx context.Context, path string, query url.Values, result interface{}) error {
	if query != nil {
		path += "?" + query.Encode()
	}
	return c.doRequest(ctx, http.MethodGet, path, nil, result)
}

// POST performs a POST request
func (c *Client) POST(ctx context.Context, path string, body, result interface{}) error {
	return c.doRequest(ctx, http.MethodPost, path, body, result)
}

// PATCH performs a PATCH request
func (c *Client) PATCH(ctx context.Context, path string, body, result interface{}) error {
	return c.doRequest(ctx, http.MethodPatch, path, body, result)
}

// DELETE performs a DELETE request
func (c *Client) DELETE(ctx context.Context, path string, result interface{}) error {
	return c.doRequest(ctx, http.MethodDelete, path, nil, result)
}

// APIError represents an API error response
type APIError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
	Details any    `json:"details,omitempty"`
}

func (e *APIError) Error() string {
	return fmt.Sprintf("API error %d: %s", e.Code, e.Message)
}

// IsNotFound returns true if the error is a 404
func (e *APIError) IsNotFound() bool {
	return e.Code == 404
}

// IsUnauthorized returns true if the error is a 401
func (e *APIError) IsUnauthorized() bool {
	return e.Code == 401
}

// Common types

// PaginatedResponse represents a paginated API response
type PaginatedResponse[T any] struct {
	Data       []T   `json:"data"`
	Total      int64 `json:"total"`
	Page       int   `json:"page"`
	PerPage    int   `json:"per_page"`
	TotalPages int   `json:"total_pages"`
}

// ListParams represents common list parameters
type ListParams struct {
	Page     int    `json:"page,omitempty"`
	PerPage  int    `json:"per_page,omitempty"`
	Search   string `json:"search,omitempty"`
	Sort     string `json:"sort,omitempty"`
	Order    string `json:"order,omitempty"` // asc or desc
}

// StreamStatus represents the status of a stream
type StreamStatus string

const (
	StreamStatusDraft     StreamStatus = "draft"
	StreamStatusStarting  StreamStatus = "starting"
	StreamStatusLive      StreamStatus = "live"
	StreamStatusStopping  StreamStatus = "stopping"
	StreamStatusStopped   StreamStatus = "stopped"
	StreamStatusError     StreamStatus = "error"
	StreamStatusArchived  StreamStatus = "archived"
)

// IngestProtocol represents the ingest protocol
type IngestProtocol string

const (
	IngestProtocolRTMP IngestProtocol = "rtmp"
	IngestProtocolSRT  IngestProtocol = "srt"
	IngestProtocolRIST IngestProtocol = "rist"
	IngestProtocolWHIP IngestProtocol = "whip"
	IngestProtocolRTSP IngestProtocol = "rtsp"
)

// PlaybackUrls represents playback URLs for a stream
type PlaybackUrls struct {
	HLS            *string `json:"hls,omitempty"`
	DASH           *string `json:"dash,omitempty"`
	WHEP           *string `json:"whep,omitempty"`
	MoQ            *string `json:"moq,omitempty"`
	WebSocketFMP4  *string `json:"websocket_fmp4,omitempty"`
}

// Stream represents a media stream
type Stream struct {
	ID                 uuid.UUID        `json:"id"`
	Name               string           `json:"name"`
	Slug               string           `json:"slug"`
	Status             StreamStatus     `json:"status"`
	TenantID           uuid.UUID        `json:"tenant_id"`
	IngestProtocol     IngestProtocol   `json:"ingest_protocol"`
	IngestURL          string           `json:"ingest_url"`
	PlaybackURLs       PlaybackUrls     `json:"playback_urls"`
	TranscodingProfile *string          `json:"transcoding_profile,omitempty"`
	RecordingEnabled   bool             `json:"recording_enabled"`
	DVREnabled         bool             `json:"dvr_enabled"`
	CreatedAt          time.Time        `json:"created_at"`
	UpdatedAt          time.Time        `json:"updated_at"`
	StartedAt          *time.Time       `json:"started_at,omitempty"`
	EndedAt            *time.Time       `json:"ended_at,omitempty"`
}

// CreateStreamRequest represents a request to create a stream
type CreateStreamRequest struct {
	Name               string         `json:"name"`
	Slug               *string        `json:"slug,omitempty"`
	IngestProtocol     IngestProtocol `json:"ingest_protocol"`
	TranscodingProfile *string        `json:"transcoding_profile,omitempty"`
	RecordingEnabled   *bool          `json:"recording_enabled,omitempty"`
	DVREnabled         *bool          `json:"dvr_enabled,omitempty"`
}

// UpdateStreamRequest represents a request to update a stream
type UpdateStreamRequest struct {
	Name               *string `json:"name,omitempty"`
	TranscodingProfile *string `json:"transcoding_profile,omitempty"`
	RecordingEnabled   *bool   `json:"recording_enabled,omitempty"`
	DVREnabled         *bool   `json:"dvr_enabled,omitempty"`
}

// StreamMetrics represents stream metrics
type StreamMetrics struct {
	StreamID       uuid.UUID  `json:"stream_id"`
	ViewersCurrent int64      `json:"viewers_current"`
	ViewersPeak    int64      `json:"viewers_peak"`
	BitrateIn      int64      `json:"bitrate_in"`
	BitrateOut     int64      `json:"bitrate_out"`
	BytesIn        int64      `json:"bytes_in"`
	BytesOut       int64      `json:"bytes_out"`
	PacketsLost    int64      `json:"packets_lost"`
	RTTMs          *float64   `json:"rtt_ms,omitempty"`
	Timestamp      time.Time  `json:"timestamp"`
}

// NodeStatus represents the status of a cluster node
type NodeStatus string

const (
	NodeStatusHealthy   NodeStatus = "healthy"
	NodeStatusDegraded  NodeStatus = "degraded"
	NodeStatusUnhealthy NodeStatus = "unhealthy"
	NodeStatusDraining  NodeStatus = "draining"
	NodeStatusOffline   NodeStatus = "offline"
)

// NodeRole represents the role of a cluster node
type NodeRole string

const (
	NodeRoleIngest     NodeRole = "ingest"
	NodeRoleTranscode  NodeRole = "transcode"
	NodeRoleEdge       NodeRole = "edge"
	NodeRoleOrigin     NodeRole = "origin"
	NodeRoleStorage    NodeRole = "storage"
	NodeRoleGateway    NodeRole = "gateway"
)

// NodeCapacity represents node capacity
type NodeCapacity struct {
	CPUCores      int   `json:"cpu_cores"`
	MemoryGB      int   `json:"memory_gb"`
	StorageGB     int64 `json:"storage_gb"`
	NetworkGbps   int   `json:"network_gbps"`
	MaxStreams    int   `json:"max_streams"`
	MaxViewers    int64 `json:"max_viewers"`
}

// NodeLoad represents node load
type NodeLoad struct {
	CPUPercent      float32 `json:"cpu_percent"`
	MemoryPercent   float32 `json:"memory_percent"`
	StoragePercent  float32 `json:"storage_percent"`
	NetworkPercent  float32 `json:"network_percent"`
	ActiveStreams   int     `json:"active_streams"`
	ActiveViewers   int64   `json:"active_viewers"`
}

// ClusterNode represents a cluster node
type ClusterNode struct {
	ID             uuid.UUID   `json:"id"`
	Name           string      `json:"name"`
	Endpoint       string      `json:"endpoint"`
	Region         string      `json:"region"`
	Zone           string      `json:"zone"`
	Capacity       NodeCapacity `json:"capacity"`
	Load           NodeLoad    `json:"load"`
	Status         NodeStatus  `json:"status"`
	Roles          []NodeRole  `json:"roles"`
	LastHeartbeat  time.Time   `json:"last_heartbeat"`
	Version        string      `json:"version"`
}

// Broadcast represents a broadcast
type Broadcast struct {
	ID            uuid.UUID `json:"id"`
	StreamID      uuid.UUID `json:"stream_id"`
	OwnerNodeID   uuid.UUID `json:"owner_node_id"`
	Status        string    `json:"status"`
	Viewers       int64     `json:"viewers"`
	StartedAt     time.Time `json:"started_at"`
	UpdatedAt     time.Time `json:"updated_at"`
}

// TenantStatus represents tenant status
type TenantStatus string

const (
	TenantStatusActive   TenantStatus = "active"
	TenantStatusTrial    TenantStatus = "trial"
	TenantStatusSuspended TenantStatus = "suspended"
	TenantStatusDeleted  TenantStatus = "deleted"
)

// TenantQuotas represents tenant quotas
type TenantQuotas struct {
	MaxStreams             int   `json:"max_streams"`
	MaxConcurrentViewers   int64 `json:"max_concurrent_viewers"`
	MaxStorageGB           int64 `json:"max_storage_gb"`
	MaxBandwidthGbps       int   `json:"max_bandwidth_gbps"`
	MaxTranscodingHours    int   `json:"max_transcoding_hours"`
	CustomDomains          int   `json:"custom_domains"`
}

// TenantBranding represents tenant branding
type TenantBranding struct {
	LogoURL       *string `json:"logo_url,omitempty"`
	FaviconURL    *string `json:"favicon_url,omitempty"`
	PrimaryColor  *string `json:"primary_color,omitempty"`
	SecondaryColor *string `json:"secondary_color,omitempty"`
	CustomCSS     *string `json:"custom_css,omitempty"`
}

// GeoBlocking represents geo-blocking settings
type GeoBlocking struct {
	AllowedCountries []string `json:"allowed_countries,omitempty"`
	BlockedCountries []string `json:"blocked_countries,omitempty"`
}

// TenantSettings represents tenant settings
type TenantSettings struct {
	AllowedProtocols        []IngestProtocol `json:"allowed_protocols"`
	DefaultTranscodingProfile *string        `json:"default_transcoding_profile,omitempty"`
	RecordingEnabled        bool             `json:"recording_enabled"`
	DVREnabled              bool             `json:"dvr_enabled"`
	GeoBlocking             *GeoBlocking     `json:"geo_blocking,omitempty"`
	DRMEnabled              bool             `json:"drm_enabled"`
}

// Tenant represents a tenant
type Tenant struct {
	ID        uuid.UUID      `json:"id"`
	Name      string         `json:"name"`
	Slug      string         `json:"slug"`
	Status    TenantStatus   `json:"status"`
	Quotas    TenantQuotas   `json:"quotas"`
	Branding  *TenantBranding `json:"branding,omitempty"`
	Settings  TenantSettings `json:"settings"`
	CreatedAt time.Time      `json:"created_at"`
	UpdatedAt time.Time      `json:"updated_at"`
}

// CreateTenantRequest represents a request to create a tenant
type CreateTenantRequest struct {
	Name     string           `json:"name"`
	Slug     *string          `json:"slug,omitempty"`
	Quotas   *TenantQuotas    `json:"quotas,omitempty"`
	Branding *TenantBranding  `json:"branding,omitempty"`
	Settings *TenantSettings  `json:"settings,omitempty"`
}

// UserStatus represents user status
type UserStatus string

const (
	UserStatusActive    UserStatus = "active"
	UserStatusInvited   UserStatus = "invited"
	UserStatusSuspended UserStatus = "suspended"
	UserStatusDeleted   UserStatus = "deleted"
)

// User represents a user
type User struct {
	ID            uuid.UUID  `json:"id"`
	Email         string     `json:"email"`
	Name          string     `json:"name"`
	Roles         []string   `json:"roles"`
	MFAEnabled    bool       `json:"mfa_enabled"`
	Status        UserStatus `json:"status"`
	TenantID      uuid.UUID  `json:"tenant_id"`
	TenantName    string     `json:"tenant_name"`
	LastLoginAt   *time.Time `json:"last_login_at,omitempty"`
	CreatedAt     time.Time  `json:"created_at"`
	UpdatedAt     time.Time  `json:"updated_at"`
}

// LoginRequest represents a login request
type LoginRequest struct {
	Email    string  `json:"email"`
	Password string  `json:"password"`
	MFACode  *string `json:"mfa_code,omitempty"`
}

// LoginResponse represents a login response
type LoginResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	User         User   `json:"user"`
	Tenant       Tenant `json:"tenant"`
	RequiresMFA  bool   `json:"requires_mfa"`
}

// InviteUserRequest represents an invite user request
type InviteUserRequest struct {
	Email    string   `json:"email"`
	Name     string   `json:"name"`
	Roles    []string `json:"roles"`
	TenantID uuid.UUID `json:"tenant_id"`
}

// ClusterHealth represents cluster health
type ClusterHealth struct {
	Status          string `json:"status"`
	NodesTotal      int    `json:"nodes_total"`
	NodesHealthy    int    `json:"nodes_healthy"`
	NodesDegraded   int    `json:"nodes_degraded"`
	NodesOffline    int    `json:"nodes_offline"`
	StreamsActive   int    `json:"streams_active"`
	ViewersTotal    int64  `json:"viewers_total"`
}