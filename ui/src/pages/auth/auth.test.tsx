import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { needsLogin, safeNext } from "../../api/auth";
import { api, setCsrfToken } from "../../api/client";
import { AuthGate } from "../../components/AuthGate";
import { LoginPage } from "./LoginPage";

type Handler = (url: string, init?: RequestInit) => Response | Promise<Response>;

function stubFetch(handler: Handler) {
  const fetchMock = vi.fn(handler);
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function Where() {
  const location = useLocation();
  return <p>ở {`${location.pathname}${location.search}`}</p>;
}

function renderAt(path: string) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={[path]}>
        <AuthGate>
          <Routes>
            <Route path="/login" element={<LoginPage />} />
            <Route path="*" element={<Where />} />
          </Routes>
        </AuthGate>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  setCsrfToken(null);
});

describe("needsLogin và safeNext", () => {
  it("chỉ cần đăng nhập khi đã có mật khẩu mà chưa có phiên", () => {
    expect(needsLogin(undefined)).toBe(false);
    expect(needsLogin({ password_set: false, logged_in: false })).toBe(false);
    expect(needsLogin({ password_set: true, logged_in: true, csrf_token: "x" })).toBe(false);
    expect(needsLogin({ password_set: true, logged_in: false })).toBe(true);
  });

  it("chỉ quay về đường dẫn trong MineHost", () => {
    expect(safeNext("/servers/01JB/files?path=plugins")).toBe("/servers/01JB/files?path=plugins");
    expect(safeNext(null)).toBe("/");
    expect(safeNext("https://evil.example")).toBe("/");
    expect(safeNext("//evil.example")).toBe("/");
    expect(safeNext("/\\evil.example")).toBe("/");
    expect(safeNext("/login?next=/jobs")).toBe("/");
  });
});

describe("AuthGate và trang Đăng nhập", () => {
  it("chưa đặt mật khẩu thì vào thẳng như P1", async () => {
    stubFetch(() => Response.json({ password_set: false, logged_in: false }));
    renderAt("/jobs");
    expect(await screen.findByText("ở /jobs")).toBeTruthy();
  });

  it("chưa đăng nhập thì sang /login, đăng nhập xong quay lại trang cũ", async () => {
    let loggedIn = false;
    const fetchMock = stubFetch((url) => {
      if (url === "/api/v1/auth/login") {
        loggedIn = true;
        return Response.json({ csrf_token: "tok" });
      }
      return Response.json(loggedIn ? { password_set: true, logged_in: true, csrf_token: "tok" } : { password_set: true, logged_in: false });
    });
    renderAt("/servers/01JB/files?path=plugins");

    fireEvent.change(await screen.findByLabelText("Mật khẩu"), { target: { value: "correct-horse" } });
    fireEvent.click(screen.getByRole("button", { name: "Đăng nhập" }));

    expect(await screen.findByText("ở /servers/01JB/files?path=plugins")).toBeTruthy();
    const login = fetchMock.mock.calls.find(([url]) => url === "/api/v1/auth/login");
    expect(JSON.parse(String(login?.[1]?.body))).toEqual({ password: "correct-horse" });
  });

  it("sai quá nhiều lần thì đếm lùi và khóa nút", async () => {
    stubFetch((url) =>
      url === "/api/v1/auth/login"
        ? Response.json({ code: "AUTH_RATE_LIMITED", params: { retry_after_s: 2 } }, { status: 429 })
        : Response.json({ password_set: true, logged_in: false }),
    );
    renderAt("/login");
    fireEvent.change(await screen.findByLabelText("Mật khẩu"), { target: { value: "wrong-pass" } });
    fireEvent.click(screen.getByRole("button", { name: "Đăng nhập" }));

    expect(await screen.findByText(/Thử lại sau 2 giây/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Đăng nhập" }).hasAttribute("disabled")).toBe(true);
    await act(() => new Promise((resolve) => setTimeout(resolve, 2100)));
    expect(screen.queryByText(/Thử lại sau/)).toBeNull();
    expect(screen.getByRole("button", { name: "Đăng nhập" }).hasAttribute("disabled")).toBe(false);
  });
});

describe("request", () => {
  it("gửi X-CSRF-Token khi có phiên", async () => {
    const fetchMock = stubFetch(() => new Response(null, { status: 202 }));
    setCsrfToken("tok");
    await api.shutdown();
    const headers = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
    expect(headers["X-CSRF-Token"]).toBe("tok");
    expect(headers["X-MineHost"]).toBe("1");
  });
});
