import { expect, test, type APIRequestContext } from "@playwright/test";

const HEADERS = { "X-MineHost": "1" };
const NAME = "Người chơi e2e";
let serverId: string | null = null;

async function serverState(request: APIRequestContext, id: string): Promise<string> {
  return ((await (await request.get(`/api/v1/servers/${id}`)).json()) as { state: string }).state;
}

/** Tạo server qua API (catalog giả của chế độ e2e), đồng ý EULA, bật và chờ `running`. */
async function runningServer(request: APIRequestContext): Promise<string> {
  const created = await request.post("/api/v1/servers", { headers: HEADERS, data: { name: NAME, loader: "paper", minecraft: "1.21.4" } });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
  expect((await request.post(`/api/v1/servers/${id}/eula`, { headers: HEADERS, data: { accept: true } })).ok()).toBe(true);
  expect((await request.post(`/api/v1/servers/${id}/actions/start`, { headers: HEADERS })).ok()).toBe(true);
  await expect.poll(() => serverState(request, id), { timeout: 30_000 }).toBe("running");
  return id;
}

// Các spec dùng chung một controller; server-lifecycle cần danh sách server trống.
test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  await request.post(`/api/v1/servers/${id}/actions/stop`, { headers: HEADERS });
  await expect.poll(() => serverState(request, id), { timeout: 30_000 }).toBe("stopped");
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(NAME)}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
});

// Spec P2, kế hoạch PR-18: `fake-mc` in dòng `joined` thì người đó hiện trong bảng Đang online.
test("người vào server hiện trong bảng Đang online", async ({ page, request }) => {
  const id = await runningServer(request);
  serverId = id;
  await page.goto(`/servers/${id}/players`);
  await expect(page.getByText("Chưa có ai online.")).toBeVisible();

  const sent = await request.post(`/api/v1/servers/${id}/console`, { headers: HEADERS, data: { command: "fake-join Steve" } });
  expect(sent.ok()).toBe(true);
  const online = page.getByRole("table", { name: "Đang online" });
  await expect(online.getByText("Steve", { exact: true })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Đang online (1/20)")).toBeVisible();

  // Thêm theo tên khi server đang chạy: gửi lệnh `whitelist add` cho Minecraft.
  await page.getByRole("textbox", { name: "Tên người chơi" }).fill("Alex");
  await page.getByRole("button", { name: "Thêm vào whitelist" }).click();
  await expect(page.getByText(/Đã gửi lệnh thêm Alex vào whitelist/)).toBeVisible();
});
