import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import { passwordErrors, passwordReady } from "./password";
import { SettingsPage } from "./SettingsPage";

interface Backend {
  passwordSet: boolean;
  lan: boolean;
  urls: string[];
}

function stubBackend(state: Backend) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    if (url === "/api/v1/auth/session") {
      return Response.json(state.passwordSet ? { password_set: true, logged_in: true, csrf_token: "tok" } : { password_set: false, logged_in: false });
    }
    if (url === "/api/v1/settings/access" && method === "PUT") {
      state.lan = (JSON.parse(String(init?.body)) as { lan: boolean }).lan;
    }
    if (url === "/api/v1/settings/access") {
      return Response.json({ lan: state.lan, lan_urls: state.lan ? state.urls : [] });
    }
    if (url === "/api/v1/auth/password" && method === "PUT") {
      state.passwordSet = true;
      return new Response(null, { status: 204 });
    }
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("password", () => {
  it("đếm ký tự Unicode và bắt nhập lại khớp", () => {
    expect(passwordErrors({ current: "", next: "ảảảảảảả", confirm: "" }).next).toBe("errors.PASSWORD_TOO_SHORT");
    expect(passwordErrors({ current: "", next: "ảảảảảảảả", confirm: "ảảảảảảảả" })).toEqual({});
    expect(passwordErrors({ current: "", next: "12345678", confirm: "1234567" }).confirm).toBe("appSettings.mismatch");
    expect(passwordReady({ current: "", next: "12345678", confirm: "12345678" }, false)).toBe(true);
    expect(passwordReady({ current: "", next: "12345678", confirm: "12345678" }, true)).toBe(false);
  });
});

describe("SettingsPage", () => {
  it("chưa có mật khẩu thì không bật được LAN; đặt mật khẩu gửi đúng body", async () => {
    const fetchMock = stubBackend({ passwordSet: false, lan: false, urls: [] });
    renderPage();

    const lan = await screen.findByRole("switch", { name: /Cho máy khác trong mạng LAN/ });
    expect(lan.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/Hãy đặt mật khẩu ở mục Bảo mật/)).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Mật khẩu mới"), { target: { value: "correct-horse" } });
    fireEvent.change(screen.getByLabelText("Nhập lại mật khẩu mới"), { target: { value: "correct-horse" } });
    fireEvent.click(screen.getByRole("button", { name: "Đặt mật khẩu" }));

    await waitFor(() => {
      const put = fetchMock.mock.calls.find(([url, init]) => url === "/api/v1/auth/password" && init?.method === "PUT");
      expect(JSON.parse(String(put?.[1]?.body))).toEqual({ new: "correct-horse" });
    });
  });

  it("bật LAN thì hiện địa chỉ để chép", async () => {
    stubBackend({ passwordSet: true, lan: false, urls: ["http://192.168.1.5:7878"] });
    renderPage();

    fireEvent.click(await screen.findByRole("switch", { name: /Cho máy khác trong mạng LAN/ }));
    expect(await screen.findByText("http://192.168.1.5:7878")).toBeTruthy();
    expect(screen.getByText(/dùng HTTP, không mã hóa/)).toBeTruthy();
  });
});
