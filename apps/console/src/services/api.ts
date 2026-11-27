import axios, { AxiosError, AxiosInstance, InternalAxiosRequestConfig } from 'axios';
import { useAuthStore } from '@/store/authStore';

const API_BASE_URL = import.meta.env.VITE_API_URL || '/api';

export const api: AxiosInstance = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
});

// Request interceptor to add auth token
api.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const { accessToken } = useAuthStore.getState();
    if (accessToken && !config.headers.Authorization) {
      config.headers.Authorization = `Bearer ${accessToken}`;
    }
    return config;
  },
  (error) => Promise.reject(error)
);

// Response interceptor for token refresh
let isRefreshing = false;
let failedQueue: Array<{
  resolve: (value: unknown) => void;
  reject: (reason: unknown) => void;
}> = [];

const processQueue = (error: Error | null, token: string | null = null) => {
  failedQueue.forEach((prom) => {
    if (error) {
      prom.reject(error);
    } else {
      prom.resolve(token);
    }
  });
  failedQueue = [];
};

api.interceptors.response.use(
  (response) => response,
  async (error: AxiosError) => {
    const originalRequest = error.config as InternalAxiosRequestConfig & { _retry?: boolean };

    if (error.response?.status === 401 && !originalRequest._retry) {
      if (isRefreshing) {
        return new Promise((resolve, reject) => {
          failedQueue.push({ resolve, reject });
        })
          .then((token) => {
            originalRequest.headers.Authorization = `Bearer ${token}`;
            return api(originalRequest);
          })
          .catch((err) => Promise.reject(err));
      }

      originalRequest._retry = true;
      isRefreshing = true;

      try {
        await useAuthStore.getState().refreshAccessToken();
        const { accessToken } = useAuthStore.getState();
        processQueue(null, accessToken);
        originalRequest.headers.Authorization = `Bearer ${accessToken}`;
        return api(originalRequest);
      } catch (refreshError) {
        processQueue(refreshError as Error, null);
        useAuthStore.getState().logout();
        window.location.href = '/login';
        return Promise.reject(refreshError);
      } finally {
        isRefreshing = false;
      }
    }

    return Promise.reject(error);
  }
);

// API endpoints
export const streamsApi = {
  list: (params?: { page?: number; limit?: number; status?: string; tenantId?: string }) =>
    api.get('/api/v1/streams', { params }),

  get: (id: string) => api.get(`/api/v1/streams/${id}`),

  create: (data: CreateStreamRequest) => api.post('/api/v1/streams', data),

  update: (id: string, data: UpdateStreamRequest) => api.patch(`/api/v1/streams/${id}`, data),

  delete: (id: string) => api.delete(`/api/v1/streams/${id}`),

  start: (id: string) => api.post(`/api/v1/streams/${id}/start`),

  stop: (id: string) => api.post(`/api/v1/streams/${id}/stop`),

  metrics: (id: string, params?: { from?: string; to?: string; interval?: string }) =>
    api.get(`/api/v1/streams/${id}/metrics`, { params }),

  playbackUrls: (id: string) => api.get(`/api/v1/streams/${id}/playback-urls`),
};

export const clusterApi = {
  nodes: () => api.get('/api/v1/cluster/nodes'),

  broadcasts: () => api.get('/api/v1/cluster/broadcasts'),

  config: () => api.get('/api/v1/cluster/config'),

  federation: () => api.get('/api/v1/cluster/federation'),

  mesh: () => api.get('/api/v1/cluster/mesh'),

  capacity: () => api.get('/api/v1/cluster/capacity'),
};

export const tenantsApi = {
  list: (params?: { page?: number; limit?: number; status?: string }) =>
    api.get('/api/v1/tenants', { params }),

  get: (id: string) => api.get(`/api/v1/tenants/${id}`),

  create: (data: CreateTenantRequest) => api.post('/api/v1/tenants', data),

  update: (id: string, data: UpdateTenantRequest) => api.patch(`/api/v1/tenants/${id}`, data),

  delete: (id: string) => api.delete(`/api/v1/tenants/${id}`),

  users: (tenantId: string) => api.get(`/api/v1/tenants/${tenantId}/users`),

  quotas: (tenantId: string) => api.get(`/api/v1/tenants/${tenantId}/quotas`),

  usage: (tenantId: string) => api.get(`/api/v1/tenants/${tenantId}/usage`),
};

