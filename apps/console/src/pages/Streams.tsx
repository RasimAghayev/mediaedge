import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, type CreateStreamRequest } from '@/services/api';
import { useSSE } from '@/services/realtime';
import { Broadcast, Plus, Search, Filter, MoreHorizontal, Play, Stop, Edit, Trash2, ExternalLink, Wifi, WifiOff } from 'lucide-react';
import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { formatDistanceToNow } from 'date-fns';

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
}

export function Streams() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('');
  const [protocolFilter, setProtocolFilter] = useState('');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [sseConnected, setSseConnected] = useState(false);

  const { data, isLoading } = useQuery({
    queryKey: ['streams', search, statusFilter, protocolFilter],
    queryFn: () => api.get('/api/v1/streams', {
      params: {
        search,
        status: statusFilter,
        protocol: protocolFilter,
        limit: 50,
      },
    }),
    refetchInterval: 30000,
  });

  useSSE(
    ['stream.created', 'stream.updated', 'stream.deleted', 'stream.started', 'stream.stopped', 'stream.error'],
    (event) => {
      queryClient.invalidateQueries({ queryKey: ['streams'] });
    },
    []
  );

  useEffect(() => {
    const checkConnection = setInterval(() => {
      // We'll use a simple check - in real implementation, we'd check the SSE service
      setSseConnected(true); // Placeholder
    }, 5000);
    return () => clearInterval(checkConnection);
  }, []);

  const createMutation = useMutation({
    mutationFn: (data: CreateStreamRequest) => api.post('/api/v1/streams', data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['streams'] });
      setShowCreateModal(false);
    },
  });

  const startMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/v1/streams/${id}/start`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['streams'] }),
  });

  const stopMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/v1/streams/${id}/stop`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['streams'] }),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/v1/streams/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['streams'] }),
  });

  const protocols = ['rtmp', 'srt', 'rist', 'whip', 'rtsp'];
  const statuses = ['live', 'offline', 'starting', 'stopping', 'error'];

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="page-title">{t('streams.title')}</h1>
          <p className="page-subtitle">{t('streams.manageStreams')}</p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2 px-3 py-1 rounded-lg bg-secondary-100 dark:bg-secondary-800">
            <span className={`w-2 h-2 rounded-full ${sseConnected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-sm text-secondary-700 dark:text-secondary-300">
              {sseConnected ? t('dashboard.realtimeConnected') : t('dashboard.realtimeDisconnected')}
            </span>
          </div>
          <button
            onClick={() => setShowCreateModal(true)}
            className="btn-primary"
          >
            <Plus className="w-4 h-4" />
            {t('streams.createStream')}
          </button>
        </div>
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
                <option key={s} value={s}>{t(`streams.${s}`)}</option>
              ))}
            </select>
            <select
              value={protocolFilter}
              onChange={(e) => setProtocolFilter(e.target.value)}
              className="input w-auto"
            >
              <option value="">{t('common.allProtocols')}</option>
              {protocols.map((p) => (
                <option key={p} value={p}>{p.toUpperCase()}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="table-container">
          <table className="table">
            <thead>
              <tr>
                <th>{t('streams.name')}</th>
                <th>{t('streams.tenant')}</th>
                <th>{t('streams.status')}</th>
                <th>{t('streams.protocol')}</th>
                <th>{t('streams.bitrate')}</th>
                <th>{t('streams.resolution')}</th>
                <th>{t('streams.viewers')}</th>
                <th>{t('streams.startedAt')}</th>
                <th className="w-32">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={9} className="text-center py-8">
                    <div className="flex items-center justify-center gap-2">
                      <Broadcast className="w-6 h-6 animate-spin text-primary-600" />
                      <span className="text-secondary-500">{t('common.loading')}</span>
                    </div>
                  </td>
                </tr>
              ) : data?.data?.streams?.length === 0 ? (
                <tr>
                  <td colSpan={9} className="text-center py-8">
                    <Broadcast className="w-12 h-12 text-secondary-300 dark:text-secondary-600 mx-auto mb-4" />
                    <p className="text-secondary-500 dark:text-secondary-400">{t('streams.noStreams')}</p>
                  </td>
                </tr>
              ) : (
                data?.data?.streams?.map((stream: Stream) => (
                  <tr key={stream.id}>
                    <td>
                      <a href={`/streams/${stream.id}`} className="font-medium hover:text-primary-600 dark:hover:text-primary-400">
                        {stream.name}
                      </a>
                    </td>
                    <td className="text-secondary-500 dark:text-secondary-400">{stream.tenant_name}</td>
                    <td>
                      <span className={clsx(
                        'badge',
                        stream.status === 'live' && 'badge-success',
                        stream.status === 'offline' && 'badge-neutral',
                        stream.status === 'starting' && 'badge-info',
                        stream.status === 'stopping' && 'badge-warning',
                        stream.status === 'error' && 'badge-danger'
                      )}>
                        {t(`streams.${stream.status}`)}
                      </span>
                    </td>
                    <td className="font-mono text-sm">{stream.protocol.toUpperCase()}</td>
                    <td className="text-secondary-500 dark:text-secondary-400">{formatBitrate(stream.bitrate)}</td>
                    <td className="text-secondary-500 dark:text-secondary-400">{stream.resolution}</td>
                    <td>{stream.viewers.toLocaleString()}</td>
                    <td className="text-secondary-500 dark:text-secondary-400">
                      {formatDistanceToNow(new Date(stream.started_at), { addSuffix: true })}
                    </td>
                    <td>
                      <div className="flex items-center justify-end gap-1">
                        <a
                          href={`/streams/${stream.id}`}
                          className="p-1.5 rounded text-secondary-400 hover:text-secondary-600 dark:hover:text-secondary-300"
                          title={t('common.view')}
                        >
                          <ExternalLink className="w-4 h-4" />
                        </a>
                        {stream.status !== 'live' && (
                          <button
                            onClick={() => startMutation.mutate(stream.id)}
                            disabled={startMutation.isPending}
                            className="p-1.5 rounded text-success-600 hover:bg-success-100 dark:hover:bg-success-900/30 disabled:opacity-50"
                            title={t('streams.start')}
                          >
                            <Play className="w-4 h-4" />
                          </button>
                        )}
                        {stream.status === 'live' && (
                          <button
                            onClick={() => stopMutation.mutate(stream.id)}
                            disabled={stopMutation.isPending}
                            className="p-1.5 rounded text-warning-600 hover:bg-warning-100 dark:hover:bg-warning-900/30 disabled:opacity-50"
                            title={t('streams.stop')}
                          >
                            <Stop className="w-4 h-4" />
                          </button>
                        )}
                        <button
                          onClick={() => deleteMutation.mutate(stream.id)}
                          disabled={deleteMutation.isPending}
                          className="p-1.5 rounded text-danger-600 hover:bg-danger-100 dark:hover:bg-danger-900/30 disabled:opacity-50"
                          title={t('common.delete')}
                        >
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

      {showCreateModal && (
        <CreateStreamModal
          onClose={() => setShowCreateModal(false)}
          onSubmit={createMutation.mutate}
          isLoading={createMutation.isPending}
        />
      )}
    </div>
  );
}

function formatBitrate(bps: number): string {
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(1)} Gbps`;
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} Mbps`;
  if (bps >= 1e3) return `${(bps / 1e3).toFixed(1)} Kbps`;
  return `${bps} bps`;
}

function CreateStreamModal({ onClose, onSubmit, isLoading }: { onClose: () => void; onSubmit: (data: CreateStreamRequest) => void; isLoading: boolean }) {
  const { t } = useTranslation();
  const [formData, setFormData] = useState<CreateStreamRequest>({
    name: '',
    protocol: 'rtmp',
    tenant_id: '',
    settings: {},
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSubmit(formData);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50">
      <div className="bg-white dark:bg-secondary-900 rounded-xl shadow-xl max-w-md w-full max-h-[90vh] overflow-y-auto animate-in">
        <div className="p-6 border-b border-secondary-200 dark:border-secondary-700 flex items-center justify-between">
          <h2 className="text-xl font-semibold">{t('streams.createStream')}</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-secondary-100 dark:hover:bg-secondary-800">
            ✕
          </button>
        </div>
        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          <div>
            <label className="label">{t('streams.name')}</label>
            <input
              type="text"
              className="input"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              required
              placeholder="My Live Stream"
            />
          </div>
          <div>
            <label className="label">{t('streams.protocol')}</label>
            <select
              className="input"
              value={formData.protocol}
              onChange={(e) => setFormData({ ...formData, protocol: e.target.value as any })}
            >
              <option value="rtmp">RTMP</option>
              <option value="srt">SRT</option>
              <option value="rist">RIST</option>
              <option value="whip">WHIP</option>
              <option value="rtsp">RTSP</option>
            </select>
          </div>
          <div>
            <label className="label">{t('common.tenant')}</label>
            <select className="input" value={formData.tenant_id} onChange={(e) => setFormData({ ...formData, tenant_id: e.target.value })} required>
              <option value="">{t('common.selectTenant')}</option>
            </select>
          </div>
          <div className="flex justify-end gap-3 pt-4 border-t border-secondary-200 dark:border-secondary-700">
            <button type="button" onClick={onClose} className="btn-secondary">{t('common.cancel')}</button>
            <button type="submit" disabled={isLoading} className="btn-primary">
              {isLoading ? t('common.loading') : t('common.create')}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}