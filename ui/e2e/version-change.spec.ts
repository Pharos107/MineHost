import { expect, test } from "@playwright/test";

const HEADERS = { "X-MineHost": "1" };
const NAME = "Version change e2e";
let serverId: string | null = null;

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(NAME)}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
});

// Spec P3, kế hoạch PR-34: đổi Paper 1.21.4 xuống 1.20.4 qua mục Phiên bản, phải đồng ý cảnh báo hạ phiên bản,
// có bản "Trước cập nhật".
test("đổi phiên bản Minecraft từ Cài đặt server", async ({ page, request }) => {
  const created = await request.post("/api/v1/servers", {
    headers: HEADERS,
    data: { name: NAME, loader: "paper", minecraft: "1.21.4" },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  serverId = id;
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");

  await page.goto(`/servers/${id}/settings`);
  await page.getByRole("button", { name: "Đổi phiên bản" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("combobox", { name: "Minecraft" }).selectOption("1.20.4");
  await expect(dialog.getByText(/World đã mở bằng bản mới/)).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Tiếp tục" })).toBeDisabled();
  await dialog.getByRole("checkbox", { name: "Tôi hiểu world có thể không mở được" }).check();
  await dialog.getByRole("button", { name: "Tiếp tục" }).click();

  await expect(page.getByText(/Đã chuyển sang Minecraft 1.20.4/)).toBeVisible({ timeout: 30_000 });
  const server = (await (await request.get(`/api/v1/servers/${id}`)).json()) as { minecraft: string };
  expect(server.minecraft).toBe("1.20.4");
  const backups = (await (await request.get(`/api/v1/servers/${id}/backups`)).json()) as { kind: string }[];
  expect(backups.map((b) => b.kind)).toContain("before_update");
});
