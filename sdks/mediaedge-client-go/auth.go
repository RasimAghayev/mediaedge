package mediaedge

import (
	"context"
	"fmt"
	"net/url"
)

// AuthService handles authentication-related API calls
type AuthService struct {
	client *Client
}

// Auth returns the auth service
func (c *Client) Auth() *AuthService {
	return &AuthService{client: c}
}

// Login performs user login
func (s *AuthService) Login(ctx context.Context, req LoginRequest) (*LoginResponse, error) {
	var result LoginResponse
	err := s.client.POST(ctx, "/api/v1/auth/login", req, &result)
	if err == nil {
		s.client.SetAccessToken(result.AccessToken)
		s.client.SetRefreshToken(result.RefreshToken)
	}
	return &result, err
}

// RefreshToken refreshes the access token
func (s *AuthService) RefreshToken(ctx context.Context) (*LoginResponse, error) {
	if s.client.refreshToken == "" {
		return nil, fmt.Errorf("no refresh token available")
	}
	var result LoginResponse
	err := s.client.POST(ctx, "/api/v1/auth/refresh", map[string]string{
		"refresh_token": s.client.refreshToken,
	}, &result)
	if err == nil {
		s.client.SetAccessToken(result.AccessToken)
		s.client.SetRefreshToken(result.RefreshToken)
	}
	return &result, err
}

// Logout logs out the current user
func (s *AuthService) Logout(ctx context.Context) error {
	err := s.client.POST(ctx, "/api/v1/auth/logout", nil, nil)
	if err == nil {
		s.client.SetAccessToken("")
		s.client.SetRefreshToken("")
	}
	return err
}

// Me gets the current user
func (s *AuthService) Me(ctx context.Context) (*User, error) {
	var result User
	err := s.client.GET(ctx, "/api/v1/auth/me", nil, &result)
	return &result, err
}

// UpdateMe updates the current user
func (s *AuthService) UpdateMe(ctx context.Context, req map[string]any) (*User, error) {
	var result User
	err := s.client.PATCH(ctx, "/api/v1/auth/me", req, &result)
	return &result, err
}

// ChangePassword changes the current user's password
func (s *AuthService) ChangePassword(ctx context.Context, currentPassword, newPassword string) error {
	return s.client.POST(ctx, "/api/v1/auth/password", map[string]string{
		"current_password": currentPassword,
		"new_password":     newPassword,
	}, nil)
}

// SetupTOTP sets up TOTP MFA
func (s *AuthService) SetupTOTP(ctx context.Context) (*TOTPSetupResponse, error) {
	var result TOTPSetupResponse
	err := s.client.POST(ctx, "/api/v1/auth/mfa/totp/setup", nil, &result)
	return &result, err
}

// VerifyTOTP verifies a TOTP code
func (s *AuthService) VerifyTOTP(ctx context.Context, code string) error {
	return s.client.POST(ctx, "/api/v1/auth/mfa/totp/verify", map[string]string{
		"code": code,
	}, nil)
}

// TOTPSetupResponse represents TOTP setup response
type TOTPSetupResponse struct {
	Secret       string   `json:"secret"`
	QRCodeURL    string   `json:"qr_code_url"`
	BackupCodes  []string `json:"backup_codes"`
}

// UsersService handles user management (admin)
type UsersService struct {
	client *Client
}

// Users returns the users service
func (c *Client) Users() *UsersService {
	return &UsersService{client: c}
}

// List lists users
func (s *UsersService) List(ctx context.Context, params ListParams) (*PaginatedResponse[User], error) {
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

	var result PaginatedResponse[User]
	err := s.client.GET(ctx, "/api/v1/users", query, &result)
	return &result, err
}

// Invite invites a new user
func (s *UsersService) Invite(ctx context.Context, req InviteUserRequest) (*User, error) {
	var result User
	err := s.client.POST(ctx, "/api/v1/users/invite", req, &result)
	return &result, err
}

// Get gets a user by ID
func (s *UsersService) Get(ctx context.Context, id uuid.UUID) (*User, error) {
	var result User
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/users/%s", id), nil, &result)
	return &result, err
}

// Update updates a user
func (s *UsersService) Update(ctx context.Context, id uuid.UUID, req map[string]any) (*User, error) {
	var result User
	err := s.client.PATCH(ctx, fmt.Sprintf("/api/v1/users/%s", id), req, &result)
	return &result, err
}

// Delete deletes a user
func (s *UsersService) Delete(ctx context.Context, id uuid.UUID) error {
	return s.client.DELETE(ctx, fmt.Sprintf("/api/v1/users/%s", id), nil)
}

// OIDCStart starts OIDC login
func (s *AuthService) OIDCStart(ctx context.Context, provider, returnURL string) (*OIDCStartResponse, error) {
	var result OIDCStartResponse
	err := s.client.POST(ctx, "/api/v1/auth/oidc/start", map[string]string{
		"provider":     provider,
		"return_url":   returnURL,
	}, &result)
	return &result, err
}

// OIDCStartResponse represents OIDC start response
type OIDCStartResponse struct {
	AuthURL string `json:"auth_url"`
	State   string `json:"state"`
}

// OIDCCallback handles OIDC callback
func (s *AuthService) OIDCCallback(ctx context.Context, provider, code, state string) (*LoginResponse, error) {
	var result LoginResponse
	err := s.client.POST(ctx, fmt.Sprintf("/api/v1/auth/oidc/%s/callback", provider), map[string]string{
		"code":  code,
		"state": state,
	}, &result)
	if err == nil {
		s.client.SetAccessToken(result.AccessToken)
		s.client.SetRefreshToken(result.RefreshToken)
	}
	return &result, err
}