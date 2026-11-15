import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../api/client";
import { isDowngrade, versionRequest } from "./version";
import { VersionSection } from "./VersionSection";

const paper: Server = {
  id: "A",
  name: "Paper",
  state: "stopped",
  loader: "paper",
  minecraft: "1.21.4",
  build: 100,
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const catalog = [
  { id: "1.21.4", release_time: "2024-12-03T00:00:00Z" },
  { id: "1.21.1", release_time: "2024-08-08T00:00:00Z" },
];

function job(result: unknown, extra: Partial<Job> = {}): Job {
  return { id: 5, kind: "change_version", state: "succeeded", server_id: "A", started_at: "2026-11-15T00:00:00Z", result, ...extra } as Job;
}

function renderSection(server: Server) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <VersionSection server={server} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("hàm trợ giúp phiên bản", () => {
  it("chỉ coi là hạ phiên bản khi catalog biết cả hai bản", () => {
    expect(isDowngrade(catalog, "1.21.4", "1.21.1")).toBe(true);
    expect(isDowngrade(catalog, "1.21.1", "1.21.4")).toBe(false);
    expect(isDowngrade(catalog, "1.21.4", "1.21.4")).toBe(false);
    expect(isDowngrade(catalog, "26.1", "1.21.1")).toBe(false);
  });

  it("giữ bản Minecraft thì không gửi `minecraft` để lấy build mới nhất", () => {
    expect(versionRequest("1.21.4", "1.21.4", "")).toEqual({ confirm: true });
    expect(versionRequest("1.21.4", "1.21.1", "0.16.9")).toEqual({ confirm: true, minecraft: "1.21.1", loader_version: "0.16.9" });
  });
});

describe("mục Phiên bản", () => {
  it("hạ phiên bản phải tích ô hiểu rủi ro rồi mới gửi", async () => {
    let body: unknown = null;
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        if (url === "/api/v1/catalog/paper/versions") return Response.json(catalog);
        if (url === "/api/v1/servers/A/version") {
          body = JSON.parse(String(init?.body));
          return Response.json({ job_id: 5 }, { status: 202 });
        }
        if (url === "/api/v1/jobs/5") return Response.json(job({ changed: true, minecraft: "1.21.1", build: 50 }));
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    const user = userEvent.setup();
    renderSection(paper);
    expect(screen.getByText("100")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Đổi phiên bản" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByRole("option", { name: "1.21.1" });
    await user.selectOptions(within(dialog).getByRole("combobox", { name: "Minecraft" }), "1.21.1");
    expect(within(dialog).getByText(/World đã mở bằng bản mới/)).toBeTruthy();
    const confirm = within(dialog).getByRole("button", { name: "Tiếp tục" });
    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.click(within(dialog).getByRole("checkbox", { name: "Tôi hiểu world có thể không mở được" }));
    await user.click(confirm);

    expect(await screen.findByText(/Đã chuyển sang Minecraft 1.21.1/)).toBeTruthy();
    expect(body).toEqual({ confirm: true, minecraft: "1.21.1" });
  });

  it("nút cập nhật modpack chỉ có với modpack Modrinth", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        if (url === "/api/v1/content/modrinth/projects/P/versions") {
          return Response.json([
            { id: "v2", name: "Pack 2", version_number: "2.0", version_type: "release", game_versions: [], loaders: [], date_published: "2026-01-02T00:00:00Z", dependencies: [], files: [] },
            { id: "v1", name: "Pack 1", version_number: "1.0", version_type: "release", game_versions: [], loaders: [], date_published: "2026-01-01T00:00:00Z", dependencies: [], files: [] },
          ]);
        }
        if (url === "/api/v1/servers/A/updates" && init?.method === "POST") return Response.json({ job_id: 5 }, { status: 202 });
        if (url === "/api/v1/jobs/5") return Response.json(job({ removed: 3, kept: 1 }, { kind: "update_modpack" }));
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    const user = userEvent.setup();
    const modpack = { source: "modrinth", project_id: "P", version_id: "v1", name: "Gói", version_name: "1.0" };
    renderSection({ ...paper, loader: "fabric", loader_version: "0.16.9", build: null, modpack });
    expect(screen.getByText("Gói 1.0")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Cập nhật modpack" }));
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByRole("option", { name: "2.0" });
    expect(within(dialog).getByRole("option", { name: "1.0 (đang dùng)" })).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Tiếp tục" }));
    expect(await screen.findByText(/gỡ 3 file của bản cũ, giữ 1 mod/)).toBeTruthy();
  });

  it("modpack tải lên thì không có nút cập nhật", () => {
    renderSection({ ...paper, modpack: { source: "file", name: "Tự làm" } });
    expect(screen.queryByRole("button", { name: "Cập nhật modpack" })).toBeNull();
  });
});
