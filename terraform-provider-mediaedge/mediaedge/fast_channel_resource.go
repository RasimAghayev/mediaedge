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

// FastChannelResource is the FAST channel resource
type FastChannelResource struct{}

// NewFastChannelResource creates a new FAST channel resource
func NewFastChannelResource() resource.Resource {
	return &FastChannelResource{}
}

// FastChannelResourceModel maps the resource schema data
type FastChannelResourceModel struct {
	ID             types.String `tfsdk:"id"`
	TenantID       types.String `tfsdk:"tenant_id"`
	Name           types.String `tfsdk:"name"`
	Slug           types.String `tfsdk:"slug"`
	Status         types.String `tfsdk:"status"`
	Timezone       types.String `tfsdk:"timezone"`
	AdBreaksEnabled types.Bool   `tfsdk:"ad_breaks_enabled"`
	AdProvider     types.String `tfsdk:"ad_provider"`
	AdTagURL       types.String `tfsdk:"ad_tag_url"`
	OutputFormats  types.List   `tfsdk:"output_formats"`
	CreatedAt      types.String `tfsdk:"created_at"`
	UpdatedAt      types.String `tfsdk:"updated_at"`
}

// Metadata returns the resource metadata
func (r *FastChannelResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_fast_channel"
}

// Schema returns the resource schema
func (r *FastChannelResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge FAST channel.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Channel ID.",
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
				Description: "Channel name.",
				Required:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Channel slug.",
				Optional:    true,
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Channel status.",
				Computed:    true,
			},
			"timezone": schema.StringAttribute{
				Description: "Channel timezone.",
				Required:    true,
			},
			"ad_breaks_enabled": schema.BoolAttribute{
				Description: "Whether ad breaks are enabled.",
				Optional:    true,
				Computed:    true,
			},
			"ad_provider": schema.StringAttribute{
				Description: "Ad provider (mediatailor, yospace, broadpeak, google_ad_manager, prebid).",
				Optional:    true,
				Validators: []validator.String{
					stringvalidator.OneOf("mediatailor", "yospace", "broadpeak", "google_ad_manager", "prebid"),
				},
			},
			"ad_tag_url": schema.StringAttribute{
				Description: "Ad tag URL.",
				Optional:    true,
			},
			"output_formats": schema.ListAttribute{
				Description: "Output formats.",
				Optional:    true,
				ElementType: types.StringType,
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
func (r *FastChannelResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan FastChannelResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Creating FAST channel", map[string]any{"name": plan.Name.ValueString()})

	plan.ID = types.StringValue("mock-fast-channel-id")
	plan.Status = types.StringValue("draft")
	plan.CreatedAt = types.StringValue("2024-01-01T00:00:00Z")
	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *FastChannelResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state FastChannelResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading FAST channel", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *FastChannelResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan FastChannelResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Updating FAST channel", map[string]any{"id": plan.ID.ValueString()})

	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *FastChannelResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state FastChannelResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Deleting FAST channel", map[string]any{"id": state.ID.ValueString()})
}

// FastChannelDataSource is the FAST channel data source
type FastChannelDataSource struct{}

// NewFastChannelDataSource creates a new FAST channel data source
func NewFastChannelDataSource() resource.DataSource {
	return &FastChannelDataSource{}
}

// Metadata returns the data source metadata
func (d *FastChannelDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_fast_channel"
}

// Schema returns the data source schema
func (d *FastChannelDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge FAST channel.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Channel ID.",
				Required:    true,
			},
			"name": schema.StringAttribute{
				Description: "Channel name.",
				Computed:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Channel slug.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Channel status.",
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
func (d *FastChannelDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config FastChannelResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading FAST channel data source", map[string]any{"id": config.ID.ValueString()})

	config.Name = types.StringValue("Example FAST Channel")
	config.Slug = types.StringValue("example-fast-channel")
	config.Status = types.StringValue("active")
	config.TenantID = types.StringValue("tenant-123")

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}