export const monetizationApi = {
  fastChannels: {
    list: (params?: { tenantId?: string; status?: string }) =>
      api.get('/api/v1/monetization/fast-channels', { params }),

    get: (id: string) => api.get(`/api/v1/monetization/fast-channels/${id}`),

    create: (data: CreateFastChannelRequest) => api.post('/api/v1/monetization/fast-channels', data),

    update: (id: string, data: UpdateFastChannelRequest) => api.patch(`/api/v1/monetization/fast-channels/${id}`, data),

    delete: (id: string) => api.delete(`/api/v1/monetization/fast-channels/${id}`),

    epg: (id: string, params?: { hours?: number }) => api.get(`/api/v1/monetization/fast-channels/${id}/epg`, { params }),

    schedule: (id: string) => api.get(`/api/v1/monetization/fast-channels/${id}/schedule`),
  },

  tvod: {
    products: (params?: { tenantId?: string; status?: string }) =>
      api.get('/api/v1/monetization/tvod/products', { params }),

    createProduct: (data: CreateTvodProductRequest) => api.post('/api/v1/monetization/tvod/products', data),

    transactions: (params?: { tenantId?: string; userId?: string }) =>
      api.get('/api/v1/monetization/tvod/transactions', { params }),
  },

  bundles: {
    list: (params?: { tenantId?: string; status?: string }) =>
      api.get('/api/v1/monetization/bundles', { params }),

    create: (data: CreateBundleRequest) => api.post('/api/v1/monetization/bundles', data),

    subscriptions: (params?: { tenantId?: string; userId?: string }) =>
      api.get('/api/v1/monetization/bundles/subscriptions', { params }),
  },

  entitlements: {
    check: (data: CheckEntitlementRequest) => api.post('/api/v1/monetization/entitlements/check', data),

    userEntitlements: (userId: string) => api.get(`/api/v1/monetization/entitlements/user/${userId}`),
  },

  billing: {
    invoices: (params?: { tenantId?: string; status?: string }) =>
      api.get('/api/v1/monetization/billing/invoices', { params }),

    subscriptions: (params?: { tenantId?: string; status?: string }) =>
      api.get('/api/v1/monetization/billing/subscriptions', { params }),

    webhook: (provider: string, payload: unknown, signature: string) =>
      api.post(`/api/v1/monetization/billing/webhook/${provider}`, payload, {
        headers: { 'X-Webhook-Signature': signature },
      }),
  },
};

export const authApi = {
  login: (email: string, password: string, mfaCode?: string) =>
    api.post('/api/v1/auth/login', { email, password, mfa_code: mfaCode }),

  logout: () => api.post('/api/v1/auth/logout'),

  refresh: (refreshToken: string) =>
    api.post('/api/v1/auth/refresh', { refresh_token: refreshToken }),

  me: () => api.get('/api/v1/auth/me'),

  mfa: {
    setupTotp: () => api.post('/api/v1/auth/mfa/totp/setup'),

    verifyTotp: (code: string) => api.post('/api/v1/auth/mfa/totp/verify', { code }),

    disableTotp: () => api.delete('/api/v1/auth/mfa/totp'),

    setupWebauthn: () => api.post('/api/v1/auth/mfa/webauthn/setup'),

    verifyWebauthn: (credential: unknown) => api.post('/api/v1/auth/mfa/webauthn/verify', credential),

    backupCodes: () => api.get('/api/v1/auth/mfa/backup-codes'),

    regenerateBackupCodes: () => api.post('/api/v1/auth/mfa/backup-codes/regenerate'),
  },

  oidc: {
    providers: () => api.get('/api/v1/auth/oidc/providers'),

    startAuth: (provider: string, returnUrl?: string) => api.post('/api/v1/auth/oidc/start', { provider, return_url: returnUrl }),

    logoutUrl: (provider: string, idTokenHint?: string) => api.post('/api/v1/auth/oidc/logout', { provider, id_token_hint: idTokenHint }),
  },
};

