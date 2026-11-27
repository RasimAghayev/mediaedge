import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '@/services/api';
import { Users, Plus, Search, Filter, MoreHorizontal, Edit, Trash2, Shield, Mail, Lock, UserPlus, UserCheck, UserX } from 'lucide-react';
import { useState } from 'react';
import { clsx } from 'clsx';
import { formatDistanceToNow } from 'date-fns';

interface User {
  id: string;
  email: string;
  name: string;
  roles: string[];
  mfa_enabled: boolean;
  status: 'active' | 'invited' | 'suspended' | 'deleted';
  tenant_id: string;
  tenant_name: string;
  last_login_at: string | null;
  created_at: string;
}

export function Users() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [roleFilter, setRoleFilter] = useState('');
  const [showInviteModal, setShowInviteModal] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ['users', search, statusFilter, roleFilter],
    queryFn: () => api.get('/api/v1/users', { params: { search, status: statusFilter, role: roleFilter, limit: 50 } }),
    refetchInterval: 30000,
  });

  const inviteMutation = useMutation({
    mutationFn: (data: { email: string; name: string; roles: string[]; tenant_id: string }) => api.post('/api/v1/users/invite', data),
    onSuccess: () => { queryClient.invalidateQueries({ queryKey: ['users'] }); setShowInviteModal(false); },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Partial<User> }) => api.patch(`/api/v1/users/${id}`, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['users'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/v1/users/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['users'] }),
  });

  const statuses = ['active', 'invited', 'suspended', 'deleted'];
  const roles = ['admin', 'operator', 'viewer', 'api-client'];

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="page-title">{t('common.users')}</h1>
          <p className="page-subtitle">{t('common.manageUsers')}</p>
        </div>
        <button onClick={() => setShowInviteModal(true)} className="btn-primary">
          <UserPlus className="w-4 h-4" />
          {t('common.inviteUser')}
        </button>
      </div>

      <div className="card">
        <div className="card-body p-4">
          <div className="flex flex-col sm:flex-row gap-4">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-secondary-400" />
              <input type="text" placeholder={t('common.search')} value={search} onChange={(e) => setSearch(e.target.value)} className="input pl-10" />
            </div>
            <select value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)} className="input w-auto">
              <option value="">{t('common.allStatus')}</option>
              {statuses.map((s) => <option key={s} value={s}>{t(`common.${s}`)}</option>)}
            </select>
            <select value={roleFilter} onChange={(e) => setRoleFilter(e.target.value)} className="input w-auto">
              <option value="">{t('common.allRoles')}</option>
              {roles.map((r) => <option key={r} value={r}>{r}</option>)}
            </select>
          </div>
        </div>

        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('common.name')}</th>
                <th>{t('common.email')}</th>
                <th>{t('common.tenant')}</th>
                <th>{t('common.roles')}</th>
                <th>{t('common.mfa')}</th>
                <th>{t('common.status')}</th>
                <th>{t('common.lastLogin')}</th>
                <th className="w-32">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr><td colSpan={8} className="text-center py-8"><Users className="w-8 h-8 animate-spin text-primary-600 mx-auto" /></td></tr>
              ) : data?.data?.users?.length === 0 ? (
                <tr><td colSpan={8} className="text-center py-8"><Users className="w-12 h-12 text-secondary-300 mx-auto mb-4" /><p className="text-secondary-500">{t('common.noUsers')}</p></td></tr>
              ) : (
                data?.data?.users?.map((user: User) => (
                  <tr key={user.id}>
                    <td className="font-medium">{user.name}</td>
                    <td>{user.email}</td>
                    <td className="text-secondary-500">{user.tenant_name}</td>
                    <td>
                      <div className="flex flex-wrap gap-1">
                        {user.roles.map((r) => <span key={r} className="badge badge-info">{r}</span>)}
                      </div>
                    </td>
                    <td><span className={clsx('badge', user.mfa_enabled ? 'badge-success' : 'badge-neutral')}>{user.mfa_enabled ? t('common.enabled') : t('common.disabled')}</span></td>
                    <td><span className={clsx('badge', user.status === 'active' && 'badge-success', user.status === 'invited' && 'badge-info', user.status === 'suspended' && 'badge-warning')}>{t(`common.${user.status}`)}</span></td>
                    <td className="text-secondary-500">{user.last_login_at ? formatDistanceToNow(new Date(user.last_login_at), { addSuffix: true }) : 'Never'}</td>
                    <td>
                      <div className="flex items-center justify-end gap-1">
                        <button className="p-1.5 rounded text-secondary-400 hover:text-secondary-600" title={t('common.edit')}>
                          <Edit className="w-4 h-4" />
                        </button>
                        <button className="p-1.5 rounded text-danger-600 hover:bg-danger-100" title={t('common.delete')} onClick={() => deleteMutation.mutate(user.id)}>
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

      {showInviteModal && <InviteUserModal onClose={() => setShowInviteModal(false)} onSubmit={inviteMutation.mutate} isLoading={inviteMutation.isPending} />}
    </div>
  );
}

function InviteUserModal({ onClose, onSubmit, isLoading }: { onClose: () => void; onSubmit: (data: any) => void; isLoading: boolean }) {
  const { t } = useTranslation();
  const [formData, setFormData] = useState({ email: '', name: '', roles: [] as string[], tenant_id: '' });
  const [roleSelection, setRoleSelection] = useState<Record<string, boolean>>({ admin: false, operator: false, viewer: false, 'api-client': false });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const roles = Object.entries(roleSelection).filter(([, v]) => v).map(([k]) => k);
    onSubmit({ ...formData, roles });
  };

  const toggleRole = (role: string) => {
    setRoleSelection((prev) => ({ ...prev, [role]: !prev[role] }));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50">
      <div className="bg-white dark:bg-secondary-900 rounded-xl shadow-xl max-w-md w-full max-h-[90vh] overflow-y-auto animate-in">
        <div className="p-6 border-b border-secondary-200 dark:border-secondary-700 flex justify-between">
          <h2 className="text-xl font-semibold">{t('common.inviteUser')}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-secondary-100">✕</button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div><label className="label">{t('common.email')}</label><input type="email" className="input" value={formData.email} onChange={(e) => setFormData({ ...formData, email: e.target.value })} required placeholder="user@example.com" /></div>
          <div><label className="label">{t('common.name')}</label><input type="text" className="input" value={formData.name} onChange={(e) => setFormData({ ...formData, name: e.target.value })} required placeholder="John Doe" /></div>
          <div><label className="label">{t('common.tenant')}</label><select className="input" value={formData.tenant_id} onChange={(e) => setFormData({ ...formData, tenant_id: e.target.value })} required><option value="">{t('common.selectTenant')}</option></select></div>
          <div>
            <label className="label">{t('common.roles')}</label>
            <div className="grid grid-cols-2 gap-2">
              {['admin', 'operator', 'viewer', 'api-client'].map((role) => (
                <label key={role} className="flex items-center gap-2 p-2 rounded-lg border border-secondary-200 dark:border-secondary-700 hover:bg-secondary-50 dark:hover:bg-secondary-800 cursor-pointer">
                  <input type="checkbox" checked={roleSelection[role]} onChange={() => toggleRole(role)} className="w-4 h-4 text-primary-600 rounded" />
                  <span className="capitalize">{role}</span>
                </label>
              ))}
            </div>
          </div>
          <div className="flex justify-end gap-3 pt-4 border-t border-secondary-200">
            <button type="button" onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" disabled={isLoading} className="btn-primary">{isLoading ? t('common.loading') : t('common.invite')}</button>
          </div>
        </form>
      </div>
    </div>
  );
}