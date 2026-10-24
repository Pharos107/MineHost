import { afterEach, describe, expect, it, vi } from "vitest";

import { LiveSocket, type LiveMessage, type SocketLike } from "./ws";

class FakeSocket implements SocketLike {
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  sent: unknown[] = [];
  closed = false;
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {
    this.closed = true;
  }
  push(message: object) {
    this.onmessage?.({ data: JSON.stringify(message) });
  }
}

function setup() {
  const sockets: FakeSocket[] = [];
  const live = new LiveSocket(() => {
    const socket = new FakeSocket();
    sockets.push(socket);
    return socket;
  });
  const last = () => sockets[sockets.length - 1] as FakeSocket;
  return { live, sockets, last };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("LiveSocket", () => {
  it("chỉ mở một kết nối và đăng ký mọi topic khi mở xong", () => {
    const { live, sockets, last } = setup();
    live.subscribe("servers", () => {});
    live.subscribe("job.1", () => {});
    expect(sockets).toHaveLength(1);
    last().onopen?.();
    expect(last().sent).toEqual([{ subscribe: ["servers", "job.1"], since: {} }]);

    live.subscribe("job.2", () => {});
    expect(last().sent[1]).toEqual({ subscribe: ["job.2"], since: {} });
  });

  it("chuyển tin tới đúng topic, kể cả lỗi backend gửi về", () => {
    const { live, last } = setup();
    const got: LiveMessage[] = [];
    live.subscribe("job.9", (m) => got.push(m));
    last().onopen?.();
    last().push({ topic: "servers", seq: 1, data: {} });
    last().push({ topic: "job.9", seq: 1, data: { id: 9 } });
    last().push({ topic: "error", seq: 0, data: { code: "NOT_FOUND", params: { topic: "job.9" } } });
    expect(got.map((m) => m.error?.code ?? m.seq)).toEqual([1, "NOT_FOUND"]);
  });

  it("mất kết nối thì nối lại và gửi `since` theo seq cuối", () => {
    vi.useFakeTimers();
    const { live, sockets, last } = setup();
    live.subscribe("server.A.console", () => {});
    last().onopen?.();
    last().push({ topic: "server.A.console", seq: 120, data: {} });
    last().onclose?.();
    vi.advanceTimersByTime(500);
    expect(sockets).toHaveLength(2);
    last().onopen?.();
    expect(last().sent).toEqual([{ subscribe: ["server.A.console"], since: { "server.A.console": 120 } }]);
  });

  it("đăng ký lại topic đã bỏ thì mở kết nối mới để nhận lại ảnh chụp", () => {
    const { live, sockets, last } = setup();
    live.subscribe("servers", () => {});
    const stop = live.subscribe("job.1", () => {});
    last().onopen?.();
    stop();
    live.subscribe("job.1", () => {});
    expect(sockets).toHaveLength(2);
    expect(sockets[0]?.closed).toBe(true);
  });

  it("bỏ đăng ký cuối cùng thì đóng kết nối, không nối lại", () => {
    vi.useFakeTimers();
    const { live, sockets, last } = setup();
    const stop = live.subscribe("servers", () => {});
    last().onopen?.();
    stop();
    expect(last().closed).toBe(true);
    last().onclose?.();
    vi.advanceTimersByTime(20_000);
    expect(sockets).toHaveLength(1);
  });
});
