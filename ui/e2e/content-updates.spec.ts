import { expect, test, type APIRequestContext } from "@playwright/test";

const HEADERS = { "X-MineHost": "1" };
const NAME = "Mod update e2e";
let serverId: string | null = null;

async function waitJob(request: APIRequestContext, job: number) {
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
}

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(NAME)}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
});

// Spec P3, kế hoạch PR-34: cài bản cũ của Lithium, kiểm tra cập nhật thấy bản mới, cập nhật có sao lưu trước.
test("kiểm tra rồi cập nhật một mod lên bản mới", async ({ page, request }) => {
  const created = await request.post("/api/v1/servers", {
    headers: HEADERS,
    data: { name: NAME, loader: "fabric", minecraft: "1.21.4" },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: createJob } = (await created.json()) as { server_id: string; job_id: number };
  serverId = id;
  await waitJob(request, createJob);
  const install = await request.post(`/api/v1/servers/${id}/content`, {
    headers: HEADERS,
    data: { source: "modrinth", project_id: "e2e-lithium", version_id: "e2e-lithium-0.14.0" },
  });
  expect(install.status()).toBe(202);
  await waitJob(request, ((await install.json()) as { job_id: number }).job_id);

  await page.goto(`/servers/${id}/content`);
  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("0.14.0")).toBeVisible();

  await page.getByRole("button", { name: "Kiểm tra cập nhật" }).click();
  await expect(table.getByText("Có bản mới")).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Cập nhật 1 mục" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText("lithium-0.14.0.jar → 0.14.2")).toBeVisible();
  await dialog.getByRole("button", { name: "Cập nhật" }).click();

  await expect(page.getByText(/Đã cập nhật 1 mục/)).toBeVisible({ timeout: 30_000 });
  await expect(table.getByText("0.14.2")).toBeVisible();
  await expect(table.getByText("Có bản mới")).toBeHidden();

  const backups = (await (await request.get(`/api/v1/servers/${id}/backups`)).json()) as { kind: string; protected: boolean }[];
  expect(backups.some((b) => b.kind === "before_update" && b.protected)).toBe(true);
});
