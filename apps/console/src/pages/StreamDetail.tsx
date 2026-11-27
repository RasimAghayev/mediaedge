import { useTranslation } from 'react-i18next';
import { useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/services/api';
import { Broadcast, Users, Activity, AlertTriangle, ExternalLink, Copy, Check, Download, Share2 } from 'lucide-react';
import { clsx } from 'clsx';
import { formatDistanceToNow, format } from 'date-fns';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';

interface Stream {
  id: string;
  name: string;
  status: 'live' | 'offline' | 'starting' | 'stopping' | 'error';
  protocol: string;
  bitrate: number;
  resolution: string;
  viewers: number;
  started_at: string;
  tenant_id: string;
  tenant_name: string;
  ingest_url: string;
  playback_urls: {
    hls?: string;
    dash?: string;
    whep?: string;
    moq?: string;
    rtmp?: string;
    srt?: string;
  };
  metrics: {
    bitrate_history: Array<{ timestamp: string; value: number }>;
    viewers_history: Array<{ timestamp: string; value: number }>;
    bandwidth_history: Array<{ timestamp: string; value: number }>;
    errors_history: Array<{ timestamp: string; value: number }>;
  };
}

export function StreamDetail() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();

  const { data: stream, isLoading } = useQuery({
    queryKey: ['stream', id],
    queryFn: () => api.get(`/api/v1/streams/${id}`),
    enabled: !!id,
    refetchInterval: 10000,
  });

  const { data: playbackUrls } = useQuery({
    queryKey: ['stream', id, 'playbackUrls'],
    queryFn: () => api.get(`/api/v1/streams/${id}/playback-urls`),
    enabled: !!id && stream?.data?.status === 'live',
  });

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Broadcast className="w-8 h-8 animate-spin text-primary-600" />
      </div>
    );
  }

  const s = stream?.data;

  if (!s) {
    return (
      <div className="text-center py-12">
        <Broadcast className="w-12 h-12 text-secondary-300 dark:text-secondary-600 mx-auto mb-4" />
        <p className="text-secondary-500">Stream not found</p>
      </div>
    );
  }

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="page-title">{s.name}</h1>
          <p className="page-subtitle">{s.tenant_name} • {s.protocol.toUpperCase()} • {formatDistanceToNow(new Date(s.started_at), { addSuffix: true })}</p>
        </div>
        <div className="flex items-center gap-2">
          <span className={clsx(
            'badge',
            s.status === 'live' && 'badge-success',
            s.status === 'offline' && 'badge-neutral',
            s.status === 'error' && 'badge-danger'
          )}>
            {t(`streams.${s.status}`)}
          </span>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <StatCard
          title={t('streams.protocol')}
          value={s.protocol.toUpperCase()}
          icon="broadcast"
        />
        <StatCard
          title={t('streams.bitrate')}
          value={formatBitrate(s.bitrate)}
          icon="activity"
        />
        <StatCard
          title={t('streams.resolution')}
          value={s.resolution}
          icon="activity"
        />
        <StatCard
          title={t('streams.viewers')}
          value={s.viewers.toLocaleString()}
          icon="users"
        />
      </div>

      <div className="card">
        <div className="card-header">
          <Tabs defaultValue="overview" onValueChange={console.log}>
            <TabsList>
              <TabsTrigger value="overview">{t('common.overview')}</TabsTrigger>
              <TabsTrigger value="metrics">{t('common.metrics')}</TabsTrigger>
              <TabsTrigger value="playback">{t('streams.playbackUrls')}</TabsTrigger>
              <TabsTrigger value="ingest">{t('streams.ingestUrl')}</TabsTrigger>
              <TabsTrigger value="logs">{t('common.logs')}</TabsTrigger>
            </TabsList>

            <TabsContent value="overview">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-4">
                <div>
                  <h3 className="font-medium text-secondary-900 dark:text-white mb-3">{t('streams.ingestUrl')}</h3>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={s.ingest_url}
                      readOnly
                      className="input flex-1 font-mono text-sm"
                    />
                    <button
                      onClick={() => navigator.clipboard.writeText(s.ingest_url)}
                      className="btn-secondary"
                      title="Copy to clipboard"
                    >
                      <Copy className="w-4 h-4" />
                    </button>
                  </div>
                </div>
                <div>
                  <h3 className="font-medium text-secondary-900 dark:text-white mb-3">{t('streams.tenant')}</h3>
                  <p className="text-secondary-700 dark:text-secondary-300">{s.tenant_name}</p>
                </div>
              </div>
            </TabsContent>

            <TabsContent value="metrics">
              <div className="mt-4 space-y-6">
                <MetricChart
                  title={t('streams.bitrate')}
                  data={s.metrics?.bitrate_history || []}
                  color="#0ea5e9"
                  unit="bps"
                />
                <MetricChart
                  title={t('streams.viewers')}
                  data={s.metrics?.viewers_history || []}
                  color="#22c55e"
                  unit=""
                />
                <MetricChart
                  title={t('dashboard.bandwidth')}
                  data={s.metrics?.bandwidth_history || []}
                  color="#f59e0b"
                  unit="B/s"
                />
                <MetricChart
                  title={t('dashboard.errorRate')}
                  data={s.metrics?.errors_history || []}
                  color="#ef4444"
                  unit=""
                />
              </div>
            </TabsContent>

            <TabsContent value="playback">
              <div className="mt-4 space-y-4">
                {playbackUrls?.data && Object.entries(playbackUrls.data).map(([protocol, url]) => (
                  <div key={protocol} className="flex items-center gap-4 p-4 bg-secondary-50 dark:bg-secondary-800 rounded-lg">
                    <span className="badge badge-info w-20">{protocol.toUpperCase()}</span>
                    <input
                      type="text"
                      value={url as string}
                      readOnly
                      className="input flex-1 font-mono text-sm"
                    />
                    <button
                      onClick={() => navigator.clipboard.writeText(url as string)}
                      className="btn-secondary"
                    >
                      <Copy className="w-4 h-4" />
                    </button>
                  </div>
                ))}
              </div>
            </TabsContent>

            <TabsContent value="ingest">
              <div className="mt-4">
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={s.ingest_url}
                    readOnly
                    className="input flex-1 font-mono text-sm"
                  />
                  <button
                    onClick={() => navigator.clipboard.writeText(s.ingest_url)}
                    className="btn-secondary"
                  >
                    <Copy className="w-4 h-4" />
                  </button>
                </div>
                <p className="text-sm text-secondary-500 mt-2">
                  {t('streams.ingestHelp')}
                </p>
              </div>
            </TabsContent>

            <TabsContent value="logs">
              <div className="mt-4">
                <pre className="bg-secondary-900 text-secondary-100 p-4 rounded-lg overflow-x-auto text-sm font-mono max-h-96">
                  {t('common.logsLoading')}
                </pre>
              </div>
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </div>
  );
}

