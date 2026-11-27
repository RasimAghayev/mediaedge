import { useTranslation } from 'react-i18next';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, type CreateFastChannelRequest, type CreateTvodProductRequest, type CreateBundleRequest } from '@/services/api';
import { TV, ShoppingBag, Package, CreditCard, Plus, TrendingUp, DollarSign, Calendar, Edit, Trash2, Play, Pause } from 'lucide-react';
import { useState } from 'react';
import { clsx } from 'clsx';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/Tabs';

interface FastChannel {
  id: string;
  name: string;
  slug: string;
  status: 'draft' | 'active' | 'paused' | 'archived';
  schedule: any;
  ad_breaks: any;
  output_formats: string[];
  created_at: string;
}

interface TvodProduct {
  id: string;
  title: string;
  rental_price_cents: number | null;
  purchase_price_cents: number | null;
  status: 'draft' | 'active' | 'hidden' | 'archived';
  availability: any;
}

interface Bundle {
  id: string;
  name: string;
  bundle_type: string;
  pricing: any;
  status: 'draft' | 'active' | 'paused' | 'archived';
}

export function Monetization() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const { data: fastChannels } = useQuery({
    queryKey: ['monetization', 'fastChannels'],
    queryFn: () => api.get('/api/v1/monetization/fast-channels'),
  });

  const { data: tvodProducts } = useQuery({
    queryKey: ['monetization', 'tvodProducts'],
    queryFn: () => api.get('/api/v1/monetization/tvod/products'),
  });

  const { data: bundles } = useQuery({
    queryKey: ['monetization', 'bundles'],
    queryFn: () => api.get('/api/v1/monetization/bundles'),
  });

  const { data: entitlements } = useQuery({
    queryKey: ['monetization', 'entitlements'],
    queryFn: () => api.get('/api/v1/monetization/entitlements/check', { params: { tenant_id: '', user_id: '' } }),
  });

  const { data: billing } = useQuery({
    queryKey: ['monetization', 'billing'],
    queryFn: () => api.get('/api/v1/monetization/billing/invoices'),
  });

  return (
    <div className="space-y-6 animate-in">
      <div className="page-header">
        <div>
          <h1 className="page-title">{t('monetization.title')}</h1>
          <p className="page-subtitle">{t('monetization.manageMonetization')}</p>
        </div>
      </div>

      <Tabs defaultValue="fast" onValueChange={console.log}>
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="fast">{t('monetization.fastChannels')}</TabsTrigger>
          <TabsTrigger value="tvod">{t('monetization.tvod')}</TabsTrigger>
          <TabsTrigger value="bundles">{t('monetization.bundles')}</TabsTrigger>
          <TabsTrigger value="entitlements">{t('monetization.entitlements')}</TabsTrigger>
          <TabsTrigger value="billing">{t('monetization.billing')}</TabsTrigger>
        </TabsList>

        <TabsContent value="fast">
          <FastChannelsTab channels={fastChannels?.data?.channels || []} />
        </TabsContent>

        <TabsContent value="tvod">
          <TvodTab products={tvodProducts?.data?.products || []} />
        </TabsContent>

        <TabsContent value="bundles">
          <BundlesTab bundles={bundles?.data?.bundles || []} />
        </TabsContent>

        <TabsContent value="entitlements">
          <EntitlementsTab />
        </TabsContent>

        <TabsContent value="billing">
          <BillingTab invoices={billing?.data?.invoices || []} />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function FastChannelsTab({ channels }: { channels: FastChannel[] }) {
  const { t } = useTranslation();
  const [showCreate, setShowCreate] = useState(false);

  return (
    <div className="mt-4 space-y-4">
      <div className="flex justify-between items-center">
        <h3 className="font-medium">{t('monetization.fastChannels')}</h3>
        <button onClick={() => setShowCreate(true)} className="btn-primary"><Plus className="w-4 h-4" /> {t('common.createChannel')}</button>
      </div>
      <div className="card">
        <div className="table-container">
          <table className="table">
            <thead><tr><th>{t('common.name')}</th><th>{t('common.slug')}</th><th>{t('common.status')}</th><th>{t('monetization.adBreaks')}</th><th>{t('monetization.outputFormats')}</th><th>{t('common.created')}</th><th>Actions</th></tr></thead>
            <tbody>
              {channels.map((c) => (
                <tr key={c.id}>
                  <td className="font-medium">{c.name}</td>
                  <td className="font-mono text-sm">{c.slug}</td>
                  <td><span className={clsx('badge', c.status === 'active' && 'badge-success', c.status === 'paused' && 'badge-warning', c.status === 'draft' && 'badge-info')}>{c.status}</span></td>
                  <td>{c.ad_breaks?.enabled ? 'Enabled' : 'Disabled'}</td>
                  <td>{c.output_formats?.join(', ')}</td>
                  <td>{new Date(c.created_at).toLocaleDateString()}</td>
                  <td>
                    <div className="flex gap-1">
                      <button className="p-1.5 rounded text-secondary-400 hover:text-secondary-600"><Edit className="w-4 h-4" /></button>
                      <button className="p-1.5 rounded text-danger-600 hover:bg-danger-100"><Trash2 className="w-4 h-4" /></button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function TvodTab({ products }: { products: TvodProduct[] }) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 space-y-4">
      <div className="flex justify-between items-center">
        <h3 className="font-medium">{t('monetization.tvod')}</h3>
        <button className="btn-primary"><Plus className="w-4 h-4" /> {t('common.addProduct')}</button>
      </div>
      <div className="card">
        <div className="table-container">
          <table className="table">
            <thead><tr><th>{t('common.title')}</th><th>{t('monetization.rentalPrice')}</th><th>{t('monetization.purchasePrice')}</th><th>{t('common.status')}</th><th>{t('monetization.availability')}</th><th>Actions</th></tr></thead>
            <tbody>
              {products.map((p) => (
                <tr key={p.id}>
                  <td className="font-medium">{p.title}</td>
                  <td>{p.rental_price_cents ? `$${(p.rental_price_cents / 100).toFixed(2)}` : '—'}</td>
                  <td>{p.purchase_price_cents ? `$${(p.purchase_price_cents / 100).toFixed(2)}` : '—'}</td>
                  <td><span className={clsx('badge', p.status === 'active' && 'badge-success')}>{p.status}</span></td>
                  <td>{p.availability?.territories?.join(', ') || 'Global'}</td>
                  <td><button className="p-1.5 rounded text-secondary-400 hover:text-secondary-600"><Edit className="w-4 h-4" /></button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function BundlesTab({ bundles }: { bundles: Bundle[] }) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 space-y-4">
      <div className="flex justify-between items-center">
        <h3 className="font-medium">{t('monetization.bundles')}</h3>
        <button className="btn-primary"><Plus className="w-4 h-4" /> {t('common.createBundle')}</button>
      </div>
      <div className="card">
        <div className="table-container">
          <table className="table">
            <thead><tr><th>{t('common.name')}</th><th>{t('monetization.bundleType')}</th><th>{t('monetization.pricing')}</th><th>{t('common.status')}</th><th>Actions</th></tr></thead>
            <tbody>
              {bundles.map((b) => (
                <tr key={b.id}>
                  <td className="font-medium">{b.name}</td>
                  <td className="capitalize">{b.bundle_type.replace(/_/g, ' ')}</td>
                  <td>${(b.pricing?.base_price_cents || 0) / 100} / {b.pricing?.billing_cycle || 'month'}</td>
                  <td><span className={clsx('badge', b.status === 'active' && 'badge-success')}>{b.status}</span></td>
                  <td><button className="p-1.5 rounded text-secondary-400 hover:text-secondary-600"><Edit className="w-4 h-4" /></button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function EntitlementsTab() {
  const { t } = useTranslation();

  return (
    <div className="mt-4">
      <div className="card">
        <div className="card-header"><h3 className="font-medium">{t('monetization.entitlements')}</h3></div>
        <div className="card-body">
          <p className="text-secondary-500">{t('monetization.entitlementsDesc')}</p>
          <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="p-4 bg-secondary-50 dark:bg-secondary-800 rounded-lg">
              <h4 className="font-medium mb-2">{t('monetization.checkEntitlement')}</h4>
              <div className="space-y-2">
                <input type="text" className="input" placeholder="User ID" />
                <input type="text" className="input" placeholder="Content ID" />
                <button className="btn-primary w-full">{t('common.check')}</button>
              </div>
            </div>
            <div className="p-4 bg-secondary-50 dark:bg-secondary-800 rounded-lg">
              <h4 className="font-medium mb-2">{t('monetization.activeEntitlements')}</h4>
              <p className="text-3xl font-bold text-primary-600">1,234</p>
            </div>
            <div className="p-4 bg-secondary-50 dark:bg-secondary-800 rounded-lg">
              <h4 className="font-medium mb-2">{t('monetization.deniedEntitlements')}</h4>
              <p className="text-3xl font-bold text-danger-600">12</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function BillingTab({ invoices }: { invoices: any[] }) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 space-y-4">
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <BillingStatCard title={t('monetization.totalRevenue')} value="$125,430" icon="dollarSign" trend="+12%" />
        <BillingStatCard title={t('monetization.mrr')} value="$45,200" icon="trendingUp" trend="+8%" />
        <BillingStatCard title={t('monetization.activeSubscriptions')} value="1,234" icon="package" trend="+5%" />
        <BillingStatCard title={t('monetization.churnRate')} value="2.3%" icon="trendingUp" trend="-0.5%" trendUp={false} />
      </div>
      <div className="card">
        <div className="card-header flex justify-between">
          <h3 className="font-medium">{t('monetization.invoices')}</h3>
          <button className="btn-secondary btn-sm">{t('common.export')}</button>
        </div>
        <div className="table-container">
          <table className="table">
            <thead><tr><th>Invoice</th><th>Customer</th><th>Amount</th><th>Status</th><th>Date</th><th>Due Date</th></tr></thead>
            <tbody>
              {invoices.map((inv: any) => (
                <tr key={inv.id}>
                  <td className="font-mono">{inv.id.slice(0, 12)}...</td>
                  <td>{inv.customer_name}</td>
                  <td>${(inv.amount_cents / 100).toFixed(2)}</td>
                  <td><span className={clsx('badge', inv.status === 'paid' && 'badge-success', inv.status === 'open' && 'badge-info')}>{inv.status}</span></td>
                  <td>{new Date(inv.created_at).toLocaleDateString()}</td>
                  <td>{new Date(inv.due_date).toLocaleDateString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function BillingStatCard({ title, value, icon, trend, trendUp = true }: { title: string; value: string; icon: string; trend: string; trendUp?: boolean }) {
  const icons = { dollarSign: DollarSign, trendingUp: TrendingUp, package: Package };
  const Icon = icons[icon as keyof typeof icons];

  return (
    <div className="card">
      <div className="card-body">
        <div className="flex items-center justify-between">
          <div><p className="text-sm text-secondary-500">{title}</p><p className="text-2xl font-bold">{value}</p><p className={clsx('text-sm', trendUp ? 'text-success-600' : 'text-danger-600')}>{trend}</p></div>
          <div className="w-10 h-10 rounded-lg bg-primary-100 dark:bg-primary-900/30 flex items-center justify-center"><Icon className="w-5 h-5 text-primary-600" /></div>
        </div>
      </div>
    </div>
  );
}