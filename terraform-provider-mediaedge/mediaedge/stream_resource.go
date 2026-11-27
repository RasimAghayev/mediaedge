package mediaedge

import (
	"context"
	"fmt"

	"github.com/hashicorp/terraform-plugin-framework-validators/stringvalidator"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/planmodifier"
	"github.com/hashicorp/terraform-plugin-framework/resource/schema/stringplanmodifier"
	"github.com/hashicorp/terraform-plugin-framework/schema/validator"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/hashicorp/terraform-plugin-log/tflog"
)

// StreamResource is the stream resource
type StreamResource struct{}

// NewStreamResource creates a new stream resource
func NewStreamResource() resource.Resource {
	return &StreamResource{}
}

// StreamResourceModel maps the resource schema data
type StreamResourceModel struct {
	ID                 types.String `tfsdk:"id"`
	Name               types.String `tfsdk:"name"`
	Slug               types.String `tfsdk:"slug"`
	Status             types.String `tfsdk:"status"`
	TenantID           types.String `tfsdk:"tenant_id"`
	IngestProtocol     types.String `tfsdk:"ingest_protocol"`
	IngestURL          types.String `tfsdk:"ingest_url"`
	PlaybackHLS        types.String `tfsdk:"playback_hls"`
	PlaybackDASH       types.String `tfsdk:"playback_dash"`
	PlaybackWHEP       types.String `tfsdk:"playback_whep"`
	PlaybackMoQ        types.String `tfsdk:"playback_moq"`
	PlaybackWSFMP4     types.String `tfsdk:"playback_ws_fmp4"`
	TranscodingProfile types.String `tfsdk:"transcoding_profile"`
	RecordingEnabled   types.Bool   `tfsdk:"recording_enabled"`
	DVREnabled         types.Bool   `tfsdk:"dvr_enabled"`
	CreatedAt          types.String `tfsdk:"created_at"`
	UpdatedAt          types.String `tfsdk:"updated_at"`
}

// Metadata returns the resource metadata
func (r *StreamResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_stream"
}

// Schema returns the resource schema
func (r *StreamResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge stream.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Stream ID.",
				Computed:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.UseStateForUnknown(),
				},
			},
			"name": schema.StringAttribute{
				Description: "Stream name.",
				Required:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Stream slug (URL-friendly name).",
				Optional:    true,
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Stream status.",
				Computed:    true,
			},
			"tenant_id": schema.StringAttribute{
				Description: "Tenant ID.",
				Required:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.RequiresReplace(),
				},
			},
			"ingest_protocol": schema.StringAttribute{
				Description: "Ingest protocol (rtmp, srt, rist, whip, rtsp).",
				Required:    true,
				Validators: []validator.String{
					stringvalidator.OneOf("rtmp", "srt", "rist", "whip", "rtsp"),
				},
			},
			"ingest_url": schema.StringAttribute{
				Description: "Ingest URL.",
				Computed:    true,
			},
			"playback_hls": schema.StringAttribute{
				Description: "HLS playback URL.",
				Computed:    true,
			},
			"playback_dash": schema.StringAttribute{
				Description: "DASH playback URL.",
				Computed:    true,
			},
			"playback_whep": schema.StringAttribute{
				Description: "WHEP playback URL.",
				Computed:    true,
			},
			"playback_moq": schema.StringAttribute{
				Description: "MoQ playback URL.",
				Computed:    true,
			},
			"playback_ws_fmp4": schema.StringAttribute{
				Description: "WebSocket fMP4 playback URL.",
				Computed:    true,
			},
			"transcoding_profile": schema.StringAttribute{
				Description: "Transcoding profile name.",
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
func (r *StreamResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan StreamResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// TODO: Implement actual API call
	tflog.Info(ctx, "Creating stream", map[string]any{"name": plan.Name.ValueString()})

	// Mock response for now
	plan.ID = types.StringValue("mock-stream-id")
	plan.Status = types.StringValue("draft")
	plan.IngestURL = types.StringValue("rtmp://ingest.example.com/live/mock-stream-id")
	plan.PlaybackHLS = types.StringValue("https://edge.example.com/live/mock-stream-id.m3u8")
	plan.PlaybackDASH = types.StringValue("https://edge.example.com/live/mock-stream-id.mpd")
	plan.CreatedAt = types.StringValue("2024-01-01T00:00:00Z")
	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *StreamResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state StreamResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// TODO: Implement actual API call
	tflog.Info(ctx, "Reading stream", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *StreamResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan StreamResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// TODO: Implement actual API call
	tflog.Info(ctx, "Updating stream", map[string]any{"id": plan.ID.ValueString()})

	plan.UpdatedAt = types.StringValue("2024-01-01T00:00:00Z")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *StreamResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state StreamResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// TODO: Implement actual API call
	tflog.Info(ctx, "Deleting stream", map[string]any{"id": state.ID.ValueString()})
}

// StreamDataSource is the stream data source
type StreamDataSource struct{}

// NewStreamDataSource creates a new stream data source
func NewStreamDataSource() resource.DataSource {
	return &StreamDataSource{}
}

// Metadata returns the data source metadata
func (d *StreamDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_stream"
}

// Schema returns the data source schema
func (d *StreamDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge stream.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Stream ID.",
				Required:    true,
			},
			"name": schema.StringAttribute{
				Description: "Stream name.",
				Computed:    true,
			},
			"slug": schema.StringAttribute{
				Description: "Stream slug.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Stream status.",
				Computed:    true,
			},
			"tenant_id": schema.StringAttribute{
				Description: "Tenant ID.",
				Computed:    true,
			},
			"ingest_protocol": schema.StringAttribute{
				Description: "Ingest protocol.",
				Computed:    true,
			},
			"ingest_url": schema.StringAttribute{
				Description: "Ingest URL.",
				Computed:    true,
			},
			"playback_hls": schema.StringAttribute{
				Description: "HLS playback URL.",
				Computed:    true,
			},
			"playback_dash": schema.StringAttribute{
				Description: "DASH playback URL.",
				Computed:    true,
			},
		},
	}
}

// Read reads the data source
func (d *StreamDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config StreamResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// TODO: Implement actual API call
	tflog.Info(ctx, "Reading stream data source", map[string]any{"id": config.ID.ValueString()})

	config.Name = types.StringValue("Example Stream")
	config.Slug = types.StringValue("example-stream")
	config.Status = types.StringValue("live")
	config.TenantID = types.StringValue("tenant-123")
	config.IngestProtocol = types.StringValue("rtmp")
	config.IngestURL = types.StringValue("rtmp://ingest.example.com/live/example-stream")
	config.PlaybackHLS = types.StringValue("https://edge.example.com/live/example-stream.m3u8")
	config.PlaybackDASH = types.StringValue("https://edge.example.com/live/example-stream.mpd")

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}