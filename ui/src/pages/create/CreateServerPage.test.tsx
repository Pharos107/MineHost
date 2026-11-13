import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Job } from "../../api/client";
import type { LiveMessage } from "../../api/ws";
import { CreateServerPage } from "./CreateServerPage";

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

const runningJob: Job = { id: 7, kind: "create_server", state: "running", server_id: "01JB", started_at: "2026-10-27T00:00:00Z" };
const system = { memory_total_bytes: 16 * GB, memory_available_bytes: 10 * GB, suggested_memory_mb: { min: 1024, max: 4096 } };
const versions = ["1.21.4", "1.21.3", "1.21.1", "1.20.6", "1.20.4", "1.19.4"].map((id) => ({ id, release_time: "" }));

function mockApi() {
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    if (url === "/api/v1/catalog/paper/versions") return Response.json(versions);
    // Purpur chưa có 1.21.4 trong dữ liệu giả: wizard phải lấy bản mới nhất của chính Purpur.
    if (url === "/api/v1/catalog/purpur/versions") return Response.json(versions.slice(1));
    if (url === "/api/v1/catalog/fabric/versions") return Response.json(versions);
    if (url === "/api/v1/catalog/fabric/1.21.4/loader-versions") {
      return Response.json([
        { version: "0.16.9", stable: true },
        { version: "0.17.0-beta.1", stable: false },
      ]);
    }
    if (url === "/api/v1/system") return Response.json(system);
    if (url === "/api/v1/servers" && init?.method === "POST") return Response.json({ server_id: "01JB", job_id: 7 }, { status: 202 });
    if (url === "/api/v1/jobs/7") return Response.json(runningJob);
    return Response.json({ code: "NOT_FOUND", params: {} }, { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function renderWizard() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={["/new"]}>
        <Routes>
          <Route path="/new" element={<CreateServerPage />} />
          <Route path="/servers/:id" element={<p>trang chi tiết</p>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function pushJob(job: Partial<Job>) {
  act(() => handlers.get("job.7")?.({ topic: "job.7", seq: 1, data: { ...runningJob, ...job } }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  handlers.clear();
});

describe("CreateServerPage", () => {
  it("chọn Purpur thì lấy phiên bản của Purpur và gửi đúng loại", async () => {
    const fetchMock = mockApi();
    const user = userEvent.setup();
    renderWizard();

    await user.click(screen.getByRole("radio", { name: /^Purpur/ }));
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    const select = await screen.findByRole("combobox", { name: "Phiên bản Minecraft" });
    expect(select).toHaveProperty("value", "1.21.3");
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await user.click(screen.getByRole("button", { name: "Tạo server" }));

    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({ loader: "purpur", minecraft: "1.21.3" });
  });

  it("đi hết 3 bước, gửi yêu cầu tạo rồi theo dõi Job tới trang chi tiết", async () => {
    const fetchMock = mockApi();
    const user = userEvent.setup();
    renderWizard();

    expect(screen.getByRole("radio", { name: /^Vanilla/ })).toHaveProperty("disabled", false);
    await user.click(screen.getByRole("button", { name: "Tiếp" }));

    const select = await screen.findByRole("combobox", { name: "Phiên bản Minecraft" });
    expect(select).toHaveProperty("value", "1.21.4");
    expect(screen.queryByRole("option", { name: "1.19.4" })).toBeNull();
    await user.click(screen.getByRole("switch", { name: "Hiện bản cũ hơn" }));
    await user.selectOptions(select, "1.19.4");
    await user.click(screen.getByRole("button", { name: "Tiếp" }));

    const name = screen.getByRole("textbox", { name: "Tên server" });
    expect(name).toHaveProperty("value", "Server của tôi");
    await user.clear(name);
    expect(screen.getByRole("button", { name: "Tạo server" })).toHaveProperty("disabled", true);
    await user.type(name, "Survival");
    expect(await screen.findByText("Đề xuất: 4 GB")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Tạo server" }));

    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(JSON.parse(String(post?.[1]?.body))).toEqual({ name: "Survival", loader: "paper", minecraft: "1.19.4", memory_mb: null, port: null });

    expect(await screen.findByText("Đang tạo server")).toBeTruthy();
    pushJob({ progress: { step: { kind: "download_java", major: 21 } as never, step_index: 3, step_count: 5, bytes_done: 48 * 1024 ** 2, bytes_total: 61 * 1024 ** 2 } });
    expect(await screen.findByText("Tải Java 21 (48/61 MB)")).toBeTruthy();

    pushJob({ state: "succeeded" });
    expect(await screen.findByText("trang chi tiết")).toBeTruthy();
  });

  it("chọn Server có mod hiện đủ 4 loader theo thứ tự và chọn được bản loader ở Nâng cao", async () => {
    const fetchMock = mockApi();
    const user = userEvent.setup();
    renderWizard();

    await user.click(screen.getByRole("radio", { name: /^Server có mod/ }));
    const loaderRadios = screen
      .getAllByRole("radio")
      .map((el) => (el as HTMLInputElement).value)
      .filter((v) => ["fabric", "neoforge", "forge", "quilt"].includes(v));
    expect(loaderRadios).toEqual(["fabric", "neoforge", "forge", "quilt"]);

    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await screen.findByRole("combobox", { name: "Phiên bản Minecraft" });
    await user.click(screen.getByRole("button", { name: "Nâng cao" }));
    const loaderSelect = await screen.findByRole("combobox", { name: "Phiên bản mod loader" });
    expect(loaderSelect).toHaveProperty("value", "0.16.9");
    await user.selectOptions(loaderSelect, "0.17.0-beta.1");
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await user.click(screen.getByRole("button", { name: "Tạo server" }));

    const post = fetchMock.mock.calls.find(([, init]) => init?.method === "POST");
    expect(JSON.parse(String(post?.[1]?.body))).toMatchObject({
      loader: "fabric",
      minecraft: "1.21.4",
      loader_version: "0.17.0-beta.1",
    });
  });

  it("Job lỗi thì báo lỗi đã dịch và quay lại được bước cuối", async () => {
    mockApi();
    const user = userEvent.setup();
    renderWizard();
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await screen.findByRole("combobox");
    await user.click(screen.getByRole("button", { name: "Tiếp" }));
    await user.click(screen.getByRole("button", { name: "Tạo server" }));
    await screen.findByText("Đang tạo server");

    pushJob({ state: "failed", error: { code: "NETWORK_UNAVAILABLE", params: {} } });
    expect(await screen.findByText("Không kết nối được Internet. Kiểm tra mạng rồi thử lại.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Quay lại" }));
    expect(screen.getByRole("textbox", { name: "Tên server" })).toHaveProperty("value", "Server của tôi");
  });
});
