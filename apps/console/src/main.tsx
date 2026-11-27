import React, { useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { Toaster } from 'react-hot-toast';
import { I18nextProvider } from 'react-i18next';
import i18n from './i18n';
import App from './App';
import './styles/index.css';
import { createScreenReaderAnnouncer, removeScreenReaderAnnouncer } from './utils/accessibility';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

function AccessibilityProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    createScreenReaderAnnouncer();
    return () => removeScreenReaderAnnouncer();
  }, []);

  return <>{children}</>;
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <I18nextProvider i18n={i18n}>
        <BrowserRouter>
          <AccessibilityProvider>
            <App />
            <Toaster
              position="top-right"
              toastOptions={{
                duration: 4000,
                style: { background: '#1e293b', color: '#f8fafc' },
                success: { iconTheme: { primary: '#22c55e', secondary: '#f8fafc' } },
                error: { iconTheme: { primary: '#ef4444', secondary: '#f8fafc' } },
              }}
            />
            <ReactQueryDevtools initialIsOpen={false} />
          </AccessibilityProvider>
        </BrowserRouter>
      </I18nextProvider>
    </QueryClientProvider>
  </React.StrictMode>
);