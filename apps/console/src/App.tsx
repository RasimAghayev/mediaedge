import { Routes, Route, Navigate } from 'react-router-dom';
import { Sidebar } from '@/components/layout/Sidebar';
import { Header } from '@/components/layout/Header';
import { Dashboard } from '@/pages/Dashboard';
import { Streams } from '@/pages/Streams';
import { StreamDetail } from '@/pages/StreamDetail';
import { Cluster } from '@/pages/Cluster';
import { Tenants } from '@/pages/Tenants';
import { TenantDetail } from '@/pages/TenantDetail';
import { Monetization } from '@/pages/Monetization';
import { Users } from '@/pages/Users';
import { Settings } from '@/pages/Settings';
import { Login } from '@/pages/Login';
import { useAuthStore } from '@/store/authStore';
import { useEffect } from 'react';
import { skipLink } from '@/utils/accessibility';

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, checkAuth } = useAuthStore();

  useEffect(() => {
    checkAuth();
  }, [checkAuth]);

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

function SkipLinks() {
  return (
    <ul className="skip-links" role="list" aria-label="Skip links">
      <li>
        <a href="#main-content" className="skip-link">
          Skip to main content
        </a>
      </li>
      <li>
        <a href="#main-navigation" className="skip-link">
          Skip to navigation
        </a>
      </li>
    </ul>
  );
}

function Layout() {
  const { isSidebarOpen, toggleSidebar } = useAuthStore();

  return (
    <div className="min-h-screen bg-secondary-50 dark:bg-secondary-900">
      <SkipLinks />
      <Sidebar />
      <div
        className={`transition-all duration-300 ${
          isSidebarOpen ? 'lg:pl-64' : 'lg:pl-20'
        }`}
      >
        <Header onMenuClick={toggleSidebar} />
        <main id="main-content" className="p-6 lg:p-8" role="main" tabIndex={-1}>
          {/* Page content rendered by routes */}
        </main>
      </div>
    </div>
  );
}

export function App() {
  return (
    <ProtectedRoute>
      <Layout>
        <Routes>
          <Route path="/" element={<Navigate to="/dashboard" replace />} />
          <Route path="/dashboard" element={<Dashboard />} />
          <Route path="/streams" element={<Streams />} />
          <Route path="/streams/:id" element={<StreamDetail />} />
          <Route path="/cluster" element={<Cluster />} />
          <Route path="/tenants" element={<Tenants />} />
          <Route path="/tenants/:id" element={<TenantDetail />} />
          <Route path="/monetization" element={<Monetization />} />
          <Route path="/users" element={<Users />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/login" element={<Login />} />
          <Route path="*" element={<Navigate to="/dashboard" replace />} />
        </Routes>
      </Layout>
    </ProtectedRoute>
  );
}