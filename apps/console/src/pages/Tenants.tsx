import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, type CreateTenantRequest, type UpdateTenantRequest } from '@/services/api';
import { Building2, Plus, Search, Filter, MoreHorizontal, Edit, Trash2, Users, Database, CreditCard, Settings } from 'lucide-react';
import { useState } from 'react';
import { clsx } from 'clsx';
import { formatDistanceToNow } from 'date-fns';

interface Tenant {
  id: string;
  name: string;
  slug: string;
  status: 'active' | 'suspended' | 'pending_verification' | 'deleted';
  quotas: Record<string, number>;
  branding?: {
    logo_url?: string;
    primary_color?: string;
    secondary_color?: string;
  };
  users_count: number;
  streams_count: number;
  storage_used_gb: number;
  created_at: string;
  updated_at: string;
}

export function Tenants() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [showCreateModal, setShowCreateModal] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ['tenants', search, statusFilter],
    queryFn: () => api.get('/api/v1/tenants', {
      params: { search, status: statusFilter, limit: 50 },
    }),
    refetchInterval: 30000,
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateTenantRequest) => api.post('/api/v1/tenants', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tenants'] });
      setShowCreateModal(false);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateTenantRequest }) => api.patch(`/api/v1/tenants/${id}`, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tenants'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/v1/tenants/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tenants'] }),
  });

  const statuses = ['active', 'suspended', 'pending_verification', 'deleted'];

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="page-title">{t('tenants.title')}</h1>
          <p className="page-subtitle">{t('tenants.manageTenants')}</p>
        </div>
        <button onClick={() => setShowCreateModal(true)} className="btn-primary">
          <Plus className="w-4 h-4" />
          {t('tenants.createTenant')}
        </button>
      </div>

      <div className="card">
        <div className="card-body p-4">
          <div className="flex flex-col sm:flex-row gap-4">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-secondary-400" />
              <input
                type="text"
                placeholder={t('common.search')}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="input pl-10"
              />
            </div>
            <select
              value={statusFilter}
              onChange={(e) => setStatusFilter(e.target.value)}
              className="input w-auto"
            >
              <option value="">{t('common.allStatus')}</option>
              {statuses.map((s) => (
                <option key={s} value={s}>{t(`tenants.${s}`)}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('tenants.name')}</th>
                <th>{t('tenants.slug')}</th>
                <th>{t('tenants.status')}</th>
                <th>{t('tenants.users')}</th>
                <th>{t('streams.streams')}</th>
                <th>{t('tenants.storage')}</th>
                <th>{t('common.created')}</th>
                <th className="w-32">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr><td colSpan={8} className="text-center py-8"><Building2 className="w-8 h-8 animate-spin text-primary-600 mx-auto" /></td></tr>
              ) : data?.data?.tenants?.length === 0 ? (
                <tr><td colSpan={8} className="text-center py-8"><Building2 className="w-12 h-12 text-secondary-300 mx-auto mb-4" /><p className="text-secondary-500">{t('tenants.noTenants')}</p></td></tr>
              ) : (
                data?.data?.tenants?.map((tenant: Tenant) => (
                  <tr key={tenant.id}>
                    <td>
                      <div className="flex items-center gap-3">
                        {tenant.branding?.logo_url && (
                          <img src={tenant.branding.logo_url} alt="" className="w-8 h-8 rounded-lg object-cover" />
                        )}
                        <div>
                          <a href={`/tenants/${tenant.id}`} className="font-medium hover:text-primary-600">{tenant.name}</a>
                          <p className="text-sm text-secondary-500">{tenant.slug}</p>
                        </div>
                      </div>
                    </td>
                    <td className="font-mono text-sm">{tenant.slug}</td>
                    <td>
                      <span className={clsx(
                        'badge',
                        tenant.status === 'active' && 'badge-success',
                        tenant.status === 'suspended' && 'badge-warning',
                        tenant.status === 'pending_verification' && 'badge-info',
                        tenant.status === 'deleted' && 'badge-danger'
                      )}>
                        {t(`tenants.${tenant.status}`)}
                      </span>
                    </td>
                    <td>{tenant.users_count}</td>
                    <td>{tenant.streams_count}</td>
                    <td>{tenant.storage_used_gb} GB</td>
                    <td className="text-secondary-500">{formatDistanceToNow(new Date(tenant.created_at), { addSuffix: true })}</td>
                    <td>
                      <div className="flex items-center justify-end gap-1">
                        <a href={`/tenants/${tenant.id}`} className="p-1.5 rounded text-secondary-400 hover:text-secondary-600" title={t('common.view')}>
                          <ExternalLink className="w-4 h-4" />
                        </a>
                        <button onClick={() => setShowCreateModal(true)} className="p-1.5 rounded text-secondary-400 hover:text-secondary-600" title={t('common.edit')}>
                          <Edit className="w-4 h-4" />
                        </button>
                        <button onClick={() => deleteMutation.mutate(tenant.id)} className="p-1.5 rounded text-danger-600 hover:bg-danger-100" title={t('common.delete')}>
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {showCreateModal && <CreateTenantModal onClose={() => setShowCreateModal(false)} onSubmit={createMutation.mutate} isLoading={createMutation.isPending} />}
    </div>
  );
}

function CreateTenantModal({ onClose, onSubmit, isLoading }: { onClose: () => void; onSubmit: (data: CreateTenantRequest) => void; isLoading: boolean }) {
  const { t } = useTranslation();
  const [formData, setFormData] = useState<CreateTenantRequest>({ name: '', slug: '', quotas: {}, branding: {}, settings: {} });

  const handleSubmit = (e: React.FormEvent) => { e.preventDefault(); onSubmit(formData); };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50">
      <div className="bg-white dark:bg-secondary-900 rounded-xl shadow-xl max-w-md w-full max-h-[90vh] overflow-y-auto animate-in">
        <div className="p-6 border-b border-secondary-200 dark:border-secondary-700 flex justify-between">
          <h2 className="text-xl font-semibold">{t('tenants.createTenant')}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-secondary-100">✕</button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div><label className="label">{t('tenants.name')}</label><input type="text" className="input" value={formData.name} onChange={(e) => setFormData({...formData, name: e.target.value})} required placeholder="Acme Corp" /></div>
          <div><label className="label">{t('tenants.slug')}</label><input type="text" className="input" value={formData.slug} onChange={(e) => setFormData({...formData, slug: e.target.value})} required placeholder="acme-corp" /></div>
          <div className="flex justify-end gap-3 pt-4 border-t border-secondary-200">
            <button type="button" onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" disabled={isLoading} className="btn-primary">{isLoading ? t('common.loading') : t('common.create')}</button>
          </div>
        </form>
      </div>
    </div>
  );
}