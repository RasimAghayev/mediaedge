package mediaedge

import (
	"context"
	"fmt"
	"net/url"
)

// MonetizationService handles monetization-related API calls
type MonetizationService struct {
	client *Client
}

// Monetization returns the monetization service
func (c *Client) Monetization() *MonetizationService {
	return &MonetizationService{client: c}
}

// FAST Channel types
type FastChannelStatus string

const (
	FastChannelStatusDraft     FastChannelStatus = "draft"
	FastChannelStatusActive    FastChannelStatus = "active"
	FastChannelStatusPaused    FastChannelStatus = "paused"
	FastChannelStatusArchived  FastChannelStatus = "archived"
)

type FastChannelSchedule struct {
	Timezone string           `json:"timezone"`
	Entries  []ScheduleEntry  `json:"entries"`
}

type ScheduleEntry struct {
	ContentID      uuid.UUID  `json:"content_id"`
	StartTime      time.Time  `json:"start_time"`
	DurationSeconds int       `json:"duration_seconds"`
	Repeat         *RepeatRule `json:"repeat,omitempty"`
}

type RepeatRule struct {
	Frequency string    `json:"frequency"` // daily, weekly, monthly
	Interval  int       `json:"interval"`
	Until     *time.Time `json:"until,omitempty"`
}

type AdBreakConfig struct {
	Enabled            bool   `json:"enabled"`
	Provider           string `json:"provider"` // mediatailor, yospace, broadpeak, google_ad_manager, prebid
	TagURL             string `json:"tag_url,omitempty"`
	MinIntervalSeconds int    `json:"min_interval_seconds"`
	MaxDurationSeconds int    `json:"max_duration_seconds"`
}

type FastChannel struct {
	ID              uuid.UUID         `json:"id"`
	TenantID        uuid.UUID         `json:"tenant_id"`
	Name            string            `json:"name"`
	Slug            string            `json:"slug"`
	Status          FastChannelStatus `json:"status"`
	Schedule        FastChannelSchedule `json:"schedule"`
	AdBreaks        AdBreakConfig     `json:"ad_breaks"`
	OutputFormats   []string          `json:"output_formats"`
	CreatedAt       time.Time         `json:"created_at"`
	UpdatedAt       time.Time         `json:"updated_at"`
}

type CreateFastChannelRequest struct {
	Name         string              `json:"name"`
	Slug         *string             `json:"slug,omitempty"`
	Schedule     FastChannelSchedule `json:"schedule"`
	AdBreaks     AdBreakConfig       `json:"ad_breaks"`
	OutputFormats []string           `json:"output_formats"`
}

// FastChannels lists FAST channels
func (s *MonetizationService) FastChannels(ctx context.Context, params ListParams) (*PaginatedResponse[FastChannel], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[FastChannel]
	err := s.client.GET(ctx, "/api/v1/monetization/fast-channels", query, &result)
	return &result, err
}

// FastChannel gets a FAST channel by ID
func (s *MonetizationService) FastChannel(ctx context.Context, id uuid.UUID) (*FastChannel, error) {
	var result FastChannel
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/monetization/fast-channels/%s", id), nil, &result)
	return &result, err
}

// CreateFastChannel creates a FAST channel
func (s *MonetizationService) CreateFastChannel(ctx context.Context, req CreateFastChannelRequest) (*FastChannel, error) {
	var result FastChannel
	err := s.client.POST(ctx, "/api/v1/monetization/fast-channels", req, &result)
	return &result, err
}

// TVOD Product types
type TvodStatus string

const (
	TvodStatusDraft    TvodStatus = "draft"
	TvodStatusActive   TvodStatus = "active"
	TvodStatusHidden   TvodStatus = "hidden"
	TvodStatusArchived TvodStatus = "archived"
)

type AvailabilityWindow struct {
	Start time.Time  `json:"start"`
	End   *time.Time `json:"end,omitempty"`
}

type TvodProduct struct {
	ID                    uuid.UUID       `json:"id"`
	TenantID              uuid.UUID       `json:"tenant_id"`
	Title                 string          `json:"title"`
	Description           *string         `json:"description,omitempty"`
	PosterURL             *string         `json:"poster_url,omitempty"`
	TrailerURL            *string         `json:"trailer_url,omitempty"`
	ContentID             uuid.UUID       `json:"content_id"`
	RentalPriceCents      *int            `json:"rental_price_cents,omitempty"`
	PurchasePriceCents    *int            `json:"purchase_price_cents,omitempty"`
	RentalDurationHours   *int            `json:"rental_duration_hours,omitempty"`
	PurchaseAvailability  *AvailabilityWindow `json:"purchase_availability,omitempty"`
	RentalAvailability    *AvailabilityWindow `json:"rental_availability,omitempty"`
	Territories           []string        `json:"territories"`
	Status                TvodStatus      `json:"status"`
	Metadata              map[string]any  `json:"metadata"`
	CreatedAt             time.Time       `json:"created_at"`
	UpdatedAt             time.Time       `json:"updated_at"`
}

