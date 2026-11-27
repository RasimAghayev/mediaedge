import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { api } from '@/services/api';

interface User {
  id: string;
  email: string;
  name: string;
  roles: string[];
  tenantId: string;
  tenantName: string;
  mfaEnabled: boolean;
  avatar?: string;
}

interface Tenant {
  id: string;
  name: string;
  slug: string;
  status: string;
  quotas: Record<string, number>;
  branding?: {
    logoUrl?: string;
    primaryColor?: string;
    secondaryColor?: string;
  };
}

interface AuthState {
  user: User | null;
  tenant: Tenant | null;
  accessToken: string | null;
  refreshToken: string | null;
  isAuthenticated: boolean;
  isSidebarOpen: boolean;
  theme: 'light' | 'dark' | 'system';
  language: string;

  // Actions
  setAuth: (tokens: { accessToken: string; refreshToken: string; user: User; tenant: Tenant }) => void;
  setUser: (user: User) => void;
  setTenant: (tenant: Tenant) => void;
  logout: () => void;
  checkAuth: () => Promise<void>;
  refreshAccessToken: () => Promise<void>;
  toggleSidebar: () => void;
  setSidebarOpen: (open: boolean) => void;
  setTheme: (theme: 'light' | 'dark' | 'system') => void;
  setLanguage: (lang: string) => void;
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set, get) => ({
      user: null,
      tenant: null,
      accessToken: null,
      refreshToken: null,
      isAuthenticated: false,
      isSidebarOpen: true,
      theme: 'system',
      language: 'en',

      setAuth: ({ accessToken, refreshToken, user, tenant }) => {
        api.defaults.headers.common['Authorization'] = `Bearer ${accessToken}`;
        set({
          accessToken,
          refreshToken,
          user,
          tenant,
          isAuthenticated: true,
        });
      },

      setUser: (user) => set({ user }),

      setTenant: (tenant) => set({ tenant }),

      logout: () => {
        delete api.defaults.headers.common['Authorization'];
        set({
          user: null,
          tenant: null,
          accessToken: null,
          refreshToken: null,
          isAuthenticated: false,
        });
      },

      checkAuth: async () => {
        const { accessToken, refreshToken } = get();
        if (!accessToken) return;

        try {
          api.defaults.headers.common['Authorization'] = `Bearer ${accessToken}`;
          const response = await api.get('/api/v1/auth/me');
          set({
            user: response.data.user,
            tenant: response.data.tenant,
            isAuthenticated: true,
          });
        } catch (error) {
          // Try to refresh token
          if (refreshToken) {
            try {
              await get().refreshAccessToken();
              await get().checkAuth();
            } catch {
              get().logout();
            }
          } else {
            get().logout();
          }
        }
      },

      refreshAccessToken: async () => {
        const { refreshToken } = get();
        if (!refreshToken) throw new Error('No refresh token');

        const response = await api.post('/api/v1/auth/refresh', { refresh_token: refreshToken });
        const { access_token, refresh_token } = response.data;

        api.defaults.headers.common['Authorization'] = `Bearer ${access_token}`;
        set({ accessToken: access_token, refreshToken: refresh_token });
      },

      toggleSidebar: () => set((state) => ({ isSidebarOpen: !state.isSidebarOpen })),
      setSidebarOpen: (open) => set({ isSidebarOpen: open }),

      setTheme: (theme) => {
        set({ theme });
        const root = document.documentElement;
        if (theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
          root.classList.add('dark');
        } else {
          root.classList.remove('dark');
        }
      },

      setLanguage: (language) => set({ language }),
    }),
    {
      name: 'mediaedge-auth',
      partialize: (state) => ({
        accessToken: state.accessToken,
        refreshToken: state.refreshToken,
        theme: state.theme,
        language: state.language,
        isSidebarOpen: state.isSidebarOpen,
      }),
    }
  )
);

// Initialize theme on load
if (typeof window !== 'undefined') {
  const stored = localStorage.getItem('mediaedge-auth');
  if (stored) {
    try {
      const { state } = JSON.parse(stored);
      const theme = state.theme || 'system';
      const root = document.documentElement;
      if (theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)) {
        root.classList.add('dark');
      }
    } catch {}
  }
}