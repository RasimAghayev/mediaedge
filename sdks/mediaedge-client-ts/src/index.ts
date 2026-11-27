import axios, { AxiosInstance, AxiosRequestConfig, AxiosResponse } from 'axios';
import { z } from 'zod';

// Base URL from environment or default
const DEFAULT_BASE_URL = 'http://localhost:8080';

// Zod schemas for validation
export const StreamStatusSchema = z.enum(['draft', 'starting', 'live', 'stopping', 'stopped', 'error', 'archived']);
export type StreamStatus = z.infer<typeof StreamStatusSchema>;

export const IngestProtocolSchema = z.enum(['rtmp', 'srt', 'rist', 'whip', 'rtsp']);
export type IngestProtocol = z.infer<typeof IngestProtocolSchema>;

export const NodeStatusSchema = z.enum(['healthy', 'degraded', 'unhealthy', 'draining', 'offline']);
export type NodeStatus = z.infer<typeof NodeStatusSchema>;

export const TenantStatusSchema = z.enum(['active', 'trial', 'suspended', 'deleted']);
export type TenantStatus = z.infer<typeof TenantStatusSchema>;

export const UserStatusSchema = z.enum(['active', 'invited', 'suspended', 'deleted']);
export type UserStatus = z.infer<typeof UserStatusSchema>;

export const FastChannelStatusSchema = z.enum(['draft', 'active', 'paused', 'archived']);
export type FastChannelStatus = z.infer<typeof FastChannelStatusSchema>;

export const TvodStatusSchema = z.enum(['draft', 'active', 'hidden', 'archived']);
export type TvodStatus = z.infer<typeof TvodStatusSchema>;

export const BundleTypeSchema = z.enum(['streamer_bundle', 'telco_bundle', 'aggregator_bundle']);
export type BundleType = z.infer<typeof BundleTypeSchema>;

export const BundleStatusSchema = z.enum(['draft', 'active', 'paused', 'archived']);
export type BundleStatus = z.infer<typeof BundleStatusSchema>;

export const InvoiceStatusSchema = z.enum(['draft', 'open', 'paid', 'void', 'uncollectible']);
export type InvoiceStatus = z.infer<typeof InvoiceStatusSchema>;

// Models
export interface PlaybackUrls {
  hls?: string;
  dash?: string;
  whep?: string;
  moq?: string;
  websocket_fmp4?: string;
}

export interface Stream {
  id: string;
  name: string;
  slug: string;
  status: StreamStatus;
  tenant_id: string;
  ingest_protocol: IngestProtocol;
  ingest_url: string;
  playback_urls: PlaybackUrls;
  transcoding_profile?: string;
  recording_enabled: boolean;
  dvr_enabled: boolean;
  created_at: string;
  updated_at: string;
  started_at?: string;
  ended_at?: string;
}

export interface CreateStreamRequest {
  name: string;
  slug?: string;
  ingest_protocol: IngestProtocol;
  transcoding_profile?: string;
  recording_enabled?: boolean;
  dvr_enabled?: boolean;
}

export interface UpdateStreamRequest {
  name?: string;
  transcoding_profile?: string;
  recording_enabled?: boolean;
  dvr_enabled?: boolean;
}

export interface StreamMetrics {
  stream_id: string;
  viewers_current: number;
  viewers_peak: number;
  bitrate_in: number;
  bitrate_out: number;
  bytes_in: number;
  bytes_out: number;
  packets_lost: number;
  rtt_ms?: number;
  timestamp: string;
}

export interface ClusterNode {
  id: string;
  name: string;
  endpoint: string;
  region: string;
  zone: string;
  capacity: NodeCapacity;
  load: NodeLoad;
  status: NodeStatus;
  roles: string[];
  last_heartbeat: string;
  version: string;
}

export interface NodeCapacity {
  cpu_cores: number;
  memory_gb: number;
  storage_gb: number;
  network_gbps: number;
  max_streams: number;
  max_viewers: number;
}

export interface NodeLoad {
  cpu_percent: number;
  memory_percent: number;
  storage_percent: number;
  network_percent: number;
  active_streams: number;
  active_viewers: number;
}