type CreateTvodProductRequest struct {
	Title                  string            `json:"title"`
	Description            *string           `json:"description,omitempty"`
	PosterURL              *string           `json:"poster_url,omitempty"`
	TrailerURL             *string           `json:"trailer_url,omitempty"`
	ContentID              uuid.UUID         `json:"content_id"`
	RentalPriceCents       *int              `json:"rental_price_cents,omitempty"`
	PurchasePriceCents     *int              `json:"purchase_price_cents,omitempty"`
	RentalDurationHours    *int              `json:"rental_duration_hours,omitempty"`
	PurchaseAvailability   *AvailabilityWindow `json:"purchase_availability,omitempty"`
	RentalAvailability     *AvailabilityWindow `json:"rental_availability,omitempty"`
	Territories            []string          `json:"territories"`
	Metadata               map[string]any    `json:"metadata"`
}

// TvodProducts lists TVOD products
func (s *MonetizationService) TvodProducts(ctx context.Context, params ListParams) (*PaginatedResponse[TvodProduct], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[TvodProduct]
	err := s.client.GET(ctx, "/api/v1/monetization/tvod/products", query, &result)
	return &result, err
}

// TvodProduct gets a TVOD product by ID
func (s *MonetizationService) TvodProduct(ctx context.Context, id uuid.UUID) (*TvodProduct, error) {
	var result TvodProduct
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/monetization/tvod/products/%s", id), nil, &result)
	return &result, err
}

// CreateTvodProduct creates a TVOD product
func (s *MonetizationService) CreateTvodProduct(ctx context.Context, req CreateTvodProductRequest) (*TvodProduct, error) {
	var result TvodProduct
	err := s.client.POST(ctx, "/api/v1/monetization/tvod/products", req, &result)
	return &result, err
}

// Bundle types
type BundleType string

const (
	BundleTypeStreamer  BundleType = "streamer_bundle"
	BundleTypeTelco     BundleType = "telco_bundle"
	BundleTypeAggregator BundleType = "aggregator_bundle"
)

type BundleStatus string

const (
	BundleStatusDraft    BundleStatus = "draft"
	BundleStatusActive   BundleStatus = "active"
	BundleStatusPaused   BundleStatus = "paused"
	BundleStatusArchived BundleStatus = "archived"
)

type BillingCycle string

const (
	BillingCycleMonthly   BillingCycle = "monthly"
	BillingCycleQuarterly BillingCycle = "quarterly"
	BillingCycleYearly    BillingCycle = "yearly"
)

type BundlePricing struct {
	BasePriceCents   int          `json:"base_price_cents"`
	BillingCycle     BillingCycle `json:"billing_cycle"`
	TrialDays        *int         `json:"trial_days,omitempty"`
	SetupFeeCents    *int         `json:"setup_fee_cents,omitempty"`
}

type ProductType string

const (
	ProductTypeFastChannel ProductType = "fast_channel"
	ProductTypeTvodProduct ProductType = "tvod_product"
	ProductTypeLiveStream  ProductType = "live_stream"
	ProductTypeStorage     ProductType = "storage"
	ProductTypeTranscoding ProductType = "transcoding"
)

type BundleProduct struct {
	ProductID   uuid.UUID  `json:"product_id"`
	ProductType ProductType `json:"product_type"`
	Quantity    int        `json:"quantity"`
}

type PartnerValidation struct {
	PartnerID      string         `json:"partner_id"`
	ValidationType string         `json:"validation_type"`
	Config         map[string]any `json:"config"`
}

type Bundle struct {
	ID                 uuid.UUID          `json:"id"`
	TenantID           uuid.UUID          `json:"tenant_id"`
	Name               string             `json:"name"`
	Description        *string            `json:"description,omitempty"`
	BundleType         BundleType         `json:"bundle_type"`
	Pricing            BundlePricing      `json:"pricing"`
	IncludedProducts   []BundleProduct    `json:"included_products"`
	PartnerValidation  *PartnerValidation `json:"partner_validation,omitempty"`
	Status             BundleStatus       `json:"status"`
	CreatedAt          time.Time          `json:"created_at"`
	UpdatedAt          time.Time          `json:"updated_at"`
}

type CreateBundleRequest struct {
	Name               string            `json:"name"`
	Description        *string           `json:"description,omitempty"`
	BundleType         BundleType        `json:"bundle_type"`
	Pricing            BundlePricing     `json:"pricing"`
	IncludedProducts   []BundleProduct   `json:"included_products"`
	PartnerValidation  *PartnerValidation `json:"partner_validation,omitempty"`
}

