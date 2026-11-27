import { EventSourcePolyfill } from 'event-source-polyfill';

export type SSEEventType =
  | 'stream.created'
  | 'stream.updated'
  | 'stream.deleted'
  | 'stream.started'
  | 'stream.stopped'
  | 'stream.error'
  | 'stream.metrics'
  | 'cluster.node.joined'
  | 'cluster.node.left'
  | 'cluster.node.health'
  | 'cluster.capacity'
  | 'tenant.created'
  | 'tenant.updated'
  | 'tenant.deleted'
  | 'tenant.quota.warning'
  | 'tenant.quota.exceeded'
  | 'monetization.fast.channel.created'
  | 'monetization.fast.channel.updated'
  | 'monetization.fast.channel.deleted'
  | 'monetization.tvod.transaction'
  | 'monetization.bundle.subscription'
  | 'monetization.entitlement.check'
  | 'monetization.billing.invoice'
  | 'alert.critical'
  | 'alert.warning'
  | 'alert.info'
  | 'metrics.threshold';

export interface SSEEvent<T = unknown> {
  type: SSEEventType;
  timestamp: string;
  tenant_id?: string;
  payload: T;
}

export interface StreamMetricsPayload {
  stream_id: string;
  viewers: number;
  bandwidth_bps: number;
  latency_ms: number;
  error_rate: number;
}

export interface NodeHealthPayload {
  node_id: string;
  status: 'healthy' | 'degraded' | 'unhealthy';
  cpu_percent: number;
  memory_percent: number;
  disk_percent: number;
  active_streams: number;
  last_heartbeat: string;
}

export interface AlertPayload {
  id: string;
  severity: 'critical' | 'warning' | 'info';
  title: string;
  message: string;
  source: string;
  metadata?: Record<string, unknown>;
}

type SSEHandler<T = unknown> = (event: SSEEvent<T>) => void;

interface Subscription {
  eventTypes: SSEEventType[];
  handler: SSEHandler;
}

class SSEService {
  private eventSource: EventSource | null = null;
  private subscriptions: Map<string, Subscription> = new Map();
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private baseReconnectDelay = 1000;
  private isConnecting = false;
  private url: string;
  private accessToken: string | null = null;

  constructor() {
    this.url = `${import.meta.env.VITE_API_URL || '/api'}/api/v1/events/stream`;
  }

  setAccessToken(token: string | null) {
    this.accessToken = token;
    if (this.eventSource) {
      this.disconnect();
      this.connect();
    }
  }

  connect() {
    if (this.isConnecting || this.eventSource?.readyState === EventSource.OPEN) {
      return;
    }

    this.isConnecting = true;

    const headers: Record<string, string> = {
      'Accept': 'text/event-stream',
    };

    if (this.accessToken) {
      headers['Authorization'] = `Bearer ${this.accessToken}`;
    }

    try {
      this.eventSource = new EventSourcePolyfill(this.url, {
        headers,
        withCredentials: true,
        heartbeatTimeout: 30000,
      });

      this.eventSource.onopen = () => {
        this.isConnecting = false;
        this.reconnectAttempts = 0;
        console.log('[SSE] Connected to real-time event stream');
      };

      this.eventSource.onerror = (error) => {
        this.isConnecting = false;
        console.error('[SSE] Connection error:', error);
        this.handleReconnect();
      };

      this.eventSource.addEventListener('message', (event) => {
        try {
          const sseEvent: SSEEvent = JSON.parse(event.data);
          this.dispatchEvent(sseEvent);
        } catch (err) {
          console.error('[SSE] Failed to parse event:', err);
        }
      });

      for (const eventType of this.getSubscribedEventTypes()) {
        this.eventSource.addEventListener(eventType, (event) => {
          try {
            const sseEvent: SSEEvent = JSON.parse(event.data);
            this.dispatchEvent(sseEvent);
          } catch (err) {
            console.error(`[SSE] Failed to parse ${eventType}:`, err);
          }
        });
      }
    } catch (err) {
      this.isConnecting = false;
      console.error('[SSE] Failed to create connection:', err);
      this.handleReconnect();
    }
  }

  private getSubscribedEventTypes(): SSEEventType[] {
    const types = new Set<SSEEventType>();
    for (const sub of this.subscriptions.values()) {
      for (const type of sub.eventTypes) {
        types.add(type);
      }
    }
    return Array.from(types);
  }

  private handleReconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error('[SSE] Max reconnect attempts reached');
      return;
    }

    const delay = this.baseReconnectDelay * Math.pow(2, this.reconnectAttempts);
    this.reconnectAttempts++;
    console.log(`[SSE] Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

    setTimeout(() => {
      this.connect();
    }, delay);
  }

  disconnect() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    this.isConnecting = false;
    this.reconnectAttempts = 0;
  }

  subscribe(id: string, eventTypes: SSEEventType[], handler: SSEHandler) {
    this.subscriptions.set(id, { eventTypes, handler });
    if (this.eventSource?.readyState === EventSource.OPEN) {
      for (const eventType of eventTypes) {
        this.eventSource.addEventListener(eventType, (event) => {
          try {
            const sseEvent: SSEEvent = JSON.parse(event.data);
            handler(sseEvent);
          } catch (err) {
            console.error(`[SSE] Failed to parse ${eventType}:`, err);
          }
        });
      }
    }
  }

  unsubscribe(id: string) {
    this.subscriptions.delete(id);
  }

  private dispatchEvent<T>(event: SSEEvent<T>) {
    for (const [, subscription] of this.subscriptions) {
      if (subscription.eventTypes.includes(event.type)) {
        try {
          subscription.handler(event);
        } catch (err) {
          console.error('[SSE] Handler error:', err);
        }
      }
    }
  }

  isConnected(): boolean {
    return this.eventSource?.readyState === EventSource.OPEN;
  }

  getReadyState(): number {
    return this.eventSource?.readyState ?? EventSource.CLOSED;
  }
}

export const sseService = new SSEService();

export function useSSE<T = unknown>(
  eventTypes: SSEEventType[],
  handler: SSEHandler<T>,
  deps: unknown[] = []
) {
  const { useEffect, useRef } = require('react');
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    const id = `sse-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    sseService.subscribe(id, eventTypes, handlerRef.current as SSEHandler);
    sseService.connect();

    return () => {
      sseService.unsubscribe(id);
    };
  }, deps);
}

export function useStreamMetrics(streamId: string) {
  const { useState, useEffect } = require('react');
  const [metrics, setMetrics] = useState<StreamMetricsPayload | null>(null);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const id = `stream-metrics-${streamId}`;
    sseService.subscribe(id, ['stream.metrics'], (event) => {
      const payload = event.payload as StreamMetricsPayload;
      if (payload.stream_id === streamId) {
        setMetrics(payload);
      }
    });

    const connId = `conn-check-${Date.now()}`;
    const checkConnection = setInterval(() => {
      setConnected(sseService.isConnected());
    }, 5000);

    sseService.connect();

    return () => {
      sseService.unsubscribe(id);
      clearInterval(checkConnection);
    };
  }, [streamId]);

  return { metrics, connected };
}

export function useClusterHealth() {
  const { useState, useEffect } = require('react');
  const [nodes, setNodes] = useState<Map<string, NodeHealthPayload>>(new Map());
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const id = 'cluster-health';
    sseService.subscribe(id, ['cluster.node.health', 'cluster.node.joined', 'cluster.node.left'], (event) => {
      const payload = event.payload as NodeHealthPayload;
      setNodes((prev) => {
        const next = new Map(prev);
        if (event.type === 'cluster.node.left') {
          next.delete(payload.node_id);
        } else {
          next.set(payload.node_id, payload);
        }
        return next;
      });
    });

    const checkConnection = setInterval(() => {
      setConnected(sseService.isConnected());
    }, 5000);

    sseService.connect();

    return () => {
      sseService.unsubscribe(id);
      clearInterval(checkConnection);
    };
  }, []);

  return { nodes: Array.from(nodes.values()), connected };
}

export function useAlerts() {
  const { useState, useEffect } = require('react');
  const [alerts, setAlerts] = useState<AlertPayload[]>([]);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const id = 'alerts';
    sseService.subscribe(id, ['alert.critical', 'alert.warning', 'alert.info'], (event) => {
      const payload = event.payload as AlertPayload;
      setAlerts((prev) => [payload, ...prev.slice(0, 49)]);
    });

    const checkConnection = setInterval(() => {
      setConnected(sseService.isConnected());
    }, 5000);

    sseService.connect();

    return () => {
      sseService.unsubscribe(id);
      clearInterval(checkConnection);
    };
  }, []);

  return { alerts, connected };
}