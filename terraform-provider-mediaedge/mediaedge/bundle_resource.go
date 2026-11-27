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

// BundleResource is the bundle resource
type BundleResource struct{}

// NewBundleResource creates a new bundle resource
func NewBundleResource() resource.Resource {
	return &BundleResource{}
}

// BundleResourceModel maps the resource schema data
type BundleResourceModel struct {
	ID              types.String `tfsdk:"id"`
	TenantID        types.String `tfsdk:"tenant_id"`
	Name            types.String `tfsdk:"name"`
	Description     types.String `tfsdk:"description"`
	BundleType      types.String `tfsdk:"bundle_type"`
	BasePriceCents  types.Int64  `tfsdk:"base_price_cents"`
	BillingCycle    types.String `tfsdk:"billing_cycle"`
	TrialDays       types.Int64  `tfsdk:"trial_days"`
	SetupFeeCents   types.Int64  `tfsdk:"setup_fee_cents"`
	Status          types.String `tfsdk:"status"`
	CreatedAt       types.String `tfsdk:"created_at"`
	UpdatedAt       types.String `tfsdk:"updated_at"`
}

// Metadata returns the resource metadata
func (r *BundleResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_bundle"
}

// Schema returns the resource schema
func (r *BundleResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge bundle.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Bundle ID.",
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
			"name": schema.StringAttribute{
				Description: "Bundle name.",
				Required:    true,
			},
			"description": schema.StringAttribute{
				Description: "Bundle description.",
				Optional:    true,
			},
			"bundle_type": schema.StringAttribute{
				Description: "Bundle type (streamer_bundle, telco_bundle, aggregator_bundle).",
				Required:    true,
				Validators: []validator.String{
					stringvalidator.OneOf("streamer_bundle", "telco_bundle", "aggregator_bundle"),
				},
			},
			"base_price_cents": schema.Int64Attribute{
				Description: "Base price in cents.",
				Required:    true,
			},
			"billing_cycle": schema.StringAttribute{
				Description: "Billing cycle (monthly, quarterly, yearly).",
				Required:    true,
				Validators: []validator.String{
					stringvalidator.OneOf("monthly", "quarterly", "yearly"),
				},
			},
			"trial_days": schema.Int64Attribute{
				Description: "Trial period in days.",
				Optional:    true,
			},
			"setup_fee_cents": schema.Int64Attribute{
				Description: "Setup fee in cents.",
				Optional:    true,
			},
			"status": schema.StringAttribute{
				Description: "Bundle status.",
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
func (r *BundleResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan BundleResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Creating bundle", map[string]any{"name": plan.Name.ValueString()})

	plan.ID = types.StringValue("mock-bundle-id")
	plan.Status = types.StringValue("draft")
	plan.CreatedAt = types.StringValue("2024-01-01T00:00:00Z")
	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *BundleResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state BundleResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading bundle", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *BundleResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan BundleResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Updating bundle", map[string]any{"id": plan.ID.ValueString()})

	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *BundleResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state BundleResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Deleting bundle", map[string]any{"id": state.ID.ValueString()})
}

// BundleDataSource is the bundle data source
type BundleDataSource struct{}

// NewBundleDataSource creates a new bundle data source
func NewBundleDataSource() resource.DataSource {
	return &BundleDataSource{}
}

// Metadata returns the data source metadata
func (d *BundleDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_bundle"
}

// Schema returns the data source schema
func (d *BundleDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge bundle.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Bundle ID.",
				Required:    true,
			},
			"name": schema.StringAttribute{
				Description: "Bundle name.",
				Computed:    true,
			},
			"bundle_type": schema.StringAttribute{
				Description: "Bundle type.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Bundle status.",
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
func (d *BundleDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config BundleResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading bundle data source", map[string]any{"id": config.ID.ValueString()})

	config.Name = types.StringValue("Example Bundle")
	config.BundleType = types.StringValue("streamer_bundle")
	config.Status = types.StringValue("active")
	config.TenantID = types.StringValue("tenant-123")

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}