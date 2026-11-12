import { request, setCsrfToken } from "./client";
import type { components } from "./schema";

export type Session = components["schemas"]["SessionDto"];
export type Access = components["schemas"]["AccessDto"];

export const SESSION_KEY = ["auth", "session"] as const;
export const ACCESS_KEY = ["settings", "access"] as const;

/** Đã đặt mật khẩu mà chưa đăng nhập thì mọi trang trừ `/login` phải chuyển đi (ADR 0003). */
export function needsLogin(session: Session | undefined): boolean {
  return session !== undefined && session.password_set && !session.logged_in;
}

/**
 * Đường dẫn quay về sau khi đăng nhập. Chỉ nhận đường dẫn trong chính MineHost, để link
 * `/login?next=//trang-la` không đưa người dùng ra ngoài.
 */
export function safeNext(next: string | null): string {
  if (!next || !next.startsWith("/") || next.startsWith("//") || next.startsWith("/\\")) return "/";
  if (next === "/login" || next.startsWith("/login?")) return "/";
  return next;
}

export const authApi = {
  session: async () => {
    const session = await request<Session>("GET", "/auth/session");
    setCsrfToken(session.csrf_token ?? null);
    return session;
  },
  login: async (password: string) => {
    const reply = await request<components["schemas"]["CsrfDto"]>("POST", "/auth/login", { password });
    setCsrfToken(reply.csrf_token);
  },
  logout: async () => {
    await request<void>("POST", "/auth/logout");
    setCsrfToken(null);
  },
  /** Đặt lần đầu thì không có `current`. Thành công thì mọi phiên bị hủy, kể cả phiên này. */
  setPassword: async (current: string | undefined, next: string) => {
    await request<void>("PUT", "/auth/password", current === undefined ? { new: next } : { current, new: next });
    setCsrfToken(null);
  },
  /** Xóa mật khẩu thì LAN tắt theo. */
  clearPassword: async (current: string) => {
    await request<void>("DELETE", "/auth/password", { current });
    setCsrfToken(null);
  },
  access: () => request<Access>("GET", "/settings/access"),
  setAccess: (lan: boolean) => request<Access>("PUT", "/settings/access", { lan }),
};
