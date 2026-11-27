import { NavLink, useLocation } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  LayoutDashboard,
  Broadcast,
  Server,
  Users,
  CreditCard,
  Settings,
  ChevronLeft,
  ChevronRight,
  Menu,
  X,
  Building2,
  TV,
  ShoppingBag,
  Package,
  BarChart3,
} from 'lucide-react';
import { useAuthStore } from '@/store/authStore';
import { clsx } from 'clsx';

const navigation = [
  { name: 'dashboard', href: '/dashboard', icon: LayoutDashboard },
  { name: 'streams', href: '/streams', icon: Broadcast },
  { name: 'cluster', href: '/cluster', icon: Server },
  { name: 'tenants', href: '/tenants', icon: Building2 },
  { name: 'monetization', href: '/monetization', icon: CreditCard },
  { name: 'users', href: '/users', icon: Users },
  { name: 'settings', href: '/settings', icon: Settings },
];

const monetizationSubItems = [
  { name: 'fastChannels', href: '/monetization?tab=fast', icon: TV },
  { name: 'tvod', href: '/monetization?tab=tvod', icon: ShoppingBag },
  { name: 'bundles', href: '/monetization?tab=bundles', icon: Package },
  { name: 'billing', href: '/monetization?tab=billing', icon: CreditCard },
];

export function Sidebar() {
  const { t } = useTranslation();
  const location = useLocation();
  const { isSidebarOpen, toggleSidebar, tenant } = useAuthStore();

  return (
    <aside
      className={clsx(
        'fixed inset-y-0 left-0 z-50 bg-white dark:bg-secondary-900 border-r border-secondary-200 dark:border-secondary-700 transition-all duration-300 flex flex-col',
        isSidebarOpen ? 'w-64' : 'w-20'
      )}
      aria-label="Main navigation"
    >
      <div className="flex h-16 items-center justify-between px-4 border-b border-secondary-200 dark:border-secondary-700">
        {isSidebarOpen && (
          <div className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-primary-600 flex items-center justify-center">
              <Broadcast className="w-5 h-5 text-white" />
            </div>
            <span className="font-bold text-xl text-secondary-900 dark:text-white">MediaEdge</span>
          </div>
        )}
        <button
          onClick={toggleSidebar}
          className={clsx(
            'p-2 rounded-lg text-secondary-500 hover:text-secondary-700 hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors',
            isSidebarOpen ? 'ml-auto' : 'mx-auto'
          )}
          aria-label={isSidebarOpen ? t('common.collapse') : t('common.expand')}
          aria-expanded={isSidebarOpen}
        >
          {isSidebarOpen ? <ChevronLeft className="w-5 h-5" /> : <ChevronRight className="w-5 h-5" />}
        </button>
      </div>

      <nav className="flex-1 overflow-y-auto p-4 space-y-1" role="navigation" aria-label="Main menu">
        {navigation.map((item) => {
          const isActive = location.pathname.startsWith(item.href);
          return (
            <NavLink
              key={item.name}
              to={item.href}
              className={({ isActive }) =>
                clsx(
                  'sidebar-link',
                  isActive && 'sidebar-link-active',
                  !isSidebarOpen && 'justify-center px-0'
                )
              }
              title={isSidebarOpen ? undefined : t(item.name)}
              aria-current={isActive ? 'page' : undefined}
            >
              <item.icon className="w-5 h-5 flex-shrink-0" aria-hidden="true" />
              {isSidebarOpen && <span>{t(item.name)}</span>}
            </NavLink>
          );
        })}

        {isSidebarOpen && (
          <div className="pt-4 mt-4 border-t border-secondary-200 dark:border-secondary-700">
            <p className="px-4 text-xs font-semibold text-secondary-500 dark:text-secondary-400 uppercase tracking-wider">
              {t('monetization.title')}
            </p>
            {monetizationSubItems.map((item) => {
              const isActive = location.pathname === item.href;
              return (
                <NavLink
                  key={item.name}
                  to={item.href}
                  className={({ isActive }) =>
                    clsx(
                      'sidebar-link text-sm',
                      isActive && 'sidebar-link-active bg-primary-50 dark:bg-primary-900/20',
                    )
                  }
                  aria-current={isActive ? 'page' : undefined}
                >
                  <item.icon className="w-5 h-5 flex-shrink-0" aria-hidden="true" />
                  <span>{t(`monetization.${item.name}`)}</span>
                </NavLink>
              );
            })}
          </div>
        )}
      </nav>

      <div className="p-4 border-t border-secondary-200 dark:border-secondary-700">
        {isSidebarOpen && tenant && (
          <div className="mb-4">
            <p className="text-xs font-medium text-secondary-500 dark:text-secondary-400 uppercase tracking-wider mb-2">
              {t('common.tenant')}
            </p>
            <div className="flex items-center gap-3 p-3 bg-secondary-50 dark:bg-secondary-800 rounded-lg">
              <div className="w-8 h-8 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
                <Building2 className="w-4 h-4 text-primary-600 dark:text-primary-400" />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-secondary-900 dark:text-white truncate">{tenant.name}</p>
                <p className="text-xs text-secondary-500 dark:text-secondary-400 truncate">{tenant.slug}</p>
              </div>
            </div>
          </div>
        )}

        <div className={clsx('flex items-center gap-3', !isSidebarOpen && 'justify-center')}>
          <div className="w-8 h-8 rounded-full bg-secondary-100 dark:bg-secondary-800 flex items-center justify-center flex-shrink-0">
            <Menu className="w-4 h-4 text-secondary-500" />
          </div>
          {isSidebarOpen && (
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-secondary-900 dark:text-white">
                {useAuthStore.getState().user?.name || 'User'}
              </p>
              <p className="text-xs text-secondary-500 dark:text-secondary-400">
                {useAuthStore.getState().user?.email || ''}
              </p>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}