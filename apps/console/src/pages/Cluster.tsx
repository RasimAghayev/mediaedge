import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/services/api';
import { useClusterHealth } from '@/services/realtime';
import { Server, Cpu, HardDrive, Wifi, Monitor, Database, Globe, Zap, Shield, AlertTriangle, CheckCircle, XCircle, Users } from 'lucide-react';
import { clsx } from 'clsx';
import { formatDistanceToNow } from 'date-fns';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';

interface Node {
  id: string;
  region: string;
  status: 'healthy' | 'degraded' | 'offline';
  cpu_usage: number;
  memory_usage: number;
  disk_usage: number;
  bandwidth_usage: number;
  gpu_usage?: number;
  streams: number;
  viewers: number;
  last_heartbeat: string;
  capacity: {
    max_streams: number;
    max_viewers: number;
    max_bandwidth: number;
  };
}

interface Broadcast {
  id: string;
  stream_id: string;
  owner_node_id: string;
  status: 'active' | 'transferring' | 'failed';
  viewers: number;
  bitrate: number;
}

export function Cluster() {
  const { t } = useTranslation();
  const { nodes: realtimeNodes, connected } = useClusterHealth();

  const { data: nodes, isLoading: nodesLoading } = useQuery({
    queryKey: ['cluster', 'nodes'],
    queryFn: () => api.get('/api/v1/cluster/nodes'),
    refetchInterval: 30000,
  });

  const { data: broadcasts } = useQuery({
    queryKey: ['cluster', 'broadcasts'],
    queryFn: () => api.get('/api/v1/cluster/broadcasts'),
    refetchInterval: 30000,
  });

  const { data: federation } = useQuery({
    queryKey: ['cluster', 'federation'],
    queryFn: () => api.get('/api/v1/cluster/federation'),
    refetchInterval: 60000,
  });

  const { data: mesh } = useQuery({
    queryKey: ['cluster', 'mesh'],
    queryFn: () => api.get('/api/v1/cluster/mesh'),
    refetchInterval: 60000,
  });

  const allNodes = nodes?.data?.nodes || [];
  const healthyCount = allNodes.filter((n: Node) => n.status === 'healthy').length;
  const totalCapacity = allNodes.reduce((sum: number, n: Node) => sum + n.capacity.max_streams, 0);
  const totalViewers = allNodes.reduce((sum: number, n: Node) => sum + n.capacity.max_viewers, 0);
  const activeBroadcasts = broadcasts?.data?.broadcasts?.filter((b: Broadcast) => b.status === 'active').length || 0;
  const currentViewers = allNodes.reduce((sum: number, n: Node) => sum + n.viewers, 0);

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="page-title">{t('cluster.title')}</h1>
          <p className="page-subtitle">{t('cluster.manageCluster')}</p>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2 px-3 py-1 rounded-lg bg-secondary-100 dark:bg-secondary-800">
            <span className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-red-500'}`} />
            <span className="text-sm text-secondary-700 dark:text-secondary-300">
              {connected ? t('dashboard.realtimeConnected') : t('dashboard.realtimeDisconnected')}
            </span>
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <ClusterStatCard
          title={t('cluster.nodes')}
          value={allNodes.length}
          icon="server"
          subtitle={`${healthyCount} ${t('cluster.healthy')}`}
        />
        <ClusterStatCard
          title={t('cluster.capacity')}
          value={totalCapacity}
          icon="database"
          subtitle={`${totalViewers.toLocaleString()} viewers`}
        />
        <ClusterStatCard
          title={t('streams.title')}
          value={activeBroadcasts}
          icon="broadcast"
          subtitle={`${activeBroadcasts} active`}
        />
        <ClusterStatCard
          title={t('dashboard.concurrentViewers')}
          value={currentViewers.toLocaleString()}
          icon="users"
        />
      </div>

      <Tabs defaultValue="nodes" onValueChange={console.log}>
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="nodes">{t('cluster.nodes')}</TabsTrigger>
          <TabsTrigger value="broadcasts">{t('cluster.broadcasts')}</TabsTrigger>
          <TabsTrigger value="federation">{t('cluster.federation')}</TabsTrigger>
          <TabsTrigger value="mesh">{t('cluster.mesh')}</TabsTrigger>
        </TabsList>

        <TabsContent value="nodes">
          <NodeTable nodes={allNodes} realtimeNodes={realtimeNodes} isLoading={nodesLoading} />
        </TabsContent>

        <TabsContent value="broadcasts">
          <BroadcastTable broadcasts={broadcasts?.data?.broadcasts || []} />
        </TabsContent>

        <TabsContent value="federation">
          <FederationView federation={federation?.data} />
        </TabsContent>

        <TabsContent value="mesh">
          <MeshView mesh={mesh?.data} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function ClusterStatCard({ title, value, icon, subtitle }: { title: string; value: number | string; icon: string; subtitle?: string }) {
  const icons = { server: Server, database: Database, broadcast: Broadcast, users: Users };
  const Icon = icons[icon as keyof typeof icons];

  return (
    <div className="card">
      <div className="card-body">
        <div className="flex items-start justify-between">
          <div>
            <p className="text-sm text-secondary-500 dark:text-secondary-400">{title}</p>
            <p className="text-3xl font-bold text-secondary-900 dark:text-white">{value}</p>
            {subtitle && <p className="text-sm text-secondary-500 dark:text-secondary-400 mt-1">{subtitle}</p>}
          </div>
          <div className="w-12 h-12 rounded-xl bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
            <Icon className="w-6 h-6 text-primary-600 dark:text-primary-400" />
          </div>
        </div>
      </div>
    </div>
  );
}

function NodeTable({ nodes, realtimeNodes, isLoading }: { nodes: Node[]; realtimeNodes: Node[]; isLoading: boolean }) {
  const { t } = useTranslation();

  if (isLoading) {
    return <div className="flex items-center justify-center h-64"><Server className="w-8 h-8 animate-spin text-primary-600" /></div>;
  }

  const realtimeMap = new Map(realtimeNodes.map(n => [n.id, n]));

  return (
    <div className="table-container mt-4">
      <table className="table">
        <thead>
          <tr>
            <th>{t('cluster.nodeId')}</th>
            <th>{t('cluster.region')}</th>
            <th>{t('cluster.status')}</th>
            <th>{t('cluster.cpu')}</th>
            <th>{t('cluster.memory')}</th>
            <th>{t('cluster.disk')}</th>
            <th>{t('cluster.bandwidth')}</th>
            <th>{t('cluster.gpu')}</th>
            <th>{t('streams.streams')}</th>
            <th>{t('streams.viewers')}</th>
            <th>{t('common.lastSeen')}</th>
          </tr>
        </thead>
        <tbody>
          {nodes.length === 0 ? (
            <tr>
              <td colSpan={11} className="text-center py-8 text-secondary-500">{t('cluster.noNodes')}</td>
            </tr>
          ) : (
            nodes.map((node) => {
              const realtime = realtimeMap.get(node.id);
              const status = realtime?.status || node.status;
              const cpu = realtime?.cpu_percent ?? node.cpu_usage;
              const memory = realtime?.memory_percent ?? node.memory_usage;
              const disk = realtime?.disk_percent ?? node.disk_usage;
              const lastHeartbeat = realtime?.last_heartbeat || node.last_heartbeat;

              return (
                <tr key={node.id}>
                  <td className="font-mono text-sm">{node.id.slice(0, 12)}...</td>
                  <td>{node.region}</td>
                  <td>
                    <span className={clsx(
                      'badge',
                      status === 'healthy' && 'badge-success',
                      status === 'degraded' && 'badge-warning',
                      status === 'offline' && 'badge-danger'
                    )}>
                      {t(`cluster.${status}`)}
                    </span>
                  </td>
                  <td><UsageBar value={cpu} /></td>
                  <td><UsageBar value={memory} /></td>
                  <td><UsageBar value={disk} /></td>
                  <td><UsageBar value={node.bandwidth_usage} /></td>
                  <td>{node.gpu_usage !== undefined ? <UsageBar value={node.gpu_usage} /> : '—'}</td>
                  <td>{node.streams} / {node.capacity.max_streams}</td>
                  <td>{node.viewers.toLocaleString()} / {node.capacity.max_viewers.toLocaleString()}</td>
                  <td className="text-secondary-500 dark:text-secondary-400">
                    {formatDistanceToNow(new Date(lastHeartbeat), { addSuffix: true })}
                  </td>
                </tr>
              );
            })
          )}
        </tbody>
      </table>
    </div>
  );
}

function UsageBar({ value }: { value: number }) {
  const color = value > 80 ? 'bg-danger-500' : value > 60 ? 'bg-warning-500' : 'bg-success-500';
  return (
    <div className="flex items-center gap-2">
      <div className="flex-1 h-2 bg-secondary-200 dark:bg-secondary-700 rounded-full overflow-hidden">
        <div className={`${color} h-full rounded-full transition-all`} style={{ width: `${Math.min(100, value)}%` }} />
      </div>
      <span className="text-xs font-mono w-10 text-right">{value.toFixed(0)}%</span>
    </div>
  );
}

function BroadcastTable({ broadcasts }: { broadcasts: any[] }) {
  const { t } = useTranslation();

  return (
    <div className="table-container mt-4">
      <table className="table">
        <thead>
          <tr>
            <th>{t('streams.streamId')}</th>
            <th>{t('cluster.ownerNode')}</th>
            <th>{t('cluster.status')}</th>
            <th>{t('streams.viewers')}</th>
            <th>{t('streams.bitrate')}</th>
          </tr>
        </thead>
        <tbody>
          {broadcasts.map((b) => (
            <tr key={b.id}>
              <td className="font-mono text-sm">{b.stream_id}</td>
              <td>{b.owner_node_id}</td>
              <td>
                <span className={clsx(
                  'badge',
                  b.status === 'active' && 'badge-success',
                  b.status === 'transferring' && 'badge-info',
                  b.status === 'failed' && 'badge-danger'
                )}>
                  {b.status}
                </span>
              </td>
              <td>{b.viewers.toLocaleString()}</td>
              <td>{formatBitrate(b.bitrate)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function FederationView({ federation }: { federation: any }) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.federationLinks')}</h3>
            <p className="text-3xl font-bold">{federation?.links?.length || 0}</p>
          </div>
        </div>
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.federationStreams')}</h3>
            <p className="text-3xl font-bold">{federation?.streams?.length || 0}</p>
          </div>
        </div>
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.federationHealth')}</h3>
            <span className="badge badge-success">{t('cluster.healthy')}</span>
          </div>
        </div>
      </div>

      <div className="card">
        <div className="card-header">
          <h3 className="font-medium">{t('cluster.federationLinks')}</h3>
        </div>
        <div className="card-body">
          <div className="table-container">
            <table className="table">
              <thead>
                <tr>
                  <th>Cluster ID</th>
                  <th>Region</th>
                  <th>Status</th>
                  <th>Last Connected</th>
                  <th>Streams</th>
                  <th>Errors</th>
                </tr>
              </thead>
              <tbody>
                {federation?.links?.map((link: any) => (
                  <tr key={link.cluster_id}>
                    <td>{link.cluster_id}</td>
                    <td>{link.region}</td>
                    <td><span className={`badge ${link.status === 'connected' ? 'badge-success' : 'badge-danger'}`}>{link.status}</span></td>
                    <td>{link.last_connected ? formatDistanceToNow(new Date(link.last_connected), { addSuffix: true }) : 'Never'}</td>
                    <td>{link.streams || 0}</td>
                    <td>{link.errors || 0}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}

function MeshView({ mesh }: { mesh: any }) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.meshPeers')}</h3>
            <p className="text-3xl font-bold">{mesh?.peers?.length || 0}</p>
          </div>
        </div>
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.meshOffloaded')}</h3>
            <p className="text-3xl font-bold">{mesh?.offloaded_percentage || 0}%</p>
          </div>
        </div>
        <div className="card">
          <div className="card-body">
            <h3 className="font-medium mb-2">{t('cluster.meshLatency')}</h3>
            <p className="text-3xl font-bold">{mesh?.avg_latency_ms || 0}ms</p>
          </div>
        </div>
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