package mediaedge

import (
	"context"
	"fmt"

	"github.com/hashicorp/terraform-plugin-framework/datasource"
	"github.com/hashicorp/terraform-plugin-framework/path"
	"github.com/hashicorp/terraform-plugin-framework/provider"
	"github.com/hashicorp/terraform-plugin-framework/provider/schema"
	"github.com/hashicorp/terraform-plugin-framework/resource"
	"github.com/hashicorp/terraform-plugin-framework/types"
	"github.com/hashicorp/terraform-plugin-log/tflog"
)

// Provider is the Terraform provider for MediaEdge
type Provider struct {
	version string
}

// ProviderModel maps provider schema data to a Go type
type ProviderModel struct {
	Endpoint      types.String `tfsdk:"endpoint"`
	AccessToken   types.String `tfsdk:"access_token"`
	RefreshToken  types.String `tfsdk:"refresh_token"`
}

// NewProvider creates a new provider instance
func NewProvider(version string) func() provider.Provider {
	return func() provider.Provider {
		return &Provider{version: version}
	}
}

// Metadata returns the provider metadata
func (p *Provider) Metadata(_ context.Context, _ provider.MetadataRequest, resp *provider.MetadataResponse) {
	resp.TypeName = "mediaedge"
	resp.Version = p.version
}

// Schema returns the provider schema
func (p *Provider) Schema(_ context.Context, _ provider.SchemaRequest, resp *provider.SchemaResponse) {
	resp.Schema = schema.Schema{
		Description: "MediaEdge provider for managing MediaEdge resources.",
		Attributes: map[string]schema.Attribute{
			"endpoint": schema.StringAttribute{
				Description: "MediaEdge API endpoint URL.",
				Optional:    true,
			},
			"access_token": schema.StringAttribute{
				Description: "Access token for authentication.",
				Optional:    true,
				Sensitive:   true,
			},
			"refresh_token": schema.StringAttribute{
				Description: "Refresh token for authentication.",
				Optional:    true,
				Sensitive:   true,
			},
		},
	}
}

// Configure configures the provider
func (p *Provider) Configure(ctx context.Context, req provider.ConfigureRequest, resp *provider.ConfigureResponse) {
	tflog.Info(ctx, "Configuring MediaEdge provider")

	var config ProviderModel
	diags := req.Config.Get(ctx, &config)
	resp.Diagnostics.Append(diags...)
	if resp.Diagnostics.HasError() {
		return
	}

	// Create client configuration
	clientConfig := map[string]interface{}{}

	if !config.Endpoint.IsNull() && !config.Endpoint.IsUnknown() {
		clientConfig["endpoint"] = config.Endpoint.ValueString()
	}

	if !config.AccessToken.IsNull() && !config.AccessToken.IsUnknown() {
		clientConfig["access_token"] = config.AccessToken.ValueString()
	}

	if !config.RefreshToken.IsNull() && !config.RefreshToken.IsUnknown() {
		clientConfig["refresh_token"] = config.RefreshToken.ValueString()
	}

	// Store client config in provider data
	resp.DataSourceData = clientConfig
	resp.ResourceData = clientConfig
}

// DataSources returns the data sources
func (p *Provider) DataSources(_ context.Context) []func() datasource.DataSource {
	return []func() datasource.DataSource{
		NewStreamDataSource,
		NewClusterNodeDataSource,
		NewTenantDataSource,
		NewFastChannelDataSource,
		NewTvodProductDataSource,
		NewBundleDataSource,
	}
}

// Resources returns the resources
func (p *Provider) Resources(_ context.Context) []func() resource.Resource {
	return []func() resource.Resource{
		NewStreamResource,
		NewClusterNodeResource,
		NewTenantResource,
		NewFastChannelResource,
		NewTvodProductResource,
		NewBundleResource,
	}
}

// ClientConfig holds client configuration
type ClientConfig struct {
	Endpoint     string
	AccessToken  string
	RefreshToken string
}

// GetClientConfig retrieves client config from provider data
func GetClientConfig(data interface{}) (*ClientConfig, error) {
	config, ok := data.(map[string]interface{})
	if !ok {
		return nil, fmt.Errorf("invalid provider data")
	}

	clientConfig := &ClientConfig{}
	if v, ok := config["endpoint"].(string); ok {
		clientConfig.Endpoint = v
	}
	if v, ok := config["access_token"].(string); ok {
		clientConfig.AccessToken = v
	}
	if v, ok := config["refresh_token"].(string); ok {
		clientConfig.RefreshToken = v
	}

	return clientConfig, nil
}

// Ensure provider implements required interfaces
var (
	_ provider.Provider = &Provider{}
)