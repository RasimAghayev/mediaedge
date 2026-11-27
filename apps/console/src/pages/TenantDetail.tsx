import { useTranslation } from 'react-i18next';
import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/services/api';
import { Building2, Users, CreditCard, Settings, Database, Broadcast, Shield, Globe, Palette, Trash2, Edit, Plus } from 'lucide-react';
import { clsx } from 'clsx';
import { formatDistanceToNow } from 'date-fns';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';

interface Tenant {
  id: string;
  name: string;
  slug: string;
  status: 'active' | 'suspended' | 'pending_verification' | 'deleted';
  quotas: Record<string, number>;
  usage: {
    streams: number;
    viewers: number;
    storage_gb: number;
    transcode_jobs: number;
    api_keys: number;
  };
  branding?: {
    logo_url?: string;
    primary_color?: string;
    secondary_color?: string;
    favicon_url?: string;
    custom_css?: string;
  };
  settings: {
    allow_public_signup: boolean;
    require_mfa: boolean;
    session_timeout_minutes: number;
    password_policy: Record<string, any>;
    allowed_ip_ranges: string[];
    webhook_urls: string[];
    features: Record<string, boolean>;
  };
  created_at: string;
  updated_at: string;
}

export function TenantDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();

  const { data: tenant, isLoading } = useQuery({
    queryKey: ['tenant', id],
    queryFn: () => api.get(`/api/v1/tenants/${id}`),
    enabled: !!id,
  });

  const { data: users } = useQuery({
    queryKey: ['tenant', id, 'users'],
    queryFn: () => api.get(`/api/v1/tenants/${id}/users`),
    enabled: !!id,
  });

  if (isLoading) return <div className="flex items-center justify-center h-64"><Building2 className="w-8 h-8 animate-spin text-primary-600" /></div>;

  const tenantData = tenant?.data;

  if (!tenantData) return <div className="text-center py-12"><p className="text-secondary-500">Tenant not found</p></div>;

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div className="flex items-center gap-4">
          {tenantData.branding?.logo_url && <img src={tenantData.branding.logo_url} alt="" className="w-12 h-12 rounded-lg" />}
          <div>
            <h1 className="page-title">{tenantData.name}</h1>
            <p className="page-subtitle">{tenantData.slug} • {t(`tenants.${tenantData.status}`)}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <span className={clsx('badge', tenantData.status === 'active' && 'badge-success', tenantData.status === 'suspended' && 'badge-warning')}>
            {t(`tenants.${tenantData.status}`)}
          </span>
        </div>
      </div>

      <Tabs defaultValue="overview" onValueChange={console.log}>
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="overview">{t('common.overview')}</TabsTrigger>
          <TabsTrigger value="users">{t('common.users')}</TabsTrigger>
          <TabsTrigger value="quotas">{t('tenants.quotas')}</TabsTrigger>
          <TabsTrigger value="settings">{t('common.settings')}</TabsTrigger>
          <TabsTrigger value="branding">{t('tenants.branding')}</TabsTrigger>
        </TabsList>

        <TabsContent value="overview">
          <div className="mt-6 grid grid-cols-1 md:grid-cols-4 gap-4">
            <TenantStatCard title={t('tenants.users')} value={tenantData.usage.users} icon="users" />
            <TenantStatCard title={t('streams.streams')} value={tenantData.usage.streams} icon="broadcast" />
            <TenantStatCard title={t('tenants.storage')} value={`${tenantData.usage.storage_gb} GB`} icon="database" />
            <TenantStatCard title={t('tenants.transcodeJobs')} value={tenantData.usage.transcode_jobs} icon="creditCard" />
          </div>
          <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('tenants.quotas')}</h3></div>
              <div className="card-body space-y-4">
                {Object.entries(tenantData.quotas).map(([key, value]) => (
                  <div key={key} className="flex justify-between">
                    <span className="text-secondary-600 dark:text-secondary-400 capitalize">{key.replace(/_/g, ' ')}</span>
                    <div className="flex items-center gap-2 w-48">
                      <div className="flex-1 h-2 bg-secondary-200 dark:bg-secondary-700 rounded-full overflow-hidden">
                        <div className="bg-primary-600 h-full rounded-full" style={{ width: `${Math.min(100, (tenantData.usage[key as keyof typeof tenantData.usage] as number || 0) / (value || 1) * 100)}%` }} />
                      </div>
                      <span className="text-sm font-mono w-16 text-right">{tenantData.usage[key as keyof typeof tenantData.usage] || 0} / {value}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.features')}</h3></div>
              <div className="card-body">
                <div className="grid grid-cols-2 gap-4">
                  {Object.entries(tenantData.settings.features).map(([key, enabled]) => (
                    <div key={key} className="flex items-center justify-between">
                      <span className="capitalize">{key.replace(/_/g, ' ')}</span>
                      <span className={clsx('badge', enabled ? 'badge-success' : 'badge-neutral')}>{enabled ? t('common.enabled') : t('common.disabled')}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="users">
          <div className="mt-4">
            <div className="flex justify-between mb-4">
              <h3 className="font-medium">{t('common.users')}</h3>
              <button className="btn-primary btn-sm"><Plus className="w-4 h-4" /> {t('common.inviteUser')}</button>
            </div>
            <div className="table-container">
              <table className="table">
                <thead><tr><th>Name</th><th>Email</th><th>Roles</th><th>MFA</th><th>Status</th><th>Last Login</th><th>Actions</th></tr></thead>
                <tbody>
                  {users?.data?.users?.map((user: any) => (
                    <tr key={user.id}>
                      <td className="font-medium">{user.name}</td>
                      <td>{user.email}</td>
                      <td>{user.roles?.join(', ') || '—'}</td>
                      <td><span className={clsx('badge', user.mfa_enabled ? 'badge-success' : 'badge-neutral')}>{user.mfa_enabled ? t('common.enabled') : t('common.disabled')}</span></td>
                      <td><span className={clsx('badge', user.status === 'active' ? 'badge-success' : 'badge-warning')}>{user.status}</span></td>
                      <td className="text-secondary-500">{user.last_login_at ? formatDistanceToNow(new Date(user.last_login_at), { addSuffix: true }) : 'Never'}</td>
                      <td><button className="p-1.5 rounded text-secondary-400 hover:text-secondary-600"><Edit className="w-4 h-4" /></button></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="quotas">
          <div className="mt-4 space-y-4">
            {Object.entries(tenantData.quotas).map(([key, value]) => (
              <div key={key} className="card">
                <div className="card-body">
                  <div className="flex justify-between mb-2">
                    <h3 className="font-medium capitalize">{key.replace(/_/g, ' ')}</h3>
                    <span className="text-secondary-500">Limit: {value}</span>
                  </div>
                  <div className="h-4 bg-secondary-200 dark:bg-secondary-700 rounded-full overflow-hidden">
                    <div className="bg-primary-600 h-full rounded-full transition-all" style={{ width: `${Math.min(100, (tenantData.usage[key as keyof typeof tenantData.usage] as number || 0) / (value || 1) * 100)}%` }} />
                  </div>
                </div>
              </div>
            ))}
          </div>
        </TabsContent>

        <TabsContent value="settings">
          <div className="mt-4 space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.general')}</h3></div>
              <div className="card-body space-y-4">
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('tenants.allowPublicSignup')}</p><p className="text-sm text-secondary-500">{t('tenants.allowPublicSignupDesc')}</p></div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input type="checkbox" defaultChecked={tenantData.settings.allow_public_signup} className="sr-only peer" />
                    <div className="w-11 h-6 bg-secondary-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-primary-300 dark:peer-focus:ring-primary-800 rounded-full peer dark:bg-secondary-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-secondary-600 peer-checked:bg-primary-600"></div>
                  </label>
                </div>
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('tenants.requireMfa')}</p><p className="text-sm text-secondary-500">{t('tenants.requireMfaDesc')}</p></div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input type="checkbox" defaultChecked={tenantData.settings.require_mfa} className="sr-only peer" />
                    <div className="w-11 h-6 bg-secondary-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-primary-300 dark:peer-focus:ring-primary-800 rounded-full peer dark:bg-secondary-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-secondary-600 peer-checked:bg-primary-600"></div>
                  </label>
                </div>
                <div>
                  <label className="label">{t('tenants.sessionTimeout')}</label>
                  <input type="number" className="input w-32" defaultValue={tenantData.settings.session_timeout_minutes} />
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('tenants.allowedIpRanges')}</h3></div>
              <div className="card-body">
                <p className="text-sm text-secondary-500 mb-4">{t('tenants.allowedIpRangesDesc')}</p>
                <div className="space-y-2">
                  {tenantData.settings.allowed_ip_ranges?.map((ip: string, i: number) => (
                    <div key={i} className="flex gap-2">
                      <input type="text" className="input" defaultValue={ip} />
                      <button className="btn-danger btn-sm"><Trash2 className="w-4 h-4" /></button>
                    </div>
                  ))}
                  <button className="btn-secondary btn-sm"><Plus className="w-4 h-4" /> {t('common.addIpRange')}</button>
                </div>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="branding">
          <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('tenants.branding')}</h3></div>
              <div className="card-body space-y-4">
                <div>
                  <label className="label">{t('tenants.logoUrl')}</label>
                  <input type="url" className="input" defaultValue={tenantData.branding?.logo_url} placeholder="https://example.com/logo.png" />
                </div>
                <div>
                  <label className="label">{t('tenants.primaryColor')}</label>
                  <input type="color" className="input w-20 h-10 p-0" defaultValue={tenantData.branding?.primary_color || '#0ea5e9'} />
                </div>
                <div>
                  <label className="label">{t('tenants.secondaryColor')}</label>
                  <input type="color" className="input w-20 h-10 p-0" defaultValue={tenantData.branding?.secondary_color || '#64748b'} />
                </div>
                <div>
                  <label className="label">{t('tenants.faviconUrl')}</label>
                  <input type="url" className="input" defaultValue={tenantData.branding?.favicon_url} placeholder="https://example.com/favicon.ico" />
                </div>
                <div>
                  <label className="label">{t('tenants.customCss')}</label>
                  <textarea className="input font-mono text-sm min-h-[150px]" defaultValue={tenantData.branding?.custom_css} placeholder="/* Custom CSS */" />
                </div>
                <div className="flex justify-end">
                  <button className="btn-primary">{t('common.save')}</button>
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('tenants.preview')}</h3></div>
              <div className="card-body">
                <div className="border border-secondary-200 dark:border-secondary-700 rounded-lg p-8 bg-white dark:bg-secondary-900" style={{ borderColor: tenantData.branding?.primary_color }}>
                  <h2 className="text-2xl font-bold" style={{ color: tenantData.branding?.primary_color }}>{tenantData.name}</h2>
                  <p className="mt-2" style={{ color: tenantData.branding?.secondary_color }}>Welcome to your branded MediaEdge console</p>
                </div>
              </div>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}

function TenantStatCard({ title, value, icon }: { title: string; value: number; icon: string }) {
  const icons = { users: Users, broadcast: Broadcast, database: Database, creditCard: CreditCard, settings: Settings, shield: Shield, globe: Globe, palette: Palette };
  const Icon = icons[icon as keyof typeof icons];

  return (
    <div className="card">
      <div className="card-body">
        <div className="flex items-center justify-between">
          <div><p className="text-sm text-secondary-500">{title}</p><p className="text-2xl font-bold">{value}</p></div>
          <div className="w-10 h-10 rounded-lg bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center"><Icon className="w-5 h-5 text-primary-600" /></div>
        </div>
      </div>
    </div>
  );
}