package mediaedge

import (
	"context"
	"fmt"
	"net/url"
)

// ClusterService handles cluster-related API calls
type ClusterService struct {
	client *Client
}

// Cluster returns the cluster service
func (c *Client) Cluster() *ClusterService {
	return &ClusterService{client: c}
}

// Nodes lists cluster nodes
func (s *ClusterService) Nodes(ctx context.Context, params ListParams) (*PaginatedResponse[ClusterNode], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[ClusterNode]
	err := s.client.GET(ctx, "/api/v1/cluster/nodes", query, &result)
	return &result, err
}

// Node gets a cluster node by ID
func (s *ClusterService) Node(ctx context.Context, id uuid.UUID) (*ClusterNode, error) {
	var result ClusterNode
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/cluster/nodes/%s", id), nil, &result)
	return &result, err
}

// Broadcasts lists broadcasts
func (s *ClusterService) Broadcasts(ctx context.Context, params ListParams) (*PaginatedResponse[Broadcast], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[Broadcast]
	err := s.client.GET(ctx, "/api/v1/cluster/broadcasts", query, &result)
	return &result, err
}

// Health gets cluster health
func (s *ClusterService) Health(ctx context.Context) (*ClusterHealth, error) {
	var result ClusterHealth
	err := s.client.GET(ctx, "/api/v1/cluster/health", nil, &result)
	return &result, err
}

// FederationStatus represents federation status
type FederationStatus struct {
	Clusters         []FederatedCluster `json:"clusters"`
	PullConnections  int                `json:"pull_connections"`
	PushConnections  int                `json:"push_connections"`
}

// FederatedCluster represents a federated cluster
type FederatedCluster struct {
	ID       uuid.UUID `json:"id"`
	Name     string    `json:"name"`
	Endpoint string    `json:"endpoint"`
	Status   string    `json:"status"`
	LatencyMs float64  `json:"latency_ms"`
}

// Federation gets federation status
func (s *ClusterService) Federation(ctx context.Context) (*FederationStatus, error) {
	var result FederationStatus
	err := s.client.GET(ctx, "/api/v1/cluster/federation", nil, &result)
	return &result, err
}

// MeshStatus represents mesh status
type MeshStatus struct {
	Peers        int     `json:"peers"`
	Connections  int     `json:"connections"`
	BandwidthMbps float64 `json:"bandwidth_mbps"`
}

// Mesh gets mesh status
func (s *ClusterService) Mesh(ctx context.Context) (*MeshStatus, error) {
	var result MeshStatus
	err := s.client.GET(ctx, "/api/v1/cluster/mesh", nil, &result)
	return &result, err
}

// SyncResult represents config sync result
type SyncResult struct {
	Success      bool     `json:"success"`
	Version      int64    `json:"version"`
	NodesSynced  int      `json:"nodes_synced"`
	Errors       []string `json:"errors"`
}

// SyncConfig triggers config sync
func (s *ClusterService) SyncConfig(ctx context.Context) (*SyncResult, error) {
	var result SyncResult
	err := s.client.POST(ctx, "/api/v1/cluster/config/sync", nil, &result)
	return &result, err
}