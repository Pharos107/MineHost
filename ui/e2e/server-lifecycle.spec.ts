import { expect, test } from "@playwright/test";

// Luồng chính của P1 qua giao diện thật: tạo server Paper (catalog giả), đồng ý EULA, bật, tắt.
test("tạo, bật và tắt một server", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Tạo server đầu tiên" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Tạo server" })).toBeVisible();
  await page.getByRole("button", { name: "Tiếp" }).click();
  await expect(page.getByRole("combobox", { name: "Phiên bản Minecraft" })).toHaveValue("1.21.4");
  await page.getByRole("button", { name: "Tiếp" }).click();
  await page.getByRole("button", { name: "Tạo server" }).click();

  // Tạo xong thì tự chuyển sang trang server.
  await expect(page.getByRole("heading", { level: 1, name: "Server của tôi" })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Bật server" }).click();
  await page.getByRole("button", { name: "Tôi đồng ý" }).click();
  await expect(page.getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });

  await page.getByRole("button", { name: "Tắt" }).click();
  await expect(page.getByText("Đã tắt", { exact: true })).toBeVisible({ timeout: 30_000 });

  await page.getByRole("link", { name: "Tất cả máy chủ" }).click();
  await page.getByRole("link", { name: "Tác vụ" }).click();
  await expect(page.getByText("Xong", { exact: true })).toBeVisible();
});