export interface Broadcast {
  id: string;
  stream_id: string;
  owner_node_id: string;
  status: string;
  viewers: number;
  started_at: string;
  updated_at: string;
}

export interface Tenant {
  id: string;
  name: string;
  slug: string;
  status: TenantStatus;
  quotas: TenantQuotas;
  branding?: TenantBranding;
  settings: TenantSettings;
  created_at: string;
  updated_at: string;
}

export interface TenantQuotas {
  max_streams: number;
  max_concurrent_viewers: number;
  max_storage_gb: number;
  max_bandwidth_gbps: number;
  max_transcoding_hours: number;
  custom_domains: number;
}

export interface TenantBranding {
  logo_url?: string;
  favicon_url?: string;
  primary_color?: string;
  secondary_color?: string;
  custom_css?: string;
}

export interface TenantSettings {
  allowed_protocols: IngestProtocol[];
  default_transcoding_profile?: string;
  recording_enabled: boolean;
  dvr_enabled: boolean;
  geo_blocking?: GeoBlocking;
  drm_enabled: boolean;
}

export interface GeoBlocking {
  allowed_countries?: string[];
  blocked_countries?: string[];
}

export interface CreateTenantRequest {
  name: string;
  slug?: string;
  quotas?: Partial<TenantQuotas>;
  branding?: Partial<TenantBranding>;
  settings?: Partial<TenantSettings>;
}

export interface FastChannel {
  id: string;
  tenant_id: string;
  name: string;
  slug: string;
  status: FastChannelStatus;
  schedule: FastChannelSchedule;
  ad_breaks: AdBreakConfig;
  output_formats: string[];
  created_at: string;
  updated_at: string;
}

export interface FastChannelSchedule {
  timezone: string;
  entries: ScheduleEntry[];
}

export interface ScheduleEntry {
  content_id: string;
  start_time: string;
  duration_seconds: number;
  repeat?: RepeatRule;
}

export interface RepeatRule {
  frequency: 'daily' | 'weekly' | 'monthly';
  interval: number;
  until?: string;
}

export interface AdBreakConfig {
  enabled: boolean;
  provider: 'mediatailor' | 'yospace' | 'broadpeak' | 'google_ad_manager' | 'prebid';
  tag_url?: string;
  min_interval_seconds: number;
  max_duration_seconds: number;
}

export interface TvodProduct {
  id: string;
  tenant_id: string;
  title: string;
  description?: string;
  poster_url?: string;
  trailer_url?: string;
  content_id: string;
  rental_price_cents?: number;
  purchase_price_cents?: number;
  rental_duration_hours?: number;
  purchase_availability?: AvailabilityWindow;
  rental_availability?: AvailabilityWindow;
  territories: string[];
  status: TvodStatus;
  metadata: Record<string, any>;
  created_at: string;
  updated_at: string;
}

export interface AvailabilityWindow {
  start: string;
  end?: string;
}

export interface Bundle {
  id: string;
  tenant_id: string;
  name: string;
  description?: string;
  bundle_type: BundleType;
  pricing: BundlePricing;
  included_products: BundleProduct[];
  partner_validation?: PartnerValidation;
  status: BundleStatus;
  created_at: string;
  updated_at: string;
}

export interface BundlePricing {
  base_price_cents: number;
  billing_cycle: 'monthly' | 'quarterly' | 'yearly';
  trial_days?: number;
  setup_fee_cents?: number;
}

export interface BundleProduct {
  product_id: string;
  product_type: 'fast_channel' | 'tvod_product' | 'live_stream' | 'storage' | 'transcoding';
  quantity: number;
}

export interface PartnerValidation {
  partner_id: string;
  validation_type: string;
  config: Record<string, any>;
}

export interface EntitlementCheck {
  user_id: string;
  content_id: string;
  content_type: 'stream' | 'vod' | 'fast_channel' | 'bundle';
  grant_type?: 'subscription' | 'rental' | 'purchase' | 'bundle' | 'promo';
  expires_at?: string;
}

export interface EntitlementResult {
  granted: boolean;
  reason?: string;
  entitlement?: Entitlement;
}

