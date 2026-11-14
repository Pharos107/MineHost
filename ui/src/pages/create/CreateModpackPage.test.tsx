import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job } from "../../api/client";
import type { ContentVersion, SearchHit } from "../../api/content";
import type { LiveMessage } from "../../api/ws";
import { CreateModpackPage } from "./CreateModpackPage";

const GB = 1024 ** 3;
const handlers = new Map<string, (message: LiveMessage) => void>();

vi.mock("../../api/ws", () => ({
  live: {
    subscribe: (topic: string, handler: (message: LiveMessage) => void) => {
      handlers.set(topic, handler);
      return () => handlers.delete(topic);
    },
  },
}));

const system = {
  memory_total_bytes: 16 * GB,
  memory_available_bytes: 12 * GB,
  suggested_memory_mb: { min: 1024, max: 4096 },
  suggested_modpack_memory_mb: { min: 1024, max: 8192 },
};

const packOne: SearchHit = {
  source: "modrinth",
  project_id: "fab-opt",
  slug: "fabulously-optimized",
  kind: "modpack",
  title: "Fabulously Optimized",
  description: "Modpack tối ưu hiệu năng Fabric.",
  icon_url: null,
  downloads: 250_000,
  categories: ["fabric", "optimization"],
  game_versions: ["1.21.4"],
  server_side: "required",
};

const packTwo: SearchHit = {
  ...packOne,
  project_id: "all-the-mods",
  slug: "all-the-mods",
  title: "All The Mods",
  description: "Modpack NeoForge tổng hợp.",
  categories: ["neoforge"],
};

const packVersions: ContentVersion[] = [
  {
    id: "v-new",
    project_id: "fab-opt",
    name: "v6.4.0",
    version_number: "6.4.0",
    version_type: "release",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-15T00:00:00Z",
    file_name: "fo-6.4.0.mrpack",
    file_size: 100_000,
    manual_download: false,
    download_page_url: null,
  },
  {
    id: "v-old",
    project_id: "fab-opt",
    name: "v6.3.0",
    version_number: "6.3.0",
    version_type: "release",
    game_versions: ["1.21.4"],
    loaders: ["fabric"],
    date_published: "2024-12-01T00:00:00Z",
    file_name: "fo-6.3.0.mrpack",
    file_size: 98_000,
    manual_download: false,
    download_page_url: null,
  },
];

const runningJob: Job = {
  id: 21,
  kind: "create_modpack",
  state: "running",
  server_id: "01PACK",
  started_at: "2026-11-15T00:00:00Z",
};

function pushJob(job: Partial<Job>) {
  act(() => handlers.get("job.21")?.({ topic: "job.21", seq: 1, data: { ...runningJob, ...job } }));
}

function renderModpackPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/new/modpack"]}>
        <Routes>
          <Route path="/new/modpack" element={<CreateModpackPage />} />
          <Route path="/servers/:id" element={<p>trang server 01PACK</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  handlers.clear();
});

