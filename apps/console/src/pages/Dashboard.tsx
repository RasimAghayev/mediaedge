import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { api, type Stream } from '@/services/api';
import { StatCard } from '@/components/dashboard/StatCard';
import { StreamsChart } from '@/components/dashboard/StreamsChart';
import { RecentStreams } from '@/components/dashboard/RecentStreams';
import { ErrorHeatmap } from '@/components/dashboard/ErrorHeatmap';
import { clsx } from 'clsx';
import { useAlerts, useClusterHealth } from '@/services/realtime';
import { announceToScreenReader } from '@/utils/accessibility';
import { AlertTriangle, Wifi, WifiOff, Bell, CheckCircle, AlertCircle, Info } from 'lucide-react';

export function Dashboard() {
  const { t } = useTranslation();
  const { alerts, connected: alertsConnected } = useAlerts();
  const { nodes, connected: clusterConnected } = useClusterHealth();
  const lastAlertCountRef = useRef(alerts.length);

  useEffect(() => {
    if (alerts.length > lastAlertCountRef.current) {
      const newAlerts = alerts.slice(0, alerts.length - lastAlertCountRef.current);
      for (const alert of newAlerts) {
        announceToScreenReader(
          `${alert.severity === 'critical' ? 'Critical alert' : alert.severity === 'warning' ? 'Warning' : 'Info'}: ${alert.title}. ${alert.message}`,
          alert.severity === 'critical' ? 'assertive' : 'polite'
        );
      }
    }
    lastAlertCountRef.current = alerts.length;
  }, [alerts]);

  const { data: stats } = useQuery({
    queryKey: ['dashboard', 'stats'],
    queryFn: async () => {
      const [streamsRes, viewersRes, bandwidthRes, errorsRes] = await Promise.all([
        api.get('/api/v1/streams', { params: { limit: 1 } }),
        api.get('/api/v1/metrics/query', { query: 'sum(rate(viewers_total[5m]))' }),
        api.get('/api/v1/metrics/query', { query: 'sum(rate(bandwidth_bytes_total[5m]))' }),
        api.get('/api/v1/metrics/query', { query: 'sum(rate(errors_total[5m]))' }),
      ]);

      return {
        liveStreams: streamsRes.data.total || 0,
        concurrentViewers: viewersRes.data.result?.[0]?.value?.[1] || 0,
        bandwidth: bandwidthRes.data.result?.[0]?.value?.[1] || 0,
        errorRate: errorsRes.data.result?.[0]?.value?.[1] || 0,
      };
    },
    refetchInterval: 30000,
  });

  const { data: recentStreams } = useQuery({
    queryKey: ['dashboard', 'recentStreams'],
    queryFn: () => api.get('/api/v1/streams', { params: { limit: 10, sort: '-created_at' } }),
    refetchInterval: 30000,
  });

  const criticalAlerts = alerts.filter(a => a.severity === 'critical').length;
  const warningAlerts = alerts.filter(a => a.severity === 'warning').length;
  const healthyNodes = nodes.filter(n => n.status === 'healthy').length;
  const totalNodes = nodes.length || 1;

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('dashboard.title')}</h1>
          <p className="page-subtitle">{t('dashboard.overview')}</p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2 px-3 py-1 rounded-lg bg-secondary-100 dark:bg-secondary-800">
            <span className={`w-2 h-2 rounded-full ${alertsConnected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-sm text-secondary-700 dark:text-secondary-300">
              {alertsConnected ? t('dashboard.realtimeConnected') : t('dashboard.realtimeDisconnected')}
            </span>
          </div>
          <div className="flex items-center gap-2 px-3 py-1 rounded-lg bg-secondary-100 dark:bg-secondary-800">
            <span className={`w-2 h-2 rounded-full ${clusterConnected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-sm text-secondary-700 dark:text-secondary-300">
              {t('dashboard.cluster')}: {healthyNodes}/{totalNodes} {t('dashboard.healthy')}
            </span>
          </div>
          <select
            className="input w-auto"
            defaultValue="1h"
            onChange={(e) => console.log('Time range:', e.target.value)}
          >
            <option value="1h">{t('dashboard.lastHour')}</option>
            <option value="24h">{t('dashboard.last24Hours')}</option>
            <option value="7d">{t('dashboard.last7Days')}</option>
          </select>
        </div>
      </div>

      {(criticalAlerts > 0 || warningAlerts > 0) && (
        <div className="card border-yellow-200 dark:border-yellow-800 bg-yellow-50 dark:bg-yellow-900/20" role="alert" aria-live="polite">
          <div className="card-body">
            <div className="flex items-center gap-3">
              <AlertTriangle className="text-yellow-600 dark:text-yellow-400" size={24} aria-hidden="true" />
              <div className="flex-1">
                <p className="font-medium text-yellow-900 dark:text-yellow-100">
                  {t('dashboard.activeAlerts')}
                </p>
                <p className="text-sm text-yellow-700 dark:text-yellow-300">
                  {criticalAlerts > 0 && (
                    <span className="mr-2">{criticalAlerts} {t('dashboard.critical')}</span>
                  )}
                  {warningAlerts > 0 && (
                    <span>{warningAlerts} {t('dashboard.warning')}</span>
                  )}
                </p>
              </div>
              <a href="/alerts" className="text-sm text-yellow-700 dark:text-yellow-300 hover:underline font-medium">
                {t('dashboard.viewAll')}
              </a>
            </div>
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title={t('dashboard.liveStreams')}
          value={stats?.liveStreams?.toLocaleString() || '0'}
          icon="broadcast"
          trend="+12%"
          trendUp
        />
        <StatCard
          title={t('dashboard.concurrentViewers')}
          value={stats?.concurrentViewers ? formatNumber(stats.concurrentViewers) : '0'}
          icon="users"
          trend="+8%"
          trendUp
        />
        <StatCard
          title={t('dashboard.bandwidth')}
          value={formatBandwidth(stats?.bandwidth || 0)}
          icon="activity"
          trend="-3%"
          trendUp={false}
        />
        <StatCard
          title={t('dashboard.errorRate')}
          value={formatPercentage(stats?.errorRate || 0)}
          icon="alertTriangle"
          trend="+0.1%"
          trendUp={false}
        />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="card">
          <div className="card-header flex items-center justify-between">
            <h2 className="text-lg font-semibold text-secondary-900 dark:text-white">
              {t('dashboard.realTime')} {t('dashboard.liveStreams')}
            </h2>
            <span className={`flex items-center gap-1 text-xs ${alertsConnected ? 'text-green-600' : 'text-red-600'}`}>
              <span className={`w-1.5 h-1.5 rounded-full ${alertsConnected ? 'bg-green-500' : 'bg-red-500'}`} aria-hidden="true" />
              {alertsConnected ? t('dashboard.live') : t('dashboard.polling')}
            </span>
          </div>
          <div className="card-body">
            <StreamsChart />
          </div>
        </div>

        <div className="card">
          <div className="card-header flex items-center justify-between">
            <h2 className="text-lg font-semibold text-secondary-900 dark:text-white">
              {t('dashboard.errorRate')} Heatmap
            </h2>
            <span className={`flex items-center gap-1 text-xs ${clusterConnected ? 'text-green-600' : 'text-red-600'}`}>
              <span className={`w-1.5 h-1.5 rounded-full ${clusterConnected ? 'bg-green-500' : 'bg-red-500'}`} aria-hidden="true" />
              {clusterConnected ? t('dashboard.live') : t('dashboard.polling')}
            </span>
          </div>
          <div className="card-body">
            <ErrorHeatmap />
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header flex items-center justify-between">
          <h2 className="text-lg font-semibold text-secondary-900 dark:text-white">
            {t('streams.title')}
          </h2>
          <a href="/streams" className="text-sm text-primary-600 hover:text-primary-700 font-medium">
            View all →
          </a>
        </div>
        <div className="card-body p-0">
          <RecentStreams streams={recentStreams?.data?.streams || []} />
        </div>
      </div>
    </div>
  );
}

function formatNumber(num: number): string {
  if (num >= 1e9) return (num / 1e9).toFixed(1) + 'B';
  if (num >= 1e6) return (num / 1e6).toFixed(1) + 'M';
  if (num >= 1e3) return (num / 1e3).toFixed(1) + 'K';
  return num.toLocaleString();
}

function formatBandwidth(bytes: number): string {
  const units = ['B/s', 'KB/s', 'MB/s', 'GB/s', 'TB/s'];
  let i = 0;
  while (bytes >= 1024 && i < units.length - 1) {
    bytes /= 1024;
    i++;
  }
  return `${bytes.toFixed(1)} ${units[i]}`;
}

function formatPercentage(rate: number): string {
  return `${(rate * 100).toFixed(2)}%`;
}