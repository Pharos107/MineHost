import { expect, test, type APIRequestContext } from "@playwright/test";

import { installQueued, queueMod } from "./helpers";

const HEADERS = { "X-MineHost": "1" };
const NAME = "Mod install e2e";
let serverId: string | null = null;

async function createFabricServer(request: APIRequestContext): Promise<string> {
  const created = await request.post("/api/v1/servers", {
    headers: HEADERS,
    data: { name: NAME, loader: "fabric", minecraft: "1.21.4" },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
  return id;
}

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(NAME)}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
});

// Spec P3, kế hoạch PR-20: tìm mod có dependency trong nguồn giả, thấy dependency trước khi cài, cả hai hiện trong bảng.
test("tìm và cài mod có dependency từ nguồn giả", async ({ page, request }) => {
  const id = await createFabricServer(request);
  serverId = id;

  const dialog = await queueMod(page, id, { search: "Lithium", result: /^Lithium/ });
  await expect(dialog.getByText("Danh sách sẽ cài")).toBeVisible();
  await expect(dialog.getByText(/Dependency cài kèm/)).toBeVisible();
  await expect(dialog.getByText("Fabric API")).toBeVisible();
  await installQueued(page, id, dialog);

  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("Lithium")).toBeVisible();
  await expect(table.getByText("Fabric API")).toBeVisible();
});

// Spec P3, kế hoạch PR-28: tìm và cài mod từ nguồn CurseForge giả kèm dependency relationType = 3 và mod tải tay.
test("tìm và cài mod có dependency từ nguồn CurseForge giả", async ({ page, request }) => {
  const id = await createFabricServer(request);
  serverId = id;

  const dialog = await queueMod(page, id, { curseforge: true, search: "JEI", result: /^Just Enough Items/ });
  await expect(dialog.getByText("Danh sách sẽ cài")).toBeVisible();
  await expect(dialog.getByText(/Dependency cài kèm/)).toBeVisible();
  await expect(dialog.getByText("Fabric API (CF)")).toBeVisible();
  await installQueued(page, id, dialog);

  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("Just Enough Items", { exact: true })).toBeVisible();
  await expect(table.getByText("Fabric API (CF)", { exact: true })).toBeVisible();

  // Kiểm tra màn xác nhận hiển thị nhãn + link tải tay cho mod cấm phân phối.
  const restricted = await queueMod(page, id, { curseforge: true, search: "Restricted", result: /^Restricted Mod/ });
  await expect(restricted.getByText("Cần tải tay")).toBeVisible();
  await expect(restricted.getByRole("link", { name: "Mở trang tải" })).toHaveAttribute("href", /curseforge\.com\/minecraft\/mc-mods\/restricted-mod/);
  await installQueued(page, id, restricted);
});
