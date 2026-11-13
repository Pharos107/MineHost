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
  // Các spec khác có thể để lại Job của chúng; chỉ cần có Job tạo server đã xong.
  await expect(page.getByRole("row", { name: /Tạo server/ }).getByText("Xong", { exact: true }).first()).toBeVisible();
});

test("tạo, bật và tắt một server Fabric từ wizard", async ({ page }) => {
  await page.goto("/new");
  await expect(page.getByRole("heading", { level: 1, name: "Tạo server" })).toBeVisible();
  await page.getByRole("radio", { name: /^Server có mod/ }).click();
  await page.getByRole("button", { name: "Tiếp" }).click();
  await expect(page.getByRole("combobox", { name: "Phiên bản Minecraft" })).toHaveValue("1.21.4");
  await page.getByRole("button", { name: "Tiếp" }).click();
  const name = page.getByRole("textbox", { name: "Tên server" });
  await name.fill("Fabric E2E");
  await page.getByRole("button", { name: "Tạo server" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Fabric E2E" })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Bật server" }).click();
  await page.getByRole("button", { name: "Tôi đồng ý" }).click();
  await expect(page.getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });

  await page.getByRole("link", { name: "Tất cả máy chủ" }).click();
  const card = page.getByRole("link", { name: "Fabric E2E" }).locator("xpath=ancestor::article");
  await expect(card.getByText(/Fabric 1\.21\.4/)).toBeVisible();

  await page.getByRole("link", { name: "Fabric E2E" }).click();
  await page.getByRole("button", { name: "Tắt" }).click();
  await expect(page.getByText("Đã tắt", { exact: true })).toBeVisible({ timeout: 30_000 });

  await page.getByRole("link", { name: "Cài đặt server" }).click();
  await page.getByRole("button", { name: "Nâng cao" }).click();
  await page.getByRole("combobox", { name: "Phiên bản Java" }).selectOption("17");
  await expect(page.getByText(/thường dùng Java 21/)).toBeVisible();
  await page.getByRole("button", { name: "Lưu" }).click();
  await expect(page.getByText("Đã lưu.")).toBeVisible();

  await page.reload();
  await expect(page.getByRole("combobox", { name: "Phiên bản Java" })).toHaveValue("17");
});
