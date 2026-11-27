import { useQuery } from '@tanstack/react-query';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { api } from '@/services/api';
import { clsx } from 'clsx';

interface DataPoint {
  timestamp: string;
  live_streams: number;
  viewers: number;
  bandwidth: number;
}

export function StreamsChart() {
  const { data } = useQuery({
    queryKey: ['dashboard', 'streamsChart'],
    queryFn: async () => {
      const response = await api.get('/api/v1/metrics/query', {
        params: {
          query: 'sum by (stream_id) (live_streams)',
          from: '-1h',
          step: '30s',
        },
      });
      return response.data;
    },
    refetchInterval: 30000,
  });

  // Generate mock data for demo
  const chartData = data?.result?.[0]?.values?.map((v: [number, string]) => ({
    timestamp: new Date(v[0] * 1000).toLocaleTimeString(),
    value: parseFloat(v[1]),
  })) || Array.from({ length: 60 }, (_, i) => ({
    timestamp: new Date(Date.now() - (59 - i) * 60000).toLocaleTimeString(),
    live_streams: Math.floor(Math.random() * 50) + 100,
    viewers: Math.floor(Math.random() * 10000) + 5000,
    bandwidth: Math.floor(Math.random() * 1000000000) + 500000000,
  }));

  return (
    <div className="h-64">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={chartData} margin={{ top: 5, right: 30, left: 20, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" className="stroke-secondary-200 dark:stroke-secondary-700" />
          <XAxis
            dataKey="timestamp"
            className="text-xs fill-secondary-500 dark:fill-secondary-400"
            tick={{ fill: 'currentColor' }}
            tickLine={false}
            axisLine={false}
            interval="preserveStartEnd"
          />
          <YAxis
            className="text-xs fill-secondary-500 dark:fill-secondary-400"
            tick={{ fill: 'currentColor' }}
            tickLine={false}
            axisLine={false}
            tickFormatter={(value) => value >= 1000 ? `${(value / 1000).toFixed(1)}k` : value}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: '#1e293b',
              border: '1px solid #334155',
              borderRadius: '8px',
              color: '#f8fafc',
            }}
            labelStyle={{ color: '#94a3b8' }}
            formatter={(value: number) => [value.toLocaleString(), 'Live Streams']}
          />
          <Legend
            wrapperStyle={{ paddingTop: '20px' }}
            iconType="circle"
            layout="horizontal"
            align="center"
          />
          <Line
            type="monotone"
            dataKey="live_streams"
            name="Live Streams"
            stroke="#0ea5e9"
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 6 }}
          />
          <Line
            type="monotone"
            dataKey="viewers"
            name="Viewers"
            stroke="#22c55e"
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 6 }}
            yAxisId="right"
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}