export interface Entitlement {
  id: string;
  user_id: string;
  content_id: string;
  content_type: string;
  grant_type: string;
  granted_at: string;
  expires_at?: string;
}

export interface Invoice {
  id: string;
  tenant_id: string;
  customer_id: string;
  amount_cents: number;
  currency: string;
  status: InvoiceStatus;
  line_items: InvoiceLineItem[];
  created_at: string;
  due_date: string;
  paid_at?: string;
}

export interface InvoiceLineItem {
  description: string;
  quantity: number;
  unit_price_cents: number;
  amount_cents: number;
}

export interface User {
  id: string;
  email: string;
  name: string;
  roles: string[];
  mfa_enabled: boolean;
  status: UserStatus;
  tenant_id: string;
  tenant_name: string;
  last_login_at?: string;
  created_at: string;
  updated_at: string;
}

export interface LoginRequest {
  email: string;
  password: string;
  mfa_code?: string;
}

export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  user: User;
  tenant: Tenant;
  requires_mfa: boolean;
}

export interface InviteUserRequest {
  email: string;
  name: string;
  roles: string[];
  tenant_id: string;
}

export interface PaginatedResponse<T> {
  data: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface ListParams {
  page?: number;
  per_page?: number;
  search?: string;
  sort?: string;
  order?: 'asc' | 'desc';
}

export interface MetricsQuery {
  metric: string;
  start: string;
  end: string;
  step?: string;
  labels?: Record<string, any>;
}

export interface MetricsResult {
  metric: string;
  data: MetricsDataPoint[];
}

export interface MetricsDataPoint {
  timestamp: string;
  value: number;
  labels: Record<string, any>;
}

export interface ClusterHealth {
  status: string;
  nodes_total: number;
  nodes_healthy: number;
  nodes_degraded: number;
  nodes_offline: number;
  streams_active: number;
  viewers_total: number;
}

// Error class
export class MediaEdgeError extends Error {
  constructor(
    message: string,
    public readonly code?: number,
    public readonly details?: any
  ) {
    super(message);
    this.name = 'MediaEdgeError';
  }

  static fromResponse(response: AxiosResponse): MediaEdgeError {
    const data = response.data;
    return new MediaEdgeError(
      data?.message || `HTTP ${response.status}`,
      response.status,
      data?.details
    );
  }
}

// Client configuration
export interface ClientConfig {
  baseURL?: string;
  accessToken?: string;
  refreshToken?: string;
  timeout?: number;
  headers?: Record<string, string>;
}

// Main client class
export class MediaEdgeClient {
  private axios: AxiosInstance;
  private accessToken?: string;
  private refreshToken?: string;

  constructor(config: ClientConfig = {}) {
    this.accessToken = config.accessToken;
    this.refreshToken = config.refreshToken;

    this.axios = axios.create({
      baseURL: config.baseURL || DEFAULT_BASE_URL,
      timeout: config.timeout || 30000,
      headers: {
        'Content-Type': 'application/json',
        ...config.headers,
      },
    });

    // Request interceptor for auth
    this.axios.interceptors.request.use((config) => {
      if (this.accessToken) {
        config.headers.Authorization = `Bearer ${this.accessToken}`;
      }
      return config;
    });

    // Response interceptor for token refresh
    this.axios.interceptors.response.use(
      (response) => response,
      async (error) => {
        const originalRequest = error.config;
        if (error.response?.status === 401 && !originalRequest._retry && this.refreshToken) {
          originalRequest._retry = true;
          try {
            const response = await this.refreshAccessToken();
            this.setAccessToken(response.access_token);
            originalRequest.headers.Authorization = `Bearer ${this.accessToken}`;
            return this.axios(originalRequest);
          } catch {
            // Refresh failed, redirect to login
            this.clearTokens();
            throw error;
          }
        }
        throw error;
      }
    );
  }

  // Token management
  setAccessToken(token: string) {
    this.accessToken = token;
  }

  setRefreshToken(token: string) {
    this.refreshToken = token;
  }

  clearTokens() {
    this.accessToken = undefined;
    this.refreshToken = undefined;
  }

  getAccessToken(): string | undefined {
    return this.accessToken;
  }

