import { expect, test } from "@playwright/test";

import { createServer, removeServer, startServer } from "./helpers";

let serverIds: string[] = [];

test.afterEach(async ({ request }) => {
  const ids = serverIds;
  serverIds = [];
  for (const id of ids) await removeServer(request, id);
});

// Spec P2, mục 6.2: lọc theo trạng thái, tìm theo tên, bật server bằng nút trên dòng của bảng.
test("lọc danh sách máy chủ và bật server từ dòng", async ({ page, request }) => {
  const idle = await createServer(request, "Danh sách nghỉ", { eula: true });
  serverIds.push(idle);
  const busy = await createServer(request, "Danh sách chạy", { eula: true });
  serverIds.push(busy);
  await startServer(request, busy);

  await page.goto("/servers");
  await page.getByRole("radio", { name: "Bảng" }).click();
  const table = page.getByRole("table", { name: "Server của bạn" });
  const idleRow = table.getByRole("row", { name: /Danh sách nghỉ/ });
  const busyRow = table.getByRole("row", { name: /Danh sách chạy/ });
  await expect(idleRow).toBeVisible();
  await expect(busyRow).toBeVisible();

  const stateFilter = page.getByRole("combobox", { name: "Lọc theo trạng thái" });
  await stateFilter.selectOption("running");
  await expect(busyRow).toBeVisible();
  await expect(idleRow).toBeHidden();
  await stateFilter.selectOption("stopped");
  await expect(idleRow).toBeVisible();
  await expect(busyRow).toBeHidden();
  await stateFilter.selectOption("all");

  await page.getByRole("searchbox", { name: "Tìm kiếm" }).fill("nghỉ");
  await expect(idleRow).toBeVisible();
  await expect(busyRow).toBeHidden();

  await idleRow.getByRole("button", { name: "Bật server: Danh sách nghỉ" }).click();
  await expect(idleRow.getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });
});
