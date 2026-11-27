package mediaedge

import (
	"context"

	"github.com/hashicorp/terraform-plugin-framework-validators/stringvalidator"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/hashicorp/terraform-plugin-log/tflog"
)

// TenantResource is the tenant resource
type TenantResource struct{}

// NewTenantResource creates a new tenant resource
func NewTenantResource() resource.Resource {
	return &TenantResource{}
}

// TenantResourceModel maps the resource schema data
type TenantResourceModel struct {
	ID                 types.String `tfsdk:"id"`
	Name               types.String `tfsdk:"name"`
	Slug               types.String `tfsdk:"slug"`
	Status             types.String `tfsdk:"status"`
	MaxStreams         types.Int64  `tfsdk:"max_streams"`
	MaxConcurrentViewers types.Int64 `tfsdk:"max_concurrent_viewers"`
	MaxStorageGB       types.Int64  `tfsdk:"max_storage_gb"`
	MaxBandwidthGbps   types.Int64  `tfsdk:"max_bandwidth_gbps"`
	MaxTranscodingHours types.Int64  `tfsdk:"max_transcoding_hours"`
	CustomDomains      types.Int64  `tfsdk:"custom_domains"`
	LogoURL            types.String `tfsdk:"logo_url"`
	FaviconURL         types.String `tfsdk:"favicon_url"`
	PrimaryColor       types.String `tfsdk:"primary_color"`
	SecondaryColor     types.String `tfsdk:"secondary_color"`
	CustomCSS          types.String `tfsdk:"custom_css"`
	AllowedProtocols   types.List   `tfsdk:"allowed_protocols"`
	DefaultTranscodingProfile types.String `tfsdk:"default_transcoding_profile"`
	RecordingEnabled   types.Bool   `tfsdk:"recording_enabled"`
	DVREnabled         types.Bool   `tfsdk:"dvr_enabled"`
	DRMEnabled         types.Bool   `tfsdk:"drm_enabled"`
	CreatedAt          types.String `tfsdk:"created_at"`
	UpdatedAt          types.String `tfsdk:"updated_at"`
}

// Metadata returns the resource metadata
func (r *TenantResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_tenant"
}

// Schema returns the resource schema
func (r *TenantResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge tenant.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Tenant ID.",
				Computed:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.UseStateForUnknown(),
				},
			},
			"name": schema.StringAttribute{
				Description: "Tenant name.",
				Required:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Tenant slug.",
				Optional:    true,
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Tenant status.",
				Computed:    true,
			},
			"max_streams": schema.Int64Attribute{
				Description: "Maximum number of streams.",
				Optional:    true,
				Computed:    true,
			},
			"max_concurrent_viewers": schema.Int64Attribute{
				Description: "Maximum concurrent viewers.",
				Optional:    true,
				Computed:    true,
			},
			"max_storage_gb": schema.Int64Attribute{
				Description: "Maximum storage in GB.",
				Optional:    true,
				Computed:    true,
			},
			"max_bandwidth_gbps": schema.Int64Attribute{
				Description: "Maximum bandwidth in Gbps.",
				Optional:    true,
				Computed:    true,
			},
			"max_transcoding_hours": schema.Int64Attribute{
				Description: "Maximum transcoding hours per month.",
				Optional:    true,
				Computed:    true,
			},
			"custom_domains": schema.Int64Attribute{
				Description: "Maximum custom domains.",
				Optional:    true,
				Computed:    true,
			},
			"logo_url": schema.StringAttribute{
				Description: "Logo URL.",
				Optional:    true,
			},
			"favicon_url": schema.StringAttribute{
				Description: "Favicon URL.",
				Optional:    true,
			},
			"primary_color": schema.StringAttribute{
				Description: "Primary brand color.",
				Optional:    true,
			},
			"secondary_color": schema.StringAttribute{
				Description: "Secondary brand color.",
				Optional:    true,
			},
			"custom_css": schema.StringAttribute{
				Description: "Custom CSS.",
				Optional:    true,
			},
			"allowed_protocols": schema.ListAttribute{
				Description: "Allowed ingest protocols.",
				Optional:    true,
				ElementType: types.StringType,
			},
			"default_transcoding_profile": schema.StringAttribute{
				Description: "Default transcoding profile.",
				Optional:    true,
			},
			"recording_enabled": schema.BoolAttribute{
				Description: "Whether recording is enabled.",
				Optional:    true,
				Computed:    true,
			},
			"dvr_enabled": schema.BoolAttribute{
				Description: "Whether DVR is enabled.",
				Optional:    true,
				Computed:    true,
			},
			"drm_enabled": schema.BoolAttribute{
				Description: "Whether DRM is enabled.",
				Optional:    true,
				Computed:    true,
			},
			"created_at": schema.StringAttribute{
				Description: "Creation timestamp.",
				Computed:    true,
			},
			"updated_at": schema.StringAttribute{
				Description: "Last update timestamp.",
				Computed:    true,
			},
		},
	}
}

// Create creates the resource
func (r *TenantResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan TenantResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Creating tenant", map[string]any{"name": plan.Name.ValueString()})

	plan.ID = types.StringValue("mock-tenant-id")
	plan.Status = types.StringValue("active")
	plan.CreatedAt = types.StringValue("2024-01-01T00:00:00Z")
	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *TenantResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state TenantResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading tenant", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *TenantResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan TenantResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Updating tenant", map[string]any{"id": plan.ID.ValueString()})

	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *TenantResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state TenantResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Deleting tenant", map[string]any{"id": state.ID.ValueString()})
}

// TenantDataSource is the tenant data source
type TenantDataSource struct{}

// NewTenantDataSource creates a new tenant data source
func NewTenantDataSource() resource.DataSource {
	return &TenantDataSource{}
}

// Metadata returns the data source metadata
func (d *TenantDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_tenant"
}

// Schema returns the data source schema
func (d *TenantDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge tenant.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Tenant ID.",
				Required:    true,
			},
			"name": schema.StringAttribute{
				Description: "Tenant name.",
				Computed:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Tenant slug.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Tenant status.",
				Computed:    true,
			},
			"max_streams": schema.Int64Attribute{
				Description: "Maximum streams.",
				Computed:    true,
			},
			"max_concurrent_viewers": schema.Int64Attribute{
				Description: "Maximum concurrent viewers.",
				Computed:    true,
			},
		},
	}
}

// Read reads the data source
func (d *TenantDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config TenantResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading tenant data source", map[string]any{"id": config.ID.ValueString()})

	config.Name = types.StringValue("Example Tenant")
	config.Slug = types.StringValue("example-tenant")
	config.Status = types.StringValue("active")
	config.MaxStreams = types.Int64Value(100)
	config.MaxConcurrentViewers = types.Int64Value(10000)

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}