function StatCard({ title, value, icon }: { title: string; value: string; icon: string }) {
  const icons = { broadcast: Broadcast, users: Users, activity: Activity, alertTriangle: AlertTriangle };
  const Icon = icons[icon as keyof typeof icons];

  return (
    <div className="card">
      <div className="card-body">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-secondary-500 dark:text-secondary-400">{title}</p>
            <p className="text-2xl font-bold text-secondary-900 dark:text-white">{value}</p>
          </div>
          <div className="w-10 h-10 rounded-lg bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
            <Icon className="w-5 h-5 text-primary-600 dark:text-primary-400" />
          </div>
        </div>
      </div>
    </div>
  );
}

function MetricChart({ title, data, color, unit }: { title: string; data: Array<{ timestamp: string; value: number }>; color: string; unit: string }) {
  return (
    <div>
      <h3 className="font-medium text-secondary-900 dark:text-white mb-2">{title}</h3>
      <div className="h-40 bg-secondary-50 dark:bg-secondary-800 rounded-lg flex items-end justify-around p-2">
        {data.slice(-50).map((point, i) => (
          <div
            key={i}
            className="flex-1 max-w-4"
            style={{
              height: `${Math.min(100, (point.value / (Math.max(...data.map(d => d.value)) || 1)) * 100)}%`,
              backgroundColor: color,
              borderRadius: '2px 2px 0 0',
              transition: 'height 0.3s ease',
            }}
            title={`${format(new Date(point.timestamp), 'HH:mm:ss')} - ${point.value.toLocaleString()} ${unit}`}
          />
        ))}
      </div>
    </div>
  );
}

function formatBitrate(bps: number): string {
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(1)} Gbps`;
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} Mbps`;
  if (bps >= 1e3) return `${(bps / 1e3).toFixed(1)} Kbps`;
  return `${bps} bps`;
}