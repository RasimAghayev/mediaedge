import { useQuery } from '@tanstack/react-query';
import { api } from '@/services/api';

interface ErrorData {
  protocol: string;
  error_type: string;
  count: number;
  rate: number;
}

export function ErrorHeatmap() {
  const { data } = useQuery({
    queryKey: ['dashboard', 'errorHeatmap'],
    queryFn: async () => {
      const response = await api.get('/api/v1/metrics/query', {
        params: {
          query: 'sum by (protocol, error_type) (rate(errors_total[5m]))',
          from: '-1h',
        },
      });
      return response.data;
    },
    refetchInterval: 60000,
  });

  // Mock data for demo
  const protocols = ['HLS', 'DASH', 'WebSocket', 'WHEP', 'MoQ'];
  const errorTypes = ['Connection Timeout', 'Segment Missing', 'Codec Error', 'Auth Failed', 'Bandwidth Exceeded'];

  const heatmapData = protocols.map((protocol) => ({
    protocol,
    errors: errorTypes.map((type) => ({
      type,
      count: Math.floor(Math.random() * 100),
      rate: Math.random() * 0.1,
    })),
  }));

  const maxCount = Math.max(...heatmapData.flatMap((p) => p.errors.map((e) => e.count)));

  return (
    <div className="h-64 overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr>
            <th className="px-4 py-2 text-left font-medium text-secondary-500 dark:text-secondary-400 w-24">
              Protocol
            </th>
            {errorTypes.map((type) => (
              <th key={type} className="px-2 py-2 text-center font-medium text-secondary-500 dark:text-secondary-400">
                <div className="whitespace-nowrap">{type}</div>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {heatmapData.map((row) => (
            <tr key={row.protocol}>
              <td className="px-4 py-2 font-medium text-secondary-700 dark:text-secondary-300">{row.protocol}</td>
              {row.errors.map((error) => (
                <td key={error.type} className="px-2 py-2 text-center">
                  <div
                    className="w-full h-8 rounded flex items-center justify-center text-xs font-medium transition-colors"
                    style={{
                      backgroundColor: `rgba(239, 68, 68, ${Math.max(0.1, error.count / maxCount)})`,
                      color: error.count > maxCount * 0.5 ? '#fef2f2' : '#991b1b',
                    }}
                  >
                    {error.count > 0 ? error.count : '—'}
                  </div>
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}