  // Generic request methods
  private async request<T>(config: AxiosRequestConfig): Promise<T> {
    try {
      const response = await this.axios.request<T>(config);
      return response.data;
    } catch (error: any) {
      if (error.response) {
        throw MediaEdgeError.fromResponse(error.response);
      }
      throw new MediaEdgeError(error.message || 'Network error');
    }
  }

  async get<T>(url: string, params?: Record<string, any>): Promise<T> {
    return this.request<T>({ method: 'GET', url, params });
  }

  async post<T>(url: string, data?: any): Promise<T> {
    return this.request<T>({ method: 'POST', url, data });
  }

  async patch<T>(url: string, data?: any): Promise<T> {
    return this.request<T>({ method: 'PATCH', url, data });
  }

  async delete<T>(url: string): Promise<T> {
    return this.request<T>({ method: 'DELETE', url });
  }

  // Auth methods
  async login(credentials: LoginRequest): Promise<LoginResponse> {
    const response = await this.post<LoginResponse>('/api/v1/auth/login', credentials);
    this.setAccessToken(response.access_token);
    this.setRefreshToken(response.refresh_token);
    return response;
  }

  async refreshAccessToken(): Promise<LoginResponse> {
    if (!this.refreshToken) {
      throw new MediaEdgeError('No refresh token available');
    }
    const response = await this.post<LoginResponse>('/api/v1/auth/refresh', {
      refresh_token: this.refreshToken,
    });
    this.setAccessToken(response.access_token);
    this.setRefreshToken(response.refresh_token);
    return response;
  }

  async logout(): Promise<void> {
    await this.post('/api/v1/auth/logout');
    this.clearTokens();
  }

  async getCurrentUser(): Promise<User> {
    return this.get<User>('/api/v1/auth/me');
  }

  // Streams API
  async listStreams(params?: ListParams): Promise<PaginatedResponse<Stream>> {
    return this.get<PaginatedResponse<Stream>>('/api/v1/streams', params);
  }

  async getStream(id: string): Promise<Stream> {
    return this.get<Stream>(`/api/v1/streams/${id}`);
  }

  async createStream(data: CreateStreamRequest): Promise<Stream> {
    return this.post<Stream>('/api/v1/streams', data);
  }

  async updateStream(id: string, data: UpdateStreamRequest): Promise<Stream> {
    return this.patch<Stream>(`/api/v1/streams/${id}`, data);
  }

  async deleteStream(id: string): Promise<void> {
    return this.delete(`/api/v1/streams/${id}`);
  }

  async startStream(id: string): Promise<Stream> {
    return this.post<Stream>(`/api/v1/streams/${id}/start`);
  }

  async stopStream(id: string): Promise<Stream> {
    return this.post<Stream>(`/api/v1/streams/${id}/stop`);
  }

  async getStreamMetrics(id: string, params?: { start?: string; end?: string; interval?: string }): Promise<StreamMetrics[]> {
    return this.get<StreamMetrics[]>(`/api/v1/streams/${id}/metrics`, params);
  }

  async getStreamPlaybackUrls(id: string): Promise<PlaybackUrls> {
    return this.get<PlaybackUrls>(`/api/v1/streams/${id}/playback`);
  }

  // Cluster API
  async listNodes(params?: ListParams): Promise<PaginatedResponse<ClusterNode>> {
    return this.get<PaginatedResponse<ClusterNode>>('/api/v1/cluster/nodes', params);
  }

  async getNode(id: string): Promise<ClusterNode> {
    return this.get<ClusterNode>(`/api/v1/cluster/nodes/${id}`);
  }

  async listBroadcasts(params?: ListParams): Promise<PaginatedResponse<Broadcast>> {
    return this.get<PaginatedResponse<Broadcast>>('/api/v1/cluster/broadcasts', params);
  }

  async getClusterHealth(): Promise<ClusterHealth> {
    return this.get<ClusterHealth>('/api/v1/cluster/health');
  }

  async getFederationStatus(): Promise<any> {
    return this.get('/api/v1/cluster/federation');
  }

