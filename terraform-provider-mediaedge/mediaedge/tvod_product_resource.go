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

// TvodProductResource is the TVOD product resource
type TvodProductResource struct{}

// NewTvodProductResource creates a new TVOD product resource
func NewTvodProductResource() resource.Resource {
	return &TvodProductResource{}
}

// TvodProductResourceModel maps the resource schema data
type TvodProductResourceModel struct {
	ID                   types.String `tfsdk:"id"`
	TenantID             types.String `tfsdk:"tenant_id"`
	Title                types.String `tfsdk:"title"`
	Description          types.String `tfsdk:"description"`
	PosterURL            types.String `tfsdk:"poster_url"`
	TrailerURL           types.String `tfsdk:"trailer_url"`
	ContentID            types.String `tfsdk:"content_id"`
	RentalPriceCents     types.Int64  `tfsdk:"rental_price_cents"`
	PurchasePriceCents   types.Int64  `tfsdk:"purchase_price_cents"`
	RentalDurationHours  types.Int64  `tfsdk:"rental_duration_hours"`
	Territories          types.List   `tfsdk:"territories"`
	Status               types.String `tfsdk:"status"`
	CreatedAt            types.String `tfsdk:"created_at"`
	UpdatedAt            types.String `tfsdk:"updated_at"`
}

// Metadata returns the resource metadata
func (r *TvodProductResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_tvod_product"
}

// Schema returns the resource schema
func (r *TvodProductResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge TVOD product.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Product ID.",
				Computed:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.UseStateForUnknown(),
				},
			},
			"tenant_id": schema.StringAttribute{
				Description: "Tenant ID.",
				Required:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.RequiresReplace(),
				},
			},
			"title": schema.StringAttribute{
				Description: "Product title.",
				Required:    true,
			},
			"description": schema.StringAttribute{
				Description: "Product description.",
				Optional:    true,
			},
			"poster_url": schema.StringAttribute{
				Description: "Poster image URL.",
				Optional:    true,
			},
			"trailer_url": schema.StringAttribute{
				Description: "Trailer video URL.",
				Optional:    true,
			},
			"content_id": schema.StringAttribute{
				Description: "Content ID.",
				Required:    true,
			},
			"rental_price_cents": schema.Int64Attribute{
				Description: "Rental price in cents.",
				Optional:    true,
			},
			"purchase_price_cents": schema.Int64Attribute{
				Description: "Purchase price in cents.",
				Optional:    true,
			},
			"rental_duration_hours": schema.Int64Attribute{
				Description: "Rental duration in hours.",
				Optional:    true,
			},
			"territories": schema.ListAttribute{
				Description: "Available territories.",
				Optional:    true,
				ElementType: types.StringType,
			},
			"status": schema.StringAttribute{
				Description: "Product status.",
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
func (r *TvodProductResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan TvodProductResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Creating TVOD product", map[string]any{"title": plan.Title.ValueString()})

	plan.ID = types.StringValue("mock-tvod-product-id")
	plan.Status = types.StringValue("draft")
	plan.CreatedAt = types.StringValue("2024-01-01T00:00:00Z")
	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *TvodProductResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state TvodProductResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading TVOD product", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *TvodProductResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan TvodProductResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Updating TVOD product", map[string]any{"id": plan.ID.ValueString()})

	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *TvodProductResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state TvodProductResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Deleting TVOD product", map[string]any{"id": state.ID.ValueString()})
}

// TvodProductDataSource is the TVOD product data source
type TvodProductDataSource struct{}

// NewTvodProductDataSource creates a new TVOD product data source
func NewTvodProductDataSource() resource.DataSource {
	return &TvodProductDataSource{}
}

// Metadata returns the data source metadata
func (d *TvodProductDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_tvod_product"
}

// Schema returns the data source schema
func (d *TvodProductDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge TVOD product.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Product ID.",
				Required:    true,
			},
			"title": schema.StringAttribute{
				Description: "Product title.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Product status.",
				Computed:    true,
			},
			"tenant_id": schema.StringAttribute{
				Description: "Tenant ID.",
				Computed:    true,
			},
		},
	}
}

// Read reads the data source
func (d *TvodProductDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config TvodProductResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading TVOD product data source", map[string]any{"id": config.ID.ValueString()})

	config.Title = types.StringValue("Example Movie")
	config.Status = types.StringValue("active")
	config.TenantID = types.StringValue("tenant-123")

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}