import { expect, test, type Page } from "@playwright/test";

const PASSWORD = "correct-horse";

/** Mọi test dùng chung một controller: luôn xóa mật khẩu ở cuối để test khác không bị bắt đăng nhập. */
async function removePassword(page: Page) {
  await page.goto("/settings");
  // Test hỏng giữa chừng có thể để trình duyệt ở trang Đăng nhập. Chờ AuthGate quyết định xong.
  await page.waitForLoadState("networkidle");
  if (page.url().includes("/login")) await login(page);
  await expect(page).toHaveURL(/\/settings$/);
  const current = page.getByLabel("Mật khẩu hiện tại", { exact: true });
  if (!(await current.isVisible())) return;
  await current.fill(PASSWORD);
  await page.getByRole("button", { name: "Xóa mật khẩu" }).click();
  await expect(page.getByText(/Chưa đặt mật khẩu/)).toBeVisible();
}

async function login(page: Page) {
  await page.getByLabel("Mật khẩu", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đăng nhập" }).click();
}

test.afterEach(async ({ page }) => {
  await removePassword(page);
});

// Spec P2, mục 6.10: đặt mật khẩu, bị đưa về /login, đăng nhập, quay lại trang cũ; bật LAN hiện địa chỉ.
test("đặt mật khẩu, đăng nhập lại và bật LAN", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByText(/Chưa đặt mật khẩu/)).toBeVisible();
  await expect(page.getByRole("switch", { name: /Cho máy khác trong mạng LAN/ })).toBeDisabled();

  await page.getByLabel("Mật khẩu mới", { exact: true }).fill(PASSWORD);
  await page.getByLabel("Nhập lại mật khẩu mới", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đặt mật khẩu" }).click();

  await expect(page).toHaveURL(/\/login\?next=%2Fsettings$/);
  await page.getByLabel("Mật khẩu", { exact: true }).fill("wrong-password");
  await page.getByRole("button", { name: "Đăng nhập" }).click();
  await expect(page.getByText("Mật khẩu không đúng.")).toBeVisible();

  await page.getByLabel("Mật khẩu", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đăng nhập" }).click();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByText(/Đã đặt mật khẩu/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Đăng xuất" })).toBeVisible();

  const lan = page.getByRole("switch", { name: /Cho máy khác trong mạng LAN/ });
  await lan.check();
  await expect(lan).toBeChecked();
  // Máy CI có địa chỉ LAN; controller mở listener trên đó cùng cổng với 127.0.0.1.
  await expect(page.getByText(/^http:\/\/\d+\.\d+\.\d+\.\d+:\d+$/).first()).toBeVisible();
  await lan.uncheck();
  await expect(lan).not.toBeChecked();
});

test("đăng xuất thì phải đăng nhập lại", async ({ page }) => {
  await page.goto("/settings");
  await page.getByLabel("Mật khẩu mới", { exact: true }).fill(PASSWORD);
  await page.getByLabel("Nhập lại mật khẩu mới", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đặt mật khẩu" }).click();
  await page.getByLabel("Mật khẩu", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đăng nhập" }).click();
  await expect(page).toHaveURL(/\/settings$/);

  await page.goto("/jobs");
  await page.getByRole("button", { name: "Đăng xuất" }).click();
  await expect(page).toHaveURL(/\/login\?next=%2Fjobs$/);

  await page.getByLabel("Mật khẩu", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Đăng nhập" }).click();
  await expect(page).toHaveURL(/\/jobs$/);
});
