package mediaedge

import (
	"context"
	"fmt"
	"net/url"
)

// TenantsService handles tenant-related API calls
type TenantsService struct {
	client *Client
}

// Tenants returns the tenants service
func (c *Client) Tenants() *TenantsService {
	return &TenantsService{client: c}
}

// List lists tenants
func (s *TenantsService) List(ctx context.Context, params ListParams) (*PaginatedResponse[Tenant], error) {
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

	var result PaginatedResponse[Tenant]
	err := s.client.GET(ctx, "/api/v1/tenants", query, &result)
	return &result, err
}

// Get gets a tenant by ID
func (s *TenantsService) Get(ctx context.Context, id uuid.UUID) (*Tenant, error) {
	var result Tenant
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/tenants/%s", id), nil, &result)
	return &result, err
}

// Create creates a new tenant
func (s *TenantsService) Create(ctx context.Context, req CreateTenantRequest) (*Tenant, error) {
	var result Tenant
	err := s.client.POST(ctx, "/api/v1/tenants", req, &result)
	return &result, err
}

// Update updates a tenant
func (s *TenantsService) Update(ctx context.Context, id uuid.UUID, req CreateTenantRequest) (*Tenant, error) {
	var result Tenant
	err := s.client.PATCH(ctx, fmt.Sprintf("/api/v1/tenants/%s", id), req, &result)
	return &result, err
}

// Delete deletes a tenant
func (s *TenantsService) Delete(ctx context.Context, id uuid.UUID) error {
	return s.client.DELETE(ctx, fmt.Sprintf("/api/v1/tenants/%s", id), nil)
}

// Quotas gets tenant quotas
func (s *TenantsService) Quotas(ctx context.Context, id uuid.UUID) (*TenantQuotas, error) {
	var result TenantQuotas
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/tenants/%s/quotas", id), nil, &result)
	return &result, err
}

// UpdateQuotas updates tenant quotas
func (s *TenantsService) UpdateQuotas(ctx context.Context, id uuid.UUID, quotas TenantQuotas) (*TenantQuotas, error) {
	var result TenantQuotas
	err := s.client.PATCH(ctx, fmt.Sprintf("/api/v1/tenants/%s/quotas", id), quotas, &result)
	return &result, err
}

// Users lists tenant users
func (s *TenantsService) Users(ctx context.Context, id uuid.UUID, params ListParams) (*PaginatedResponse[User], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[User]
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/tenants/%s/users", id), query, &result)
	return &result, err
}