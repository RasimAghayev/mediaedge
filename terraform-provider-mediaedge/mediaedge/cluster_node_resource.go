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

// ClusterNodeResource is the cluster node resource
type ClusterNodeResource struct{}

// NewClusterNodeResource creates a new cluster node resource
func NewClusterNodeResource() resource.Resource {
	return &ClusterNodeResource{}
}

// ClusterNodeResourceModel maps the resource schema data
type ClusterNodeResourceModel struct {
	ID          types.String `tfsdk:"id"`
	Name        types.String `tfsdk:"name"`
	Endpoint    types.String `tfsdk:"endpoint"`
	Region      types.String `tfsdk:"region"`
	Zone        types.String `tfsdk:"zone"`
	Status      types.String `tfsdk:"status"`
	Roles       types.List   `tfsdk:"roles"`
	CPUCores    types.Int64  `tfsdk:"cpu_cores"`
	MemoryGB    types.Int64  `tfsdk:"memory_gb"`
	StorageGB   types.Int64  `tfsdk:"storage_gb"`
	NetworkGbps types.Int64  `tfsdk:"network_gbps"`
	MaxStreams  types.Int64  `tfsdk:"max_streams"`
	MaxViewers  types.Int64  `tfsdk:"max_viewers"`
	Version     types.String `tfsdk:"version"`
}

// Metadata returns the resource metadata
func (r *ClusterNodeResource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_cluster_node"
}

// Schema returns the resource schema
func (r *ClusterNodeResource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Manages a MediaEdge cluster node.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Node ID.",
				Computed:    true,
				PlanModifiers: []planmodifier.String{
					stringplanmodifier.UseStateForUnknown(),
				},
			},
			"name": schema.StringAttribute{
				Description: "Node name.",
				Required:    true,
			},
			"endpoint": schema.StringAttribute{
				Description: "Node endpoint.",
				Required:    true,
			},
			"region": schema.StringAttribute{
				Description: "Node region.",
				Required:    true,
			},
			"zone": schema.StringAttribute{
				Description: "Node zone.",
				Required:    true,
			},
			"status": schema.StringAttribute{
				Description: "Node status.",
				Computed:    true,
			},
			"roles": schema.ListAttribute{
				Description: "Node roles.",
				Required:    true,
				ElementType: types.StringType,
			},
			"cpu_cores": schema.Int64Attribute{
				Description: "CPU cores.",
				Required:    true,
			},
			"memory_gb": schema.Int64Attribute{
				Description: "Memory in GB.",
				Required:    true,
			},
			"storage_gb": schema.Int64Attribute{
				Description: "Storage in GB.",
				Required:    true,
			},
			"network_gbps": schema.Int64Attribute{
				Description: "Network in Gbps.",
				Required:    true,
			},
			"max_streams": schema.Int64Attribute{
				Description: "Maximum streams.",
				Required:    true,
			},
			"max_viewers": schema.Int64Attribute{
				Description: "Maximum viewers.",
				Required:    true,
			},
			"version": schema.StringAttribute{
				Description: "Node version.",
				Computed:    true,
			},
		},
	}
}

// Create creates the resource
func (r *ClusterNodeResource) Create(ctx context.Context, req resource.CreateRequest, resp *resource.CreateResponse) {
	var plan ClusterNodeResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Creating cluster node", map[string]any{"name": plan.Name.ValueString()})

	plan.ID = types.StringValue("mock-node-id")
	plan.Status = types.StringValue("healthy")
	plan.Version = types.StringValue("1.0.0")

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Read reads the resource
func (r *ClusterNodeResource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var state ClusterNodeResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading cluster node", map[string]any{"id": state.ID.ValueString()})
}

// Update updates the resource
func (r *ClusterNodeResource) Update(ctx context.Context, req resource.UpdateRequest, resp *resource.UpdateResponse) {
	var plan ClusterNodeResourceModel
	diags := req.Plan.Get(ctx, &plan)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Updating cluster node", map[string]any{"id": plan.ID.ValueString()})

	diags = resp.State.Set(ctx, plan)
	resp.Diagnostics.Append(diags...)
}

// Delete deletes the resource
func (r *ClusterNodeResource) Delete(ctx context.Context, req resource.DeleteRequest, resp *resource.DeleteResponse) {
	var state ClusterNodeResourceModel
	diags := req.State.Get(ctx, &state)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Deleting cluster node", map[string]any{"id": state.ID.ValueString()})
}

// ClusterNodeDataSource is the cluster node data source
type ClusterNodeDataSource struct{}

// NewClusterNodeDataSource creates a new cluster node data source
func NewClusterNodeDataSource() resource.DataSource {
	return &ClusterNodeDataSource{}
}

// Metadata returns the data source metadata
func (d *ClusterNodeDataSource) Metadata(_ context.Context, req resource.MetadataRequest, resp *resource.MetadataResponse) {
	resp.TypeName = req.ProviderTypeName + "_cluster_node"
}

// Schema returns the data source schema
func (d *ClusterNodeDataSource) Schema(_ context.Context, _ resource.SchemaRequest, resp *resource.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "Fetches a MediaEdge cluster node.",
		Attributes: map[string]schema.Attribute{
			"id": schema.StringAttribute{
				Description: "Node ID.",
				Required:    true,
			},
			"name": schema.StringAttribute{
				Description: "Node name.",
				Computed:    true,
			},
			"endpoint": schema.StringAttribute{
				Description: "Node endpoint.",
				Computed:    true,
			},
			"region": schema.StringAttribute{
				Description: "Node region.",
				Computed:    true,
			},
			"zone": schema.StringAttribute{
				Description: "Node zone.",
				Computed:    true,
			},
			"status": schema.StringAttribute{
				Description: "Node status.",
				Computed:    true,
			},
		},
	}
}

// Read reads the data source
func (d *ClusterNodeDataSource) Read(ctx context.Context, req resource.ReadRequest, resp *resource.ReadResponse) {
	var config ClusterNodeResourceModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	tflog.Info(ctx, "Reading cluster node data source", map[string]any{"id": config.ID.ValueString()})

	config.Name = types.StringValue("edge-node-1")
	config.Endpoint = types.StringValue("https://edge1.example.com")
	config.Region = types.StringValue("us-east-1")
	config.Zone = types.StringValue("us-east-1a")
	config.Status = types.StringValue("healthy")

	diags = resp.State.Set(ctx, config)
	resp.Diagnostics.Append(diags...)
}