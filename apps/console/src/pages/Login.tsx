import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuthStore } from '@/store/authStore';
import { Mail, Lock, Eye, EyeOff, AlertCircle, CheckCircle, Loader2, Github, Gitlab, Google, Microsoft } from 'lucide-react';
import { clsx } from 'clsx';
import { api } from '@/services/api';

export function Login() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setAuth } = useAuthStore();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [mfaCode, setMfaCode] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showMfa, setShowMfa] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [mfaLoading, setMfaLoading] = useState(false);

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      const response = await api.post('/api/v1/auth/login', { email, password, mfa_code: mfaCode || undefined });

      if (response.data.requires_mfa) {
        setShowMfa(true);
        return;
      }

      setAuth({
        accessToken: response.data.access_token,
        refreshToken: response.data.refresh_token,
        user: response.data.user,
        tenant: response.data.tenant,
      });

      navigate('/dashboard');
    } catch (err: any) {
      setError(err.response?.data?.message || t('common.loginFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleMfaLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setMfaLoading(true);

    try {
      const response = await api.post('/api/v1/auth/login', { email, password, mfa_code: mfaCode });

      setAuth({
        accessToken: response.data.access_token,
        refreshToken: response.data.refresh_token,
        user: response.data.user,
        tenant: response.data.tenant,
      });

      navigate('/dashboard');
    } catch (err: any) {
      setError(err.response?.data?.message || t('common.invalidMfaCode'));
    } finally {
      setMfaLoading(false);
    }
  };

  const handleOidcLogin = (provider: string) => {
    api.post('/api/v1/auth/oidc/start', { provider, return_url: window.location.origin })
      .then((response) => {
        window.location.href = response.data.auth_url;
      })
      .catch(() => {
        setError(t('common.oidcLoginFailed'));
      });
  };

  if (useAuthStore.getState().isAuthenticated) {
    return navigate('/dashboard');
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-secondary-50 dark:bg-secondary-900 px-4">
      <div className="w-full max-w-md animate-in">
        <div className="text-center mb-8">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-primary-600 mb-4">
            <Broadcast className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-2xl font-bold text-secondary-900 dark:text-white">MediaEdge</h1>
          <p className="text-secondary-500 mt-2">{showMfa ? t('common.enterMfaCode') : t('common.signInToAccount')}</p>
        </div>

        <div className="card">
          <div className="card-body p-6 space-y-4">
            {showMfa ? (
              <form onSubmit={handleMfaLogin} className="space-y-4">
                <div>
                  <label className="label">{t('common.mfaCode')}</label>
                  <div className="relative">
                    <input
                      type="text"
                      className="input text-center text-2xl tracking-widest font-mono"
                      value={mfaCode}
                      onChange={(e) => setMfaCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                      placeholder="000000"
                      maxLength={6}
                      autoComplete="one-time-code"
                      required
                      autoFocus
                    />
                  </div>
                </div>
                {error && <div className="flex items-center gap-2 text-sm text-danger-600 bg-danger-50 dark:bg-danger-900/30 p-3 rounded-lg"><AlertCircle className="w-4 h-4" />{error}</div>}
                <button type="submit" disabled={mfaLoading} className="btn-primary w-full">
                  {mfaLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : ''} {t('common.verify')}
                </button>
                <button type="button" onClick={() => { setShowMfa(false); setMfaCode(''); }} className="btn-secondary w-full">{t('common.back')}</button>
              </form>
            ) : (
              <form onSubmit={handleLogin} className="space-y-4">
                <div>
                  <label className="label">{t('common.email')}</label>
                  <div className="relative">
                    <Mail className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-secondary-400" />
                    <input
                      type="email"
                      className="input pl-10"
                      value={email}
                      onChange={(e) => setEmail(e.target.value)}
                      placeholder="user@example.com"
                      required
                      autoComplete="email"
                    />
                  </div>
                </div>
                <div>
                  <label className="label">{t('common.password')}</label>
                  <div className="relative">
                    <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-secondary-400" />
                    <input
                      type={showPassword ? 'text' : 'password'}
                      className="input pl-10 pr-10"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder="••••••••"
                      required
                      autoComplete="current-password"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-secondary-400 hover:text-secondary-600"
                    >
                      {showPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                    </button>
                  </div>
                </div>
                {error && <div className="flex items-center gap-2 text-sm text-danger-600 bg-danger-50 dark:bg-danger-900/30 p-3 rounded-lg"><AlertCircle className="w-4 h-4" />{error}</div>}
                <button type="submit" disabled={loading} className="btn-primary w-full">
                  {loading ? <Loader2 className="w-4 h-4 animate-spin" /> : ''} {t('common.signIn')}
                </button>
              </form>
            )}

            <div className="relative">
              <div className="absolute inset-0 flex items-center">
                <div className="w-full border-t border-secondary-200 dark:border-secondary-700" />
              </div>
              <div className="relative flex justify-center text-sm">
                <span className="px-2 bg-white dark:bg-secondary-900 text-secondary-500">{t('common.orContinueWith')}</span>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-3">
              <button type="button" onClick={() => handleOidcLogin('github')} className="btn-secondary flex items-center justify-center gap-2">
                <Github className="w-5 h-5" />
                <span>GitHub</span>
              </button>
              <button type="button" onClick={() => handleOidcLogin('gitlab')} className="btn-secondary flex items-center justify-center gap-2">
                <Gitlab className="w-5 h-5" />
                <span>GitLab</span>
              </button>
              <button type="button" onClick={() => handleOidcLogin('google')} className="btn-secondary flex items-center justify-center gap-2">
                <Google className="w-5 h-5" />
                <span>Google</span>
              </button>
              <button type="button" onClick={() => handleOidcLogin('azure_ad')} className="btn-secondary flex items-center justify-center gap-2">
                <Microsoft className="w-5 h-5" />
                <span>Azure AD</span>
              </button>
            </div>

            <p className="text-center text-sm text-secondary-500">
              {t('common.dontHaveAccount')}{' '}
              <a href="#" className="text-primary-600 hover:text-primary-700 font-medium">{t('common.contactAdmin')}</a>
            </p>
          </div>
        </div>

        <div className="mt-6 text-center text-sm text-secondary-500">
          <p>MediaEdge v0.1.0</p>
          <p className="mt-1"><a href="#" className="text-primary-600 hover:underline">{t('common.docs')}</a> · <a href="#" className="text-primary-600 hover:underline">{t('common.support')}</a> · <a href="#" className="text-primary-600 hover:underline">{t('common.status')}</a></p>
        </div>
      </div>
    </div>
  );
}

// Need to import Broadcast
import { Broadcast } from 'lucide-react';