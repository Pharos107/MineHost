// Hàm dùng chung cho các spec e2e: tạo, bật và dọn server qua API để mỗi test chỉ đi qua giao diện
// ở phần nó cần kiểm tra. Không phải file test (Playwright chỉ chạy `*.spec.ts`).
import { expect, type APIRequestContext } from "@playwright/test";

export const HEADERS = { "X-MineHost": "1" };

interface ServerDto {
  id: string;
  name: string | null;
  state: string;
  port: number | null;
}

export async function getServer(request: APIRequestContext, id: string): Promise<ServerDto> {
  return (await (await request.get(`/api/v1/servers/${id}`)).json()) as ServerDto;
}

/** Chờ Job xong; Job lỗi thì báo ngay thay vì chờ hết giờ. */
export async function waitJob(request: APIRequestContext, job: number): Promise<void> {
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
}

/** Tạo server Paper qua API (catalog giả của chế độ e2e) và chờ cài xong. `eula` thì đồng ý luôn. */
export async function createServer(request: APIRequestContext, name: string, opts: { eula?: boolean } = {}): Promise<string> {
  const created = await request.post("/api/v1/servers", { headers: HEADERS, data: { name, loader: "paper", minecraft: "1.21.4" } });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await waitJob(request, job);
  if (opts.eula) {
    expect((await request.post(`/api/v1/servers/${id}/eula`, { headers: HEADERS, data: { accept: true } })).ok()).toBe(true);
  }
  return id;
}

export async function startServer(request: APIRequestContext, id: string): Promise<void> {
  expect((await request.post(`/api/v1/servers/${id}/actions/start`, { headers: HEADERS })).ok()).toBe(true);
  await expect.poll(async () => (await getServer(request, id)).state, { timeout: 30_000 }).toBe("running");
}

/**
 * Tắt (nếu đang chạy) rồi xóa server. Các spec dùng chung một controller và server-lifecycle cần
 * danh sách server trống, nên mọi spec tạo server phải gọi hàm này trong `afterEach`.
 * Tên lấy từ API vì test có thể đã đổi tên server.
 */
export async function removeServer(request: APIRequestContext, id: string): Promise<void> {
  const server = await getServer(request, id);
  if (server.state !== "stopped" && server.state !== "crashed") {
    await request.post(`/api/v1/servers/${id}/actions/stop`, { headers: HEADERS });
    await expect.poll(async () => (await getServer(request, id)).state, { timeout: 30_000 }).toBe("stopped");
  }
  const name = encodeURIComponent(server.name ?? id);
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${name}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
}