  async getMeshStatus(): Promise<any> {
    return this.get('/api/v1/cluster/mesh');
  }

  async syncConfig(): Promise<any> {
    return this.post('/api/v1/cluster/config/sync');
  }

  // Tenants API
  async listTenants(params?: ListParams): Promise<PaginatedResponse<Tenant>> {
    return this.get<PaginatedResponse<Tenant>>('/api/v1/tenants', params);
  }

  async getTenant(id: string): Promise<Tenant> {
    return this.get<Tenant>(`/api/v1/tenants/${id}`);
  }

  async createTenant(data: CreateTenantRequest): Promise<Tenant> {
    return this.post<Tenant>('/api/v1/tenants', data);
  }

  async updateTenant(id: string, data: Partial<CreateTenantRequest>): Promise<Tenant> {
    return this.patch<Tenant>(`/api/v1/tenants/${id}`, data);
  }

  async deleteTenant(id: string): Promise<void> {
    return this.delete(`/api/v1/tenants/${id}`);
  }

  async getTenantQuotas(id: string): Promise<TenantQuotas> {
    return this.get<TenantQuotas>(`/api/v1/tenants/${id}/quotas`);
  }

  async updateTenantQuotas(id: string, quotas: TenantQuotas): Promise<TenantQuotas> {
    return this.patch<TenantQuotas>(`/api/v1/tenants/${id}/quotas`, quotas);
  }

  // Monetization API
  async listFastChannels(params?: ListParams): Promise<PaginatedResponse<FastChannel>> {
    return this.get<PaginatedResponse<FastChannel>>('/api/v1/monetization/fast-channels', params);
  }

  async getFastChannel(id: string): Promise<FastChannel> {
    return this.get<FastChannel>(`/api/v1/monetization/fast-channels/${id}`);
  }

  async createFastChannel(data: any): Promise<FastChannel> {
    return this.post<FastChannel>('/api/v1/monetization/fast-channels', data);
  }

  async listTvodProducts(params?: ListParams): Promise<PaginatedResponse<TvodProduct>> {
    return this.get<PaginatedResponse<TvodProduct>>('/api/v1/monetization/tvod/products', params);
  }

  async getTvodProduct(id: string): Promise<TvodProduct> {
    return this.get<TvodProduct>(`/api/v1/monetization/tvod/products/${id}`);
  }

  async createTvodProduct(data: any): Promise<TvodProduct> {
    return this.post<TvodProduct>('/api/v1/monetization/tvod/products', data);
  }

  async listBundles(params?: ListParams): Promise<PaginatedResponse<Bundle>> {
    return this.get<PaginatedResponse<Bundle>>('/api/v1/monetization/bundles', params);
  }

  async getBundle(id: string): Promise<Bundle> {
    return this.get<Bundle>(`/api/v1/monetization/bundles/${id}`);
  }

  async createBundle(data: any): Promise<Bundle> {
    return this.post<Bundle>('/api/v1/monetization/bundles', data);
  }

  async checkEntitlement(data: EntitlementCheck): Promise<EntitlementResult> {
    return this.post<EntitlementResult>('/api/v1/monetization/entitlements/check', data);
  }

  async listInvoices(params?: ListParams): Promise<PaginatedResponse<Invoice>> {
    return this.get<PaginatedResponse<Invoice>>('/api/v1/monetization/billing/invoices', params);
  }

  // Users API
  async listUsers(params?: ListParams): Promise<PaginatedResponse<User>> {
    return this.get<PaginatedResponse<User>>('/api/v1/users', params);
  }

  async inviteUser(data: InviteUserRequest): Promise<User> {
    return this.post<User>('/api/v1/users/invite', data);
  }

  // Metrics API
  async queryMetrics(query: string, time?: string): Promise<any> {
    return this.get('/api/v1/metrics/query', { query, time });
  }

  async queryMetricsRange(query: MetricsQuery): Promise<MetricsResult> {
    return this.get<MetricsResult>('/api/v1/metrics/query_range', query);
  }
}

// Factory function
export function createClient(config?: ClientConfig): MediaEdgeClient {
  return new MediaEdgeClient(config);
}

// Default export
export default MediaEdgeClient;