// Bundles lists bundles
func (s *MonetizationService) Bundles(ctx context.Context, params ListParams) (*PaginatedResponse[Bundle], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[Bundle]
	err := s.client.GET(ctx, "/api/v1/monetization/bundles", query, &result)
	return &result, err
}

// Bundle gets a bundle by ID
func (s *MonetizationService) Bundle(ctx context.Context, id uuid.UUID) (*Bundle, error) {
	var result Bundle
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/monetization/bundles/%s", id), nil, &result)
	return &result, err
}

// CreateBundle creates a bundle
func (s *MonetizationService) CreateBundle(ctx context.Context, req CreateBundleRequest) (*Bundle, error) {
	var result Bundle
	err := s.client.POST(ctx, "/api/v1/monetization/bundles", req, &result)
	return &result, err
}

// Entitlement types
type ContentType string

const (
	ContentTypeStream       ContentType = "stream"
	ContentTypeVod          ContentType = "vod"
	ContentTypeFastChannel  ContentType = "fast_channel"
	ContentTypeBundle       ContentType = "bundle"
)

type GrantType string

const (
	GrantTypeSubscription GrantType = "subscription"
	GrantTypeRental       GrantType = "rental"
	GrantTypePurchase     GrantType = "purchase"
	GrantTypeBundle       GrantType = "bundle"
	GrantTypePromo        GrantType = "promo"
)

type EntitlementCheck struct {
	UserID      uuid.UUID  `json:"user_id"`
	ContentID   uuid.UUID  `json:"content_id"`
	ContentType ContentType `json:"content_type"`
	GrantType   *GrantType  `json:"grant_type,omitempty"`
	ExpiresAt   *time.Time  `json:"expires_at,omitempty"`
}

type EntitlementResult struct {
	Granted     bool         `json:"granted"`
	Reason      *string      `json:"reason,omitempty"`
	Entitlement *Entitlement `json:"entitlement,omitempty"`
}

type Entitlement struct {
	ID          uuid.UUID  `json:"id"`
	UserID      uuid.UUID  `json:"user_id"`
	ContentID   uuid.UUID  `json:"content_id"`
	ContentType ContentType `json:"content_type"`
	GrantType   GrantType   `json:"grant_type"`
	GrantedAt   time.Time  `json:"granted_at"`
	ExpiresAt   *time.Time  `json:"expires_at,omitempty"`
}

// CheckEntitlement checks an entitlement
func (s *MonetizationService) CheckEntitlement(ctx context.Context, req EntitlementCheck) (*EntitlementResult, error) {
	var result EntitlementResult
	err := s.client.POST(ctx, "/api/v1/monetization/entitlements/check", req, &result)
	return &result, err
}

// Invoice types
type InvoiceStatus string

const (
	InvoiceStatusDraft         InvoiceStatus = "draft"
	InvoiceStatusOpen          InvoiceStatus = "open"
	InvoiceStatusPaid          InvoiceStatus = "paid"
	InvoiceStatusVoid          InvoiceStatus = "void"
	InvoiceStatusUncollectible InvoiceStatus = "uncollectible"
)

type InvoiceLineItem struct {
	Description     string `json:"description"`
	Quantity        int    `json:"quantity"`
	UnitPriceCents  int64  `json:"unit_price_cents"`
	AmountCents     int64  `json:"amount_cents"`
}

type Invoice struct {
	ID            uuid.UUID        `json:"id"`
	TenantID      uuid.UUID        `json:"tenant_id"`
	CustomerID    uuid.UUID        `json:"customer_id"`
	AmountCents   int64            `json:"amount_cents"`
	Currency      string           `json:"currency"`
	Status        InvoiceStatus    `json:"status"`
	LineItems     []InvoiceLineItem `json:"line_items"`
	CreatedAt     time.Time        `json:"created_at"`
	DueDate       time.Time        `json:"due_date"`
	PaidAt        *time.Time       `json:"paid_at,omitempty"`
}

// Invoices lists invoices
func (s *MonetizationService) Invoices(ctx context.Context, params ListParams) (*PaginatedResponse[Invoice], error) {
	query := url.Values{}
	if params.Page > 0 {
		query.Set("page", fmt.Sprintf("%d", params.Page))
	}
	if params.PerPage > 0 {
		query.Set("per_page", fmt.Sprintf("%d", params.PerPage))
	}

	var result PaginatedResponse[Invoice]
	err := s.client.GET(ctx, "/api/v1/monetization/billing/invoices", query, &result)
	return &result, err
}

// Invoice gets an invoice by ID
func (s *MonetizationService) Invoice(ctx context.Context, id uuid.UUID) (*Invoice, error) {
	var result Invoice
	err := s.client.GET(ctx, fmt.Sprintf("/api/v1/monetization/billing/invoices/%s", id), nil, &result)
	return &result, err
}