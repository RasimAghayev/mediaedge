import { useTranslation } from 'react-i18next';
import { useAuthStore } from '@/store/authStore';
import { Menu, Bell, Moon, Sun, Monitor, Globe, LogOut, User, Shield, HelpCircle } from 'lucide-react';
import { useState, useRef, useEffect } from 'react';
import { clsx } from 'clsx';

interface HeaderProps {
  onMenuClick: () => void;
}

export function Header({ onMenuClick }: HeaderProps) {
  const { t } = useTranslation();
  const { user, tenant, logout, theme, setTheme, language, setLanguage } = useAuthStore();
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [userMenuOpen, setUserMenuOpen] = useState(false);
  const [themeMenuOpen, setThemeMenuOpen] = useState(false);
  const [langMenuOpen, setLangMenuOpen] = useState(false);
  const notificationsRef = useRef<HTMLDivElement>(null);
  const userMenuRef = useRef<HTMLDivElement>(null);
  const themeMenuRef = useRef<HTMLDivElement>(null);
  const langMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (notificationsRef.current && !notificationsRef.current.contains(event.target as Node)) {
        setNotificationsOpen(false);
      }
      if (userMenuRef.current && !userMenuRef.current.contains(event.target as Node)) {
        setUserMenuOpen(false);
      }
      if (themeMenuRef.current && !themeMenuRef.current.contains(event.target as Node)) {
        setThemeMenuOpen(false);
      }
      if (langMenuRef.current && !langMenuRef.current.contains(event.target as Node)) {
        setLangMenuOpen(false);
      }
    }

    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const themes = [
    { value: 'light' as const, label: t('common.light'), icon: Sun },
    { value: 'dark' as const, label: t('common.dark'), icon: Moon },
    { value: 'system' as const, label: t('common.system'), icon: Monitor },
  ];

  const languages = [
    { value: 'en', label: 'English', flag: '🇺🇸' },
    { value: 'az', label: 'Azərbaycan dili', flag: '🇦🇿' },
  ];

  return (
    <header className="sticky top-0 z-40 bg-white/80 dark:bg-secondary-900/80 backdrop-blur-sm border-b border-secondary-200 dark:border-secondary-700">
      <div className="flex h-16 items-center justify-between px-4 lg:px-6">
        <div className="flex items-center gap-4">
          <button
            onClick={onMenuClick}
            className="lg:hidden p-2 rounded-lg text-secondary-500 hover:text-secondary-700 hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors"
            aria-label="Open menu"
          >
            <Menu className="w-6 h-6" />
          </button>
        </div>

        <div className="flex-1 lg:flex-none" />

        <div className="flex items-center gap-2">
          {/* Notifications */}
          <div className="relative" ref={notificationsRef}>
            <button
              onClick={() => setNotificationsOpen(!notificationsOpen)}
              className="relative p-2 rounded-lg text-secondary-500 hover:text-secondary-700 hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors"
              aria-label="Notifications"
              aria-expanded={notificationsOpen}
            >
              <Bell className="w-5 h-5" />
              <span className="absolute top-1 right-1 w-2 h-2 bg-danger-500 rounded-full" />
            </button>

            {notificationsOpen && (
              <div className="absolute right-0 mt-2 w-80 bg-white dark:bg-secondary-800 rounded-lg shadow-lg border border-secondary-200 dark:border-secondary-700 py-2 animate-in">
                <div className="px-4 py-3 border-b border-secondary-200 dark:border-secondary-700">
                  <h3 className="font-semibold text-secondary-900 dark:text-white">{t('common.alerts')}</h3>
                </div>
                <div className="max-h-96 overflow-y-auto">
                  <div className="py-4 px-4 text-center text-secondary-500 dark:text-secondary-400">
                    No notifications
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* Theme */}
          <div className="relative" ref={themeMenuRef}>
            <button
              onClick={() => setThemeMenuOpen(!themeMenuOpen)}
              className="p-2 rounded-lg text-secondary-500 hover:text-secondary-700 hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors"
              aria-label="Theme"
              aria-expanded={themeMenuOpen}
            >
              {theme === 'dark' ? <Moon className="w-5 h-5" /> : <Sun className="w-5 h-5" />}
            </button>

            {themeMenuOpen && (
              <div className="absolute right-0 mt-2 w-40 bg-white dark:bg-secondary-800 rounded-lg shadow-lg border border-secondary-200 dark:border-secondary-700 py-1 animate-in">
                {themes.map((t) => (
                  <button
                    key={t.value}
                    onClick={() => {
                      setTheme(t.value);
                      setThemeMenuOpen(false);
                    }}
                    className={clsx(
                      'w-full flex items-center gap-3 px-4 py-2 text-sm text-secondary-700 dark:text-secondary-300 hover:bg-secondary-100 dark:hover:bg-secondary-700',
                      theme === t.value && 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                    )}
                  >
                    <t.icon className="w-5 h-5" />
                    <span>{t.label}</span>
                    {theme === t.value && <span className="ml-auto text-primary-600 dark:text-primary-400">✓</span>}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Language */}
          <div className="relative" ref={langMenuRef}>
            <button
              onClick={() => setLangMenuOpen(!langMenuOpen)}
              className="p-2 rounded-lg text-secondary-500 hover:text-secondary-700 hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors"
              aria-label="Language"
              aria-expanded={langMenuOpen}
            >
              <Globe className="w-5 h-5" />
            </button>

            {langMenuOpen && (
              <div className="absolute right-0 mt-2 w-40 bg-white dark:bg-secondary-800 rounded-lg shadow-lg border border-secondary-200 dark:border-secondary-700 py-1 animate-in">
                {languages.map((l) => (
                  <button
                    key={l.value}
                    onClick={() => {
                      setLanguage(l.value);
                      setLangMenuOpen(false);
                    }}
                    className={clsx(
                      'w-full flex items-center gap-3 px-4 py-2 text-sm text-secondary-700 dark:text-secondary-300 hover:bg-secondary-100 dark:hover:bg-secondary-700',
                      language === l.value && 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                    )}
                  >
                    <span>{l.flag}</span>
                    <span>{l.label}</span>
                    {language === l.value && <span className="ml-auto text-primary-600 dark:text-primary-400">✓</span>}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* User Menu */}
          <div className="relative" ref={userMenuRef}>
            <button
              onClick={() => setUserMenuOpen(!userMenuOpen)}
              className="flex items-center gap-3 p-2 rounded-lg hover:bg-secondary-100 dark:hover:bg-secondary-800 transition-colors"
              aria-label="User menu"
              aria-expanded={userMenuOpen}
            >
              <div className="w-8 h-8 rounded-full bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
                <User className="w-4 h-4 text-primary-600 dark:text-primary-400" />
              </div>
              <div className="hidden md:block text-left">
                <p className="text-sm font-medium text-secondary-900 dark:text-white">{user?.name || 'User'}</p>
                <p className="text-xs text-secondary-500 dark:text-secondary-400">{tenant?.name || ''}</p>
              </div>
            </button>

            {userMenuOpen && (
              <div className="absolute right-0 mt-2 w-56 bg-white dark:bg-secondary-800 rounded-lg shadow-lg border border-secondary-200 dark:border-secondary-700 py-1 animate-in">
                <div className="px-4 py-3 border-b border-secondary-200 dark:border-secondary-700">
                  <p className="font-medium text-secondary-900 dark:text-white">{user?.name || 'User'}</p>
                  <p className="text-sm text-secondary-500 dark:text-secondary-400">{user?.email || ''}</p>
                </div>
                <button
                  onClick={() => {
                    setUserMenuOpen(false);
                    // Navigate to profile
                  }}
                  className="w-full flex items-center gap-3 px-4 py-2 text-sm text-secondary-700 dark:text-secondary-300 hover:bg-secondary-100 dark:hover:bg-secondary-700"
                >
                  <User className="w-5 h-5" />
                  <span>{t('common.profile')}</span>
                </button>
                <button
                  onClick={() => {
                    setUserMenuOpen(false);
                    // Navigate to settings
                  }}
                  className="w-full flex items-center gap-3 px-4 py-2 text-sm text-secondary-700 dark:text-secondary-300 hover:bg-secondary-100 dark:hover:bg-secondary-700"
                >
                  <Shield className="w-5 h-5" />
                  <span>{t('common.settings')}</span>
                </button>
                <div className="border-t border-secondary-200 dark:border-secondary-700 my-1" />
                <button
                  onClick={() => {
                    logout();
                    setUserMenuOpen(false);
                  }}
                  className="w-full flex items-center gap-3 px-4 py-2 text-sm text-danger-600 dark:text-danger-400 hover:bg-secondary-100 dark:hover:bg-secondary-700"
                >
                  <LogOut className="w-5 h-5" />
                  <span>{t('common.logout')}</span>
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </header>
  );
}