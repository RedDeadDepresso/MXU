/**
 * WebSocket 服务
 *
 * 仅在浏览器（非 Tauri）环境中激活，建立与后端 /api/ws 的长连接，
 * 接收实时推送事件，通过订阅 API 将事件分发给各消费者。
 */

import { createLogger } from '@/utils/logger';

const log = createLogger('wsService');

// ============================================================================
// 事件类型（与 Rust WsEvent 对应）
// ============================================================================

export interface WsKkafioOutputPayload {
  stream: string;
  line: string;
}

export type WsMessage =
  | { type: 'kkafio-output'; payload: WsKkafioOutputPayload }
  | { type: 'config-changed'; payload: undefined };

// ============================================================================
// 订阅者类型
// ============================================================================

type KkafioOutputHandler = (stream: string, line: string) => void;
type ConfigChangedHandler = () => void;
type ConnectionStatusHandler = (connected: boolean) => void;

// ============================================================================
// 内部状态
// ============================================================================

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;
let stopped = false;
let serverPort: number | null = null;

const kkafioOutputHandlers = new Set<KkafioOutputHandler>();
const configChangedHandlers = new Set<ConfigChangedHandler>();
const connectionStatusHandlers = new Set<ConnectionStatusHandler>();

let currentlyConnected = false;
let hasEverConnected = false;
let hasUnexpectedDisconnect = false;

function notifyConnectionStatus(connected: boolean, force = false) {
  const changed = connected !== currentlyConnected;
  currentlyConnected = connected;
  if (connected) {
    hasEverConnected = true;
    hasUnexpectedDisconnect = false;
  }
  if (!changed && !force) return;
  connectionStatusHandlers.forEach((h) => h(connected));
}

// ============================================================================
// URL
// ============================================================================

export function setServerPort(port: number): void {
  serverPort = port;
  log.info('WebSocket 目标端口已设置:', port);
}

function getWsUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  if (serverPort) {
    return `${protocol}//${window.location.hostname}:${serverPort}/api/ws`;
  }
  return `${protocol}//${window.location.host}/api/ws`;
}

// ============================================================================
// 连接管理
// ============================================================================

function scheduleReconnect() {
  if (stopped || reconnectTimer) return;
  log.info(`WebSocket 将在 ${reconnectDelay}ms 后重连...`);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, reconnectDelay);
  reconnectDelay = Math.min(reconnectDelay * 2, 30000);
}

function onOpen() {
  log.info('WebSocket 已连接');
  reconnectDelay = 1000;
  if (hasEverConnected && !currentlyConnected) {
    log.info('后端恢复，刷新页面以重新同步状态');
    window.location.reload();
    return;
  }
  notifyConnectionStatus(true);
}

function onClose(event: CloseEvent) {
  ws = null;
  currentlyConnected = false;
  if (!stopped) {
    log.warn(`WebSocket 断开 (code=${event.code})，准备重连`);
    const shouldForceNotify = !hasUnexpectedDisconnect;
    hasUnexpectedDisconnect = true;
    notifyConnectionStatus(false, shouldForceNotify);
    scheduleReconnect();
  }
}

function onError() {
  log.debug('WebSocket 发生错误');
}

function onMessage(event: MessageEvent) {
  let msg: WsMessage;
  try {
    msg = JSON.parse(event.data as string) as WsMessage;
  } catch {
    log.warn('收到无法解析的 WS 消息:', event.data);
    return;
  }

  switch (msg.type) {
    case 'kkafio-output':
      kkafioOutputHandlers.forEach((h) => h(msg.payload.stream, msg.payload.line));
      break;
    case 'config-changed':
      configChangedHandlers.forEach((h) => h());
      break;
    default:
      log.debug('收到未知 WS 消息类型:', (msg as { type: string }).type);
  }
}

// ============================================================================
// 公开 API
// ============================================================================

export function connect(): void {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) return;
  stopped = false;
  const url = getWsUrl();
  log.info('连接 WebSocket:', url);
  ws = new WebSocket(url);
  ws.addEventListener('open', onOpen);
  ws.addEventListener('close', onClose);
  ws.addEventListener('error', onError);
  ws.addEventListener('message', onMessage);
}

export function disconnect(): void {
  stopped = true;
  currentlyConnected = false;
  hasUnexpectedDisconnect = false;
  if (reconnectTimer) { clearTimeout(reconnectTimer); reconnectTimer = null; }
  if (ws) { ws.close(); ws = null; }
}

export function isConnected(): boolean {
  return ws?.readyState === WebSocket.OPEN;
}

export function onKkafioOutput(handler: KkafioOutputHandler): () => void {
  kkafioOutputHandlers.add(handler);
  return () => kkafioOutputHandlers.delete(handler);
}

export function onConfigChanged(handler: ConfigChangedHandler): () => void {
  configChangedHandlers.add(handler);
  return () => configChangedHandlers.delete(handler);
}

export function onConnectionStatus(handler: ConnectionStatusHandler): () => void {
  connectionStatusHandlers.add(handler);
  if (currentlyConnected || hasUnexpectedDisconnect) handler(currentlyConnected);
  return () => connectionStatusHandlers.delete(handler);
}

export function hasConnectedBefore(): boolean {
  return hasEverConnected;
}

// Legacy stubs kept so other modules that still import these don't crash.
// They are no-ops since Maa is removed.
export function onMaaCallback(_handler: unknown): () => void { return () => {}; }
export function onAgentOutput(_handler: unknown): () => void { return () => {}; }
export function onStateChanged(_handler: unknown): () => void { return () => {}; }
