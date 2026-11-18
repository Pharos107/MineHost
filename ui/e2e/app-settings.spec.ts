import { expect, test, type APIRequestContext } from "@playwright/test";

import { HEADERS } from "./helpers";

/** Mọi spec dùng chung một controller: luôn trả key về bản có sẵn để test CurseForge khác không đổi kết quả. */
async function restoreBuiltinKey(request: APIRequestContext) {
  const res = await request.delete("/api/v1/settings/curseforge", { headers: HEADERS });
  expect(res.ok()).toBe(true);
}

test.afterEach(async ({ request }) => {
  await restoreBuiltinKey(request);
});

// Spec P3, mục 6.5: Cài đặt chung có mục CurseForge, Bộ nhớ đệm tải về và Giới hạn tài nguyên.
test("lưu rồi xóa API key CurseForge riêng trong Cài đặt chung", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByText("Đang dùng key của MineHost.")).toBeVisible();

  const save = page.getByRole("button", { name: "Lưu", exact: true });
  await expect(save).toBeDisabled();
  await page.getByLabel("API key riêng").fill("e2e-user-key");
  await save.click();
  await expect(page.getByText("Đã lưu API key CurseForge.")).toBeVisible();
  await expect(page.getByText("Đang dùng key của bạn.")).toBeVisible();
  // Key không bao giờ được trả về cho trình duyệt: ô nhập trống lại sau khi lưu.
  await expect(page.getByLabel("API key riêng")).toHaveValue("");

  await page.getByRole("button", { name: "Xóa", exact: true }).click();
  await expect(page.getByText("Đang dùng key của MineHost.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Xóa", exact: true })).toHaveCount(0);
});

test("Cài đặt chung hiện dung lượng bộ nhớ đệm và trạng thái giới hạn tài nguyên", async ({ page }) => {
  await page.goto("/settings");

  // Chế độ e2e ghi jar giả thẳng vào thư mục server, không qua cache: dung lượng luôn bằng 0.
  const cache = page.locator("section").filter({ has: page.getByRole("heading", { name: "Bộ nhớ đệm tải về" }) });
  await expect(cache.getByText("Dung lượng")).toBeVisible();
  await expect(cache.getByRole("button", { name: "Xóa bộ nhớ đệm tải về" })).toBeDisabled();

  // Có giới hạn được hay không tùy máy chạy test; trang phải nói rõ một trong hai.
  const limits = page.locator("section").filter({ has: page.getByRole("heading", { name: "Giới hạn tài nguyên" }) });
  const on = limits.getByText(/MineHost giới hạn được RAM/);
  const off = limits.getByText("Chưa giới hạn được RAM trên máy này");
  await expect(on.or(off)).toBeVisible();
});