export const metricsApi = {
  query: (query: string, params?: { from?: string; to?: string; step?: string }) =>
    api.post('/api/v1/metrics/query', { query, ...params }),

  series: (metric: string, params?: { from?: string; to?: string; labels?: Record<string, string> }) =>
    api.get(`/api/v1/metrics/series/${metric}`, { params }),

  labels: () => api.get('/api/v1/metrics/labels'),

  labelValues: (label: string) => api.get(`/api/v1/metrics/label/${label}/values`),
};

export const logsApi = {
  tail: (params?: { level?: string; service?: string; limit?: number }) =>
    api.get('/api/v1/logs/tail', { params }),

  recent: (params?: { level?: string; service?: string; limit?: number; since?: string }) =>
    api.get('/api/v1/logs/recent', { params }),

  search: (query: string, params?: { from?: string; to?: string; limit?: number }) =>
    api.post('/api/v1/logs/search', { query, ...params }),
};

export const sloApi = {
  status: () => api.get('/api/v1/slo/status'),

  report: (params?: { from?: string; to?: string }) => api.get('/api/v1/slo/report', { params }),

  clientSample: (data: ClientSampleRequest) => api.post('/api/v1/slo/client-sample', data),
};

// Type definitions
export interface CreateStreamRequest {
  name: string;
  protocol: 'rtmp' | 'srt' | 'rist' | 'whip' | 'rtsp';
  tenant_id: string;
  settings?: Record<string, unknown>;
}

export interface UpdateStreamRequest {
  name?: string;
  settings?: Record<string, unknown>;
}

export interface CreateTenantRequest {
  name: string;
  slug: string;
  quotas?: Record<string, number>;
  branding?: {
    logo_url?: string;
    primary_color?: string;
    secondary_color?: string;
  };
  settings?: Record<string, unknown>;
}

export interface UpdateTenantRequest {
  name?: string;
  quotas?: Record<string, number>;
  branding?: {
    logo_url?: string;
    primary_color?: string;
    secondary_color?: string;
  };
  settings?: Record<string, unknown>;
  status?: 'active' | 'suspended' | 'pending_verification' | 'deleted';
}

export interface CreateFastChannelRequest {
  name: string;
  slug: string;
  tenant_id: string;
  schedule: Record<string, unknown>;
  ad_breaks: Record<string, unknown>;
  output_formats: string[];
}

export interface UpdateFastChannelRequest {
  name?: string;
  description?: string;
  schedule?: Record<string, unknown>;
  ad_breaks?: Record<string, unknown>;
  output_formats?: string[];
  status?: 'draft' | 'active' | 'paused' | 'archived';
}

export interface CreateTvodProductRequest {
  content_id: string;
  title: string;
  description?: string;
  rental_price_cents?: number;
  purchase_price_cents?: number;
  rental_window_hours?: number;
  rental_duration_hours?: number;
  availability?: Record<string, unknown>;
}

export interface CreateBundleRequest {
  name: string;
  description?: string;
  tenant_id: string;
  bundle_type: 'streamer_bundle' | 'telco_bundle' | 'aggregator_bundle' | 'corporate_bundle' | 'promotional_bundle';
  included_content: Array<{ content_id: string; access_level: string }>;
  pricing: Record<string, unknown>;
}

export interface CheckEntitlementRequest {
  tenant_id: string;
  user_id: string;
  content_id?: string;
  channel_id?: string;
  bundle_id?: string;
  features?: string[];
}

export interface ClientSampleRequest {
  transport: 'hls' | 'dash' | 'ws' | 'whep' | 'moq';
  latency_ms: number;
  timestamp: string;
  viewer_id?: string;
  stream_id?: string;
}