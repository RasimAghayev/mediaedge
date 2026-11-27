import { useTranslation } from 'react-i18next';
import { useAuthStore } from '@/store/authStore';
import { User, Shield, Bell, Palette, Globe, Key, Database, Server, Save, Loader2 } from 'lucide-react';
import { useState } from 'react';
import { clsx } from 'clsx';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';

export function Settings() {
  const { t } = useTranslation();
  const { user, theme, setTheme, language, setLanguage, tenant } = useAuthStore();
  const [saving, setSaving] = useState(false);
  const [notifications, setNotifications] = useState({
    email: true,
    push: true,
    streamAlerts: true,
    billingAlerts: true,
    securityAlerts: true,
    weeklyDigest: false,
  });

  const handleSave = async () => {
    setSaving(true);
    await new Promise((r) => setTimeout(r, 1000));
    setSaving(false);
  };

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('common.settings')}</h1>
          <p className="page-subtitle">{t('common.manageSettings')}</p>
        </div>
      </div>

      <Tabs defaultValue="profile" onValueChange={console.log}>
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="profile">{t('common.profile')}</TabsTrigger>
          <TabsTrigger value="security">{t('common.security')}</TabsTrigger>
          <TabsTrigger value="notifications">{t('common.notifications')}</TabsTrigger>
          <TabsTrigger value="appearance">{t('common.appearance')}</TabsTrigger>
          <TabsTrigger value="integrations">{t('common.integrations')}</TabsTrigger>
        </TabsList>

        <TabsContent value="profile">
          <div className="mt-6 max-w-2xl space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.profile')}</h3></div>
              <div className="card-body space-y-4">
                <div className="flex items-center gap-6">
                  <div className="w-20 h-20 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
                    <User className="w-10 h-10 text-primary-600" />
                  </div>
                  <div>
                    <h3 className="text-lg font-medium">{user?.name || 'User'}</h3>
                    <p className="text-secondary-500">{user?.email || ''}</p>
                    <p className="text-sm text-secondary-400">{tenant?.name || ''}</p>
                  </div>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div><label className="label">{t('common.name')}</label><input type="text" className="input" defaultValue={user?.name} /></div>
                  <div><label className="label">{t('common.email')}</label><input type="email" className="input" defaultValue={user?.email} disabled /></div>
                  <div><label className="label">{t('common.tenant')}</label><input type="text" className="input" defaultValue={tenant?.name} disabled /></div>
                  <div><label className="label">{t('common.role')}</label><input type="text" className="input" defaultValue={user?.roles?.[0] || 'viewer'} disabled /></div>
                </div>
                <div className="flex justify-end">
                  <button onClick={handleSave} disabled={saving} className="btn-primary">
                    {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}{' '}
                    {t('common.save')}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="security">
          <div className="mt-6 max-w-2xl space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.password')}</h3></div>
              <div className="card-body space-y-4">
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('common.changePassword')}</p><p className="text-sm text-secondary-500">{t('common.changePasswordDesc')}</p></div>
                  <button className="btn-secondary">{t('common.change')}</button>
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.twoFactorAuth')}</h3></div>
              <div className="card-body space-y-4">
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('common.totp')}</p><p className="text-sm text-secondary-500">{t('common.totpDesc')}</p></div>
                  <button className="btn-secondary">{t('common.setup')}</button>
                </div>
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('common.webauthn')}</p><p className="text-sm text-secondary-500">{t('common.webauthnDesc')}</p></div>
                  <button className="btn-secondary">{t('common.setup')}</button>
                </div>
                <div className="flex items-center justify-between">
                  <div><p className="font-medium">{t('common.backupCodes')}</p><p className="text-sm text-secondary-500">{t('common.backupCodesDesc')}</p></div>
                  <button className="btn-secondary">{t('common.regenerate')}</button>
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.sessions')}</h3></div>
              <div className="card-body">
                <p className="text-sm text-secondary-500">{t('common.sessionsDesc')}</p>
                <button className="btn-danger mt-4">{t('common.revokeAllSessions')}</button>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="notifications">
          <div className="mt-6 max-w-2xl space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.notificationPreferences')}</h3></div>
              <div className="card-body space-y-4">
                {Object.entries(notifications).map(([key, enabled]) => (
                  <div key={key} className="flex items-center justify-between">
                    <div>
                      <p className="font-medium capitalize">{key.replace(/([A-Z])/g, ' $1')}</p>
                      <p className="text-sm text-secondary-500">{t(`common.${key}Desc`)}</p>
                    </div>
                    <label className="relative inline-flex items-center cursor-pointer">
                      <input type="checkbox" defaultChecked={enabled} onChange={(e) => setNotifications((prev) => ({ ...prev, [key]: e.target.checked }))} className="sr-only peer" />
                      <div className="w-11 h-6 bg-secondary-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-primary-300 dark:peer-focus:ring-primary-800 rounded-full peer dark:bg-secondary-700 peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-secondary-600 peer-checked:bg-primary-600"></div>
                    </label>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="appearance">
          <div className="mt-6 max-w-2xl space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.theme')}</h3></div>
              <div className="card-body">
                <div className="grid grid-cols-3 gap-4">
                  {(['light', 'dark', 'system'] as const).map((t) => (
                    <button
                      key={t}
                      onClick={() => setTheme(t)}
                      className={clsx(
                        'p-4 rounded-lg border-2 transition-all',
                        theme === t ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20' : 'border-secondary-200 dark:border-secondary-700 hover:border-secondary-300 dark:hover:border-secondary-600'
                      )}
                    >
                      <div className="w-10 h-10 rounded-lg mx-auto mb-2 bg-secondary-100 dark:bg-secondary-800 flex items-center justify-center">
                        {t === 'light' && <Sun className="w-5 h-5" />}
                        {t === 'dark' && <Moon className="w-5 h-5" />}
                        {t === 'system' && <Monitor className="w-5 h-5" />}
                      </div>
                      <p className="text-center capitalize">{t(`common.${t}`)}</p>
                    </button>
                  ))}
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.language')}</h3></div>
              <div className="card-body">
                <select value={language} onChange={(e) => setLanguage(e.target.value)} className="input w-48">
                  <option value="en">English</option>
                  <option value="az">Azərbaycan dili</option>
                </select>
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="integrations">
          <div className="mt-6 max-w-2xl space-y-6">
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.apiKeys')}</h3></div>
              <div className="card-body">
                <div className="flex justify-between items-center mb-4">
                  <p className="text-sm text-secondary-500">{t('common.apiKeysDesc')}</p>
                  <button className="btn-primary btn-sm"><Plus className="w-4 h-4" /> {t('common.createApiKey')}</button>
                </div>
                <div className="table-container">
                  <table className="table">
                    <thead><tr><th>Name</th><th>Prefix</th><th>Created</th><th>Last Used</th><th>Status</th><th>Actions</th></tr></thead>
                    <tbody>
                      <tr><td colSpan={6} className="text-center py-8 text-secondary-500">{t('common.noApiKeys')}</td></tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
            <div className="card">
              <div className="card-header"><h3 className="font-medium">{t('common.webhooks')}</h3></div>
              <div className="card-body">
                <p className="text-sm text-secondary-500 mb-4">{t('common.webhooksDesc')}</p>
                <button className="btn-secondary btn-sm"><Plus className="w-4 h-4" /> {t('common.addWebhook')}</button>
              </div>
            </div>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}