describe("CreateModpackPage", () => {
  it("nguồn CurseForge luôn mờ với đúng lý do khi thiếu key và khi đã có key", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url === "/api/v1/settings/curseforge") return Response.json({ available: false, source: "none" });
        if (url.startsWith("/api/v1/content/search")) return Response.json({ hits: [packOne], page: 1, page_size: 20, total: 1 });
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    renderModpackPage();

    const cfRadio = await screen.findByRole("radio", { name: "CurseForge" });
    expect(cfRadio).toHaveProperty("disabled", true);
    expect(await screen.findByText("CurseForge chưa sẵn sàng vì chưa cấu hình API key trong Cài đặt.")).toBeTruthy();

    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url === "/api/v1/settings/curseforge") return Response.json({ available: true, source: "builtin" });
        if (url.startsWith("/api/v1/content/search")) return Response.json({ hits: [packOne], page: 1, page_size: 20, total: 1 });
        return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
      }),
    );
    renderModpackPage();
    const cfRadios = await screen.findAllByRole("radio", { name: "CurseForge" });
    expect(cfRadios[1]).toHaveProperty("disabled", true);
    expect(await screen.findByText("Modpack CurseForge sắp có.")).toBeTruthy();
  });

  it("tìm pack, xem thêm, chọn bản, gửi đúng body và hiện tóm tắt sau khi cài xong", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      if (url === "/api/v1/settings/curseforge") return Response.json({ available: true, source: "builtin" });
      if (url.startsWith("/api/v1/content/search")) {
        const page = new URL(url, "http://localhost").searchParams.get("page");
        if (page === "2") return Response.json({ hits: [packTwo], page: 2, page_size: 1, total: 2 });
        return Response.json({ hits: [packOne], page: 1, page_size: 1, total: 2 });
      }
      if (url === "/api/v1/content/modrinth/projects/fab-opt/versions") return Response.json(packVersions);
      if (url === "/api/v1/system") return Response.json(system);
      if (url === "/api/v1/servers/modpack" && init?.method === "POST") {
        return Response.json({ server_id: "01PACK", job_id: 21 }, { status: 202 });
      }
      if (url === "/api/v1/jobs/21") return Response.json(runningJob);
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    });
    vi.stubGlobal("fetch", fetchMock);

    const user = userEvent.setup();
    renderModpackPage();

    await screen.findByRole("button", { name: /Fabulously Optimized/ });
    await user.click(screen.getByRole("button", { name: "Xem thêm" }));
    await screen.findByRole("button", { name: /All The Mods/ });

    await user.click(screen.getByRole("button", { name: /Fabulously Optimized/ }));
    const versionSelect = await screen.findByRole("combobox", { name: "Phiên bản modpack" });
    expect(versionSelect).toHaveProperty("value", "v-new");
    await user.selectOptions(versionSelect, "v-old");

    expect(await screen.findByText("Đề xuất: 8 GB")).toBeTruthy();
    const nameInput = screen.getByRole("textbox", { name: "Tên server" });
    await user.clear(nameInput);
    await user.type(nameInput, "Pack VIP");

    await user.click(screen.getByRole("button", { name: "Nâng cao" }));
    await user.type(screen.getByRole("spinbutton", { name: "Cổng" }), "25570");
    await user.click(screen.getByRole("button", { name: "Tạo server" }));

    const post = fetchMock.mock.calls.find(([u, init]) => u === "/api/v1/servers/modpack" && init?.method === "POST");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({
      source: "modrinth",
      project_id: "fab-opt",
      version_id: "v-old",
      name: "Pack VIP",
      memory_mb: null,
      port: 25570,
    });

    expect(await screen.findByText("Đang cài modpack")).toBeTruthy();
    pushJob({
      progress: {
        step: { kind: "download_pack_file", name: "sodium.jar", index: 1, total: 2 } as never,
        step_index: 7,
        step_count: 9,
        bytes_done: 512 * 1024,
        bytes_total: 1024 * 1024,
      },
    });
    expect(await screen.findByText(/Tải file modpack \(1\/2\): sodium\.jar/)).toBeTruthy();

    pushJob({
      state: "succeeded",
      result: {
        installed_mods: 18,
        skipped_client: ["mods/iris.jar"],
        pending_curseforge: ["mods/optifine.jar"],
      } as never,
    });
    expect(await screen.findByText("Đã cài xong modpack")).toBeTruthy();
    expect(screen.getByText("18")).toBeTruthy();
    expect(screen.getByText("mods/iris.jar")).toBeTruthy();
    expect(screen.getByText("mods/optifine.jar")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Mở trang server" }));
    expect(await screen.findByText("trang server 01PACK")).toBeTruthy();
  });

  it("kéo thả file .mrpack gửi multipart upload và file .zip hiện lỗi MODPACK_INVALID đã dịch", async () => {
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      if (url === "/api/v1/settings/curseforge") return Response.json({ available: false, source: "none" });
      if (url.startsWith("/api/v1/content/search")) return Response.json({ hits: [], page: 1, page_size: 20, total: 0 });
      if (url === "/api/v1/system") return Response.json(system);
      if (url === "/api/v1/servers/modpack/upload" && init?.method === "POST") {
        const form = init.body as FormData;
        const file = form.get("file") as File;
        if (file.name.endsWith(".zip")) {
          return Response.json({ code: "MODPACK_INVALID", params: { reason: "chỉ nhận .mrpack" } }, { status: 422 });
        }
        return Response.json({ server_id: "01PACK", job_id: 21 }, { status: 202 });
      }
      if (url === "/api/v1/jobs/21") return Response.json({ ...runningJob, state: "succeeded", result: { installed_mods: 3, skipped_client: [], pending_curseforge: [] } });
      return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
    });
    vi.stubGlobal("fetch", fetchMock);

    const user = userEvent.setup();
    renderModpackPage();

    const dropInput = await screen.findByLabelText("Chọn file modpack");
    const zipFile = new File(["zip"], "curse-pack.zip", { type: "application/zip" });
    fireEvent.change(dropInput, { target: { files: [zipFile] } });

    expect(await screen.findByText("curse-pack.zip")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Tên server" })).toHaveProperty("value", "curse-pack");
    await screen.findByText("Đề xuất: 8 GB");
    await user.click(screen.getByRole("button", { name: "Tạo server" }));
    expect(await screen.findByText("File modpack không hợp lệ hoặc bị hỏng.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Quay lại" }));
    const mrpackFile = new File(["mrpack"], "my-pack.mrpack", { type: "application/octet-stream" });
    fireEvent.change(await screen.findByLabelText("Chọn file modpack"), { target: { files: [mrpackFile] } });
    await screen.findByText("Đề xuất: 8 GB");
    await user.click(screen.getByRole("button", { name: "Tạo server" }));

    const uploadCalls = fetchMock.mock.calls.filter(([u, init]) => u === "/api/v1/servers/modpack/upload" && init?.method === "POST");
    const sent = uploadCalls[uploadCalls.length - 1]?.[1]?.body as FormData;
    expect(sent.get("name")).toBe("my-pack");
    expect((sent.get("file") as File).name).toBe("my-pack.mrpack");
    expect(await screen.findByText("Đã cài xong modpack")).toBeTruthy();
  });
});
