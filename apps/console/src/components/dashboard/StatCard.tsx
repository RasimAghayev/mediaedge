import { clsx } from 'clsx';
import { Broadcast, Users, Activity, AlertTriangle, TrendingUp, TrendingDown } from 'lucide-react';

interface StatCardProps {
  title: string;
  value: string;
  icon: 'broadcast' | 'users' | 'activity' | 'alertTriangle';
  trend?: string;
  trendUp?: boolean;
}

const icons = {
  broadcast: Broadcast,
  users: Users,
  activity: Activity,
  alertTriangle: AlertTriangle,
};

export function StatCard({ title, value, icon, trend, trendUp = true }: StatCardProps) {
  const Icon = icons[icon];

  return (
    <div className="card">
      <div className="card-body">
        <div className="flex items-start justify-between">
          <div>
            <p className="text-sm font-medium text-secondary-500 dark:text-secondary-400">{title}</p>
            <p className="text-3xl font-bold text-secondary-900 dark:text-white mt-1">{value}</p>
            {trend && (
              <div className={clsx('flex items-center gap-1 mt-2 text-sm', trendUp ? 'text-success-600' : 'text-danger-600')}>
                {trendUp ? <TrendingUp className="w-4 h-4" /> : <TrendingDown className="w-4 h-4" />}
                <span>{trend}</span>
                <span className="text-secondary-500 dark:text-secondary-400">vs last hour</span>
              </div>
            )}
          </div>
          <div className="w-12 h-12 rounded-xl bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center">
            <Icon className="w-6 h-6 text-primary-600 dark:text-primary-400" />
          </div>
        </div>
      </div>
    </div>
  );
}