import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../../api/client";
import type { ServerContent, UpdateReport } from "../../../api/content";
import { fileNameOf, updateFor, updateItems } from "./content";
import { ContentPage } from "./ContentPage";

const server: Server = {
  id: "A",
  name: "Fabric",
  state: "stopped",
  loader: "fabric",
  content_kind: "mods",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const content: ServerContent = {
  kind: "mods",
  pending: [],
  files: [
    {
      file_name: "lithium-0.14.2.jar",
      name: "Lithium",
      version: "0.14.2",
      enabled: true,
      size: 1,
      source: "modrinth",
      manual: false,
      client_only: false,
      project_id: "gvQqBUqZ",
      version_id: "v1",
      from_modpack: false,
      icon_url: null,
      needs_restart: false,
    },
  ],
};

const report: UpdateReport = {
  updates: [
    {
      file: "mods/lithium-0.14.2.jar",
      source: "modrinth",
      project_id: "gvQqBUqZ",
      current_version_id: "v1",
      latest_version_id: "v2",
      latest_version_name: "0.15.0",
      manual: false,
    },
  ],
  checked: 1,
  unknown: 0,
  from_modpack: 0,
};

function job(id: number, kind: string, extra: Partial<Job> = {}): Job {
  return { id, kind, state: "succeeded", server_id: "A", started_at: "2026-11-15T00:00:00Z", ...extra } as Job;
}

function renderPage() {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={["/servers/A/content"]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="content" element={<ContentPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("cập nhật mod", () => {
  it("kiểm tra, hiện nhãn có bản mới rồi gửi đúng bản để cập nhật", async () => {
    let applied: unknown = null;
    const updateResult = job(8, "update_content");
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string, init?: RequestInit) => {
        if (url === "/api/v1/servers/A/content") return Response.json(content);
        if (url === "/api/v1/servers/A/updates/check") return Response.json({ job_id: 7 }, { status: 202 });
        if (url === "/api/v1/jobs/7") return Response.json(job(7, "check_updates", { result: report as never }));
        if (url === "/api/v1/servers/A/updates" && init?.method === "POST") {
          applied = JSON.parse(String(init.body));
          return Response.json({ job_id: 8 }, { status: 202 });
        }
        if (url === "/api/v1/jobs/8") return Response.json(updateResult);
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("Lithium");

    await user.click(screen.getByRole("button", { name: "Kiểm tra cập nhật" }));
    expect(await screen.findByText("Có bản mới")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cập nhật 1 mục" }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("lithium-0.14.2.jar → 0.15.0")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Cập nhật" }));

    expect(await screen.findByText(/Đã cập nhật 1 mục/)).toBeTruthy();
    expect(applied).toEqual({
      items: [{ file: "mods/lithium-0.14.2.jar", source: "modrinth", project_id: "gvQqBUqZ", version_id: "v2" }],
    });
    await waitFor(() => expect(screen.queryByText("Có bản mới")).toBeNull());
  });

  it("lỗi sau khi sao lưu thì chỉ tới bản sao lưu", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url === "/api/v1/servers/A/content") return Response.json(content);
        if (url === "/api/v1/servers/A/updates/check") return Response.json({ job_id: 7 }, { status: 202 });
        if (url === "/api/v1/jobs/7") return Response.json(job(7, "check_updates", { result: report as never }));
        if (url === "/api/v1/servers/A/updates") return Response.json({ job_id: 8 }, { status: 202 });
        if (url === "/api/v1/jobs/8") {
          const error = { code: "DOWNLOAD_FAILED", params: { backup_id: "01ABC" } };
          return Response.json(job(8, "update_content", { state: "failed", error: error as never }));
        }
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    const user = userEvent.setup();
    renderPage();
    await screen.findByText("Lithium");
    await user.click(screen.getByRole("button", { name: "Kiểm tra cập nhật" }));
    await user.click(await screen.findByRole("button", { name: "Cập nhật 1 mục" }));
    await user.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Cập nhật" }));
    expect(await screen.findByText(/Bản sao lưu "Trước cập nhật"/)).toBeTruthy();
  });
});

describe("hàm trợ giúp cập nhật", () => {
  it("ghép dòng bảng với bản mới theo tên file", () => {
    expect(fileNameOf("mods/a.jar")).toBe("a.jar");
    expect(updateFor(report, { file_name: "lithium-0.14.2.jar" })?.latest_version_id).toBe("v2");
    expect(updateFor(null, { file_name: "lithium-0.14.2.jar" })).toBeUndefined();
    expect(updateItems(report.updates)).toEqual([
      { file: "mods/lithium-0.14.2.jar", source: "modrinth", project_id: "gvQqBUqZ", version_id: "v2" },
    ]);
  });
});
