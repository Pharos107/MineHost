import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job, Server } from "../../../api/client";
import type { ContentPlan, ContentProject, ContentVersion, SearchHit } from "../../../api/content";
import type { LiveMessage } from "../../../api/ws";
import { ContentAddPage } from "./ContentAddPage";
import { ContentPage } from "./ContentPage";

const handlers = new Map<string, (message: LiveMessage) => void>();

vi.mock("../../../api/ws", () => ({
  live: {
    subscribe: (topic: string, handler: (message: LiveMessage) => void) => {
      handlers.set(topic, handler);
      return () => handlers.delete(topic);
    },
  },
}));

const stoppedFabric: Server = {
  id: "A",
  name: "Fabric Test",
  state: "stopped",
  loader: "fabric",
  minecraft: "1.21.4",
  content_kind: "mods",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const stoppedPaper: Server = {
  ...stoppedFabric,
  id: "P",
  name: "Paper Test",
  loader: "paper",
  content_kind: "plugins",
};

const lithiumHit: SearchHit = {
  source: "modrinth",
  project_id: "lithium",
  slug: "lithium",
  kind: "mod",
  title: "Lithium",
  description: "Tối ưu hiệu năng server Fabric.",
  icon_url: null,
  downloads: 500_000,
  categories: ["fabric", "optimization"],
  game_versions: ["1.21.4"],
  server_side: "optional",
};

const fabricApiHit: SearchHit = {
  ...lithiumHit,
  project_id: "fabric-api",
  slug: "fabric-api",
  title: "Fabric API",
  description: "Thư viện nền cho mod Fabric.",
  downloads: 1_000_000,
};

const lithiumVersions: ContentVersion[] = [
  {
    id: "lithium-0.14.2",
    project_id: "lithium",
    name: "v0.14.2",
    version_number: "0.14.2",
    version_type: "release",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-12T00:00:00Z",
    file_name: "lithium-0.14.2.jar",
    file_size: 650_000,
    manual_download: false,
    download_page_url: null,
  },
  {
    id: "lithium-0.14.0",
    project_id: "lithium",
    name: "v0.14.0",
    version_number: "0.14.0",
    version_type: "beta",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-05T00:00:00Z",
    file_name: "lithium-0.14.0.jar",
    file_size: 620_000,
    manual_download: false,
    download_page_url: null,
  },
];

const samplePlan: ContentPlan = {
  kind: "mods",
  items: [
    {
      source: "modrinth",
      project_id: "lithium",
      project_name: "Lithium",
      version_id: "lithium-0.14.2",
      version_number: "0.14.2",
      file_name: "lithium-0.14.2.jar",
      file_size: 650_000,
      client_only: true,
      replaces_existing: true,
      is_dependency: false,
      manual_download: false,
      download_page_url: null,
    },
    {
      source: "modrinth",
      project_id: "fabric-api",
      project_name: "Fabric API",
      version_id: "fabric-api-0.110.0",
      version_number: "0.110.0",
      file_name: "fabric-api-0.110.0.jar",
      file_size: 2_100_000,
      client_only: false,
      replaces_existing: false,
      is_dependency: true,
      manual_download: false,
      download_page_url: null,
    },
  ],
};

const lithiumProject: ContentProject = {
  source: "modrinth",
  project_id: "lithium",
  slug: "lithium",
  kind: "mod",
  title: "Lithium",
  description: "Tối ưu hiệu năng server Fabric.",
  body: "## Tính năng\n\n**Nhanh** hơn nhiều.<script>alert(1)</script>",
  icon_url: null,
  downloads: 500_000,
  loaders: ["fabric"],
  game_versions: ["1.21.4"],
  links: { page: "https://modrinth.com/mod/lithium", source: "https://github.com/CaffeineMC/lithium", wiki: null, issues: null, discord: null },
};

const runningJob: Job = {
  id: 12,
  kind: "install_content",
  state: "running",
  server_id: "A",
  started_at: "2026-11-15T00:00:00Z",
};

type Handler = (url: string, init?: RequestInit) => Response | Promise<Response> | undefined;

function mockFetch(handler: Handler) {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const res = await handler(url, init);
    return res ?? Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

/** Catalog giả cho server A (Fabric): tìm, chi tiết, bản, kế hoạch, cài và danh sách mod. */
function catalogHandler(plan: ContentPlan): Handler {
  return (url, init) => {
    if (url === "/api/v1/settings/curseforge") return Response.json({ available: false, source: "none" });
    if (url.startsWith("/api/v1/content/search")) {
      return Response.json({ hits: [fabricApiHit, lithiumHit], page: 1, page_size: 20, total: 2 });
    }
    if (url === "/api/v1/content/modrinth/projects/lithium") return Response.json(lithiumProject);
    if (url === "/api/v1/content/modrinth/projects/lithium/versions?server_id=A") return Response.json(lithiumVersions);
    if (url === "/api/v1/servers/A/content/plan" && init?.method === "POST") return Response.json(plan);
    if (url === "/api/v1/servers/A/content" && init?.method === "POST") return Response.json({ job_id: 12 }, { status: 202 });
    if (url === "/api/v1/servers/A/content") return Response.json({ kind: "mods", files: [], pending: [] });
    if (url === "/api/v1/jobs/12") return Response.json(runningJob);
    return undefined;
  };
}

function renderAdd(server: Server = stoppedFabric) {
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <MemoryRouter initialEntries={[`/servers/${server.id}/content/add`]}>
        <Routes>
          <Route path="/servers/:id" element={<Outlet context={server} />}>
            <Route path="content" element={<ContentPage />} />
            <Route path="content/add" element={<ContentAddPage />} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function pushJob(job: Partial<Job>) {
  act(() => handlers.get("job.12")?.({ topic: "job.12", seq: 1, data: { ...runningJob, ...job } }));
}

const QUEUED = JSON.stringify([{ source: "modrinth", project_id: "lithium", version_id: null, title: "Lithium", version_label: null }]);

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  handlers.clear();
  window.localStorage.clear();
});

describe("ContentAddPage", () => {
  it("xem chi tiết có mô tả đã lọc, chọn bản, thêm vào danh sách chờ, xem kế hoạch gộp rồi cài", async () => {
    const fetchMock = mockFetch(catalogHandler(samplePlan));
    const user = userEvent.setup();
    renderAdd();

    expect(await screen.findByText("Chọn một mục bên trái để xem chi tiết")).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: /^Lithium/ }));
    const description = await screen.findByLabelText("Mô tả");
    expect(within(description).getByRole("heading", { name: "Tính năng" })).toBeTruthy();
    expect(description.innerHTML).not.toContain("script");
    expect(screen.getByRole("link", { name: "Mã nguồn" }).getAttribute("href")).toBe("https://github.com/CaffeineMC/lithium");

    await user.selectOptions(await screen.findByRole("combobox", { name: "Phiên bản" }), "lithium-0.14.0");
    await user.click(screen.getByRole("button", { name: "Thêm vào danh sách" }));
    expect(screen.getByText("1 mục chờ cài: Lithium")).toBeTruthy();
    expect(screen.getByText("Đã có trong danh sách")).toBeTruthy();
    expect(JSON.parse(window.localStorage.getItem("minehost.contentQueue.A") ?? "[]")).toMatchObject([
      { source: "modrinth", project_id: "lithium", version_id: "lithium-0.14.0" },
    ]);

    await user.click(screen.getByRole("button", { name: "Xem và cài" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText("Fabric API")).toBeTruthy();
    expect(within(dialog).getByText("Thay bản cũ")).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "Cài 1 mục" }));
    await waitFor(() => {
      const post = fetchMock.mock.calls.find(([u, i]) => u === "/api/v1/servers/A/content" && i?.method === "POST");
      expect(JSON.parse(String(post?.[1]?.body))).toEqual({
        items: [{ source: "modrinth", project_id: "lithium", version_id: "lithium-0.14.0" }],
      });
    });
    expect(await within(dialog).findByText("Đang bắt đầu…")).toBeTruthy();
    pushJob({ state: "succeeded" });

    // Cài xong: về trang Mod, danh sách chờ được xóa.
    expect(await screen.findByText("Chưa có mod nào")).toBeTruthy();
    expect(window.localStorage.getItem("minehost.contentQueue.A")).toBeNull();
  });

  it("server đang chạy: kế hoạch phải thay jar đang mở thì không cho cài; bỏ mục khỏi danh sách", async () => {
    window.localStorage.setItem("minehost.contentQueue.A", QUEUED);
    mockFetch(catalogHandler(samplePlan));
    const user = userEvent.setup();
    renderAdd({ ...stoppedFabric, state: "running" });

    await user.click(await screen.findByRole("button", { name: "Xem và cài" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText(/Tắt server để thay bản cũ của Lithium/)).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Cài 1 mục" })).toHaveProperty("disabled", true);

    await user.click(within(dialog).getByRole("button", { name: "Bỏ Lithium" }));
    expect(await within(dialog).findByText("Danh sách chờ đang trống.")).toBeTruthy();
    expect(screen.queryByText(/mục chờ cài/)).toBeNull();
  });

  it("server đang chạy và chỉ thêm mod mới thì cài được, kèm lời nhắc khởi động lại", async () => {
    window.localStorage.setItem("minehost.contentQueue.A", QUEUED);
    const fresh = { ...samplePlan, items: samplePlan.items.map((item) => ({ ...item, replaces_existing: false })) };
    mockFetch(catalogHandler(fresh));
    const user = userEvent.setup();
    renderAdd({ ...stoppedFabric, state: "running" });

    await user.click(await screen.findByRole("button", { name: "Xem và cài" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText("Server đang chạy: mod mới sẽ chạy sau khi khởi động lại.")).toBeTruthy();
    expect(within(dialog).getByRole("button", { name: "Cài 1 mục" })).toHaveProperty("disabled", false);
  });

  it("plugin chỉ tìm trên Modrinth và gửi kiểu sắp xếp đã chọn", async () => {
    const fetchMock = mockFetch((url) =>
      url.startsWith("/api/v1/content/search") ? Response.json({ hits: [], page: 1, page_size: 20, total: 0 }) : undefined,
    );
    const user = userEvent.setup();
    renderAdd(stoppedPaper);

    expect(await screen.findByText("Không tìm thấy kết quả")).toBeTruthy();
    expect(screen.queryByRole("radio", { name: "CurseForge" })).toBeNull();
    await user.selectOptions(screen.getByRole("combobox", { name: "Sắp xếp" }), "downloads");
    await waitFor(() => {
      const urls = fetchMock.mock.calls.map(([u]) => String(u)).filter((u) => u.startsWith("/api/v1/content/search"));
      const last = new URL(urls[urls.length - 1] ?? "", "http://localhost").searchParams;
      expect([last.get("kind"), last.get("sort"), last.get("server_id")]).toEqual(["plugin", "downloads", "P"]);
    });
  });
});
