import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { DownloadCacheCard, ResourceLimitsCard } from "./StorageCards";

function renderCards() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <DownloadCacheCard />
      <ResourceLimitsCard />
    </QueryClientProvider>,
  );
}

function stub(enforced: boolean, busy = false) {
  let size = 3 * 1024 * 1024;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      if (url === "/api/v1/settings/download-cache" && init?.method === "DELETE") {
        if (busy) return Response.json({ code: "SERVER_BUSY", params: {} }, { status: 409 });
        const freed = size;
        size = 0;
        return Response.json({ freed_bytes: freed });
      }
      if (url === "/api/v1/settings/download-cache") return Response.json({ size_bytes: size });
      if (url === "/api/v1/system") return Response.json({ resource_limits: { enforced } });
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    }),
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("bộ nhớ đệm và giới hạn tài nguyên", () => {
  it("hiện dung lượng, xóa có hỏi lại", async () => {
    stub(true);
    const user = userEvent.setup();
    renderCards();
    expect(await screen.findByText(/3[,.]0 MB|3 MB/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Xóa bộ nhớ đệm tải về" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Xóa bộ nhớ đệm tải về" }));
    expect(await screen.findByText(/Đã xóa/)).toBeTruthy();
    expect(screen.getByText(/giới hạn được RAM và số process/)).toBeTruthy();
  });

  it("đang có Job tải thì báo lỗi trong hộp thoại", async () => {
    stub(false, true);
    const user = userEvent.setup();
    renderCards();
    await user.click(await screen.findByRole("button", { name: "Xóa bộ nhớ đệm tải về" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Xóa bộ nhớ đệm tải về" }));
    expect(await within(dialog).findByRole("alert")).toBeTruthy();
    expect(screen.getByText("Chưa giới hạn được RAM trên máy này")).toBeTruthy();
  });
});
