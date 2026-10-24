/** Tin từ `/api/v1/ws` (spec P1, mục 5.8). `error` có khi backend từ chối topic này. */
export interface LiveMessage {
  topic: string;
  seq: number;
  data: unknown;
  error?: { code: string; params: Record<string, unknown> };
}

type Handler = (message: LiveMessage) => void;

/** Phần của `WebSocket` mà client cần, để test thay được bằng bản giả. */
export interface SocketLike {
  onopen: (() => void) | null;
  onclose: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  send(data: string): void;
  close(): void;
}

const MAX_BACKOFF_MS = 10_000;

/**
 * Một kết nối WebSocket dùng chung cho cả app. Mất kết nối thì tự nối lại và gửi `since` theo
 * `seq` cuối của từng topic, để không lỡ dòng console nào.
 */
export class LiveSocket {
  private socket: SocketLike | null = null;
  private readonly handlers = new Map<string, Set<Handler>>();
  private readonly since = new Map<string, number>();
  /** Topic đã gửi đăng ký trên kết nối hiện tại. Backend chưa có lệnh hủy đăng ký. */
  private sent = new Set<string>();
  private isOpen = false;
  private retries = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private readonly open: () => SocketLike;

  constructor(open: () => SocketLike) {
    this.open = open;
  }

  subscribe(topic: string, handler: Handler): () => void {
    let set = this.handlers.get(topic);
    if (!set) {
      set = new Set();
      this.handlers.set(topic, set);
      this.request(topic);
    }
    set.add(handler);
    return () => this.unsubscribe(topic, handler);
  }

  private unsubscribe(topic: string, handler: Handler) {
    const set = this.handlers.get(topic);
    if (!set?.delete(handler) || set.size > 0) return;
    this.handlers.delete(topic);
    this.since.delete(topic);
    if (this.handlers.size === 0) this.disconnect();
  }

  private request(topic: string) {
    if (!this.socket) return this.connect();
    // Backend không gửi lại ảnh chụp cho topic đã đăng ký trên kết nối này: nối lại từ đầu.
    if (this.sent.has(topic)) return this.reconnectNow();
    // Kết nối đang mở dở thì `onopen` đăng ký mọi topic, kể cả topic này.
    if (this.isOpen) this.sendSubscribe([topic]);
  }

  private connect() {
    clearTimeout(this.timer);
    const socket = this.open();
    this.socket = socket;
    this.sent = new Set();
    socket.onopen = () => {
      this.isOpen = true;
      this.retries = 0;
      this.sendSubscribe([...this.handlers.keys()]);
    };
    socket.onmessage = (event) => this.receive(event.data);
    socket.onclose = () => {
      if (this.socket !== socket) return;
      this.socket = null;
      this.isOpen = false;
      if (this.handlers.size > 0) this.scheduleReconnect();
    };
  }

  private sendSubscribe(topics: string[]) {
    if (!this.socket || topics.length === 0) return;
    const since = Object.fromEntries(topics.flatMap((t) => (this.since.has(t) ? [[t, this.since.get(t)]] : [])));
    topics.forEach((topic) => this.sent.add(topic));
    this.socket.send(JSON.stringify({ subscribe: topics, since }));
  }

  private receive(raw: unknown) {
    let message: LiveMessage;
    try {
      message = JSON.parse(String(raw)) as LiveMessage;
    } catch {
      return;
    }
    if (message.topic === "error") {
      const error = message.data as NonNullable<LiveMessage["error"]>;
      const topic = String(error.params?.topic ?? "");
      return this.dispatch({ topic, seq: 0, data: null, error });
    }
    this.since.set(message.topic, message.seq);
    this.dispatch(message);
  }

  private dispatch(message: LiveMessage) {
    this.handlers.get(message.topic)?.forEach((handler) => handler(message));
  }

  private scheduleReconnect() {
    const delay = Math.min(500 * 2 ** this.retries, MAX_BACKOFF_MS);
    this.retries += 1;
    this.timer = setTimeout(() => this.connect(), delay);
  }

  private reconnectNow() {
    const old = this.socket;
    this.socket = null;
    this.isOpen = false;
    old?.close();
    this.connect();
  }

  private disconnect() {
    clearTimeout(this.timer);
    const old = this.socket;
    this.socket = null;
    this.isOpen = false;
    this.retries = 0;
    old?.close();
  }
}

function openBrowserSocket(): SocketLike {
  const scheme = window.location.protocol === "https:" ? "wss" : "ws";
  const socket = new WebSocket(`${scheme}://${window.location.host}/api/v1/ws`);
  const wrapper: SocketLike = {
    onopen: null,
    onclose: null,
    onmessage: null,
    send: (data) => socket.send(data),
    close: () => socket.close(),
  };
  socket.onopen = () => wrapper.onopen?.();
  socket.onclose = () => wrapper.onclose?.();
  socket.onmessage = (event) => wrapper.onmessage?.(event);
  return wrapper;
}

export const live = new LiveSocket(openBrowserSocket);
