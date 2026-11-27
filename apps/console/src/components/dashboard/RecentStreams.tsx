import { useTranslation } from 'react-i18next';
import { Broadcast, Users, Activity, ExternalLink, MoreHorizontal } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import { clsx } from 'clsx';

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
}

interface RecentStreamsProps {
  streams: Stream[];
}

export function RecentStreams({ streams }: RecentStreamsProps) {
  const { t } = useTranslation();

  if (streams.length === 0) {
    return (
      <div className="text-center py-12">
        <Broadcast className="w-12 h-12 text-secondary-300 dark:text-secondary-600 mx-auto mb-4" />
        <p className="text-secondary-500 dark:text-secondary-400">{t('streams.noStreams')}</p>
      </div>
    );
  }

  return (
    <div className="table-container">
      <table className="table">
        <thead>
          <tr>
            <th>{t('streams.name')}</th>
            <th className="hidden md:table-cell">{t('streams.status')}</th>
            <th className="hidden lg:table-cell">{t('streams.protocol')}</th>
            <th className="hidden lg:table-cell">{t('streams.bitrate')}</th>
            <th className="hidden lg:table-cell">{t('streams.resolution')}</th>
            <th className="hidden md:table-cell">{t('streams.viewers')}</th>
            <th className="hidden lg:table-cell">{t('streams.startedAt')}</th>
            <th className="w-24">{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {streams.slice(0, 10).map((stream) => (
            <tr key={stream.id}>
              <td className="font-medium">
                <a href={`/streams/${stream.id}`} className="hover:text-primary-600 dark:hover:text-primary-400">
                  {stream.name}
                </a>
              </td>
              <td className="hidden md:table-cell">
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
              <td className="hidden lg:table-cell text-secondary-500 dark:text-secondary-400">
                {stream.protocol.toUpperCase()}
              </td>
              <td className="hidden lg:table-cell text-secondary-500 dark:text-secondary-400">
                {formatBitrate(stream.bitrate)}
              </td>
              <td className="hidden lg:table-cell text-secondary-500 dark:text-secondary-400">
                {stream.resolution}
              </td>
              <td className="hidden md:table-cell">
                <div className="flex items-center gap-1">
                  <Users className="w-4 h-4" />
                  <span>{stream.viewers.toLocaleString()}</span>
                </div>
              </td>
              <td className="hidden lg:table-cell text-secondary-500 dark:text-secondary-400">
                {formatDistanceToNow(new Date(stream.started_at), { addSuffix: true })}
              </td>
              <td>
                <div className="flex items-center justify-end gap-1">
                  <a
                    href={`/streams/${stream.id}`}
                    className="p-1.5 rounded text-secondary-400 hover:text-secondary-600 dark:hover:text-secondary-300"
                    title="View details"
                  >
                    <ExternalLink className="w-4 h-4" />
                  </a>
                  <button
                    className="p-1.5 rounded text-secondary-400 hover:text-secondary-600 dark:hover:text-secondary-300"
                    title="More options"
                  >
                    <MoreHorizontal className="w-4 h-4" />
                  </button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function formatBitrate(bps: number): string {
  if (bps >= 1e9) return `${(bps / 1e9).toFixed(1)} Gbps`;
  if (bps >= 1e6) return `${(bps / 1e6).toFixed(1)} Mbps`;
  if (bps >= 1e3) return `${(bps / 1e3).toFixed(1)} Kbps`;
  return `${bps} bps`;
}