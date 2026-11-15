import { expect, test } from "@playwright/test";

import { createServer, removeServer } from "./helpers";

let serverId: string | null = null;

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  await removeServer(request, id);
});

// Spec P2, mục 6.6: tạo thư mục, tải file lên, đổi tên, xóa vào thùng rác.
test("tạo thư mục, tải lên, đổi tên và xóa file", async ({ page, request }) => {
  const id = await createServer(request, "Tệp e2e");
  serverId = id;
  await page.goto(`/servers/${id}/files`);
  const table = page.getByRole("table", { name: "Tệp trong thư mục" });
  await expect(table.getByRole("link", { name: "server.properties" })).toBeVisible();

  await page.getByRole("button", { name: "Thư mục mới" }).click();
  const create = page.getByRole("dialog", { name: "Tạo thư mục mới" });
  await create.getByRole("textbox", { name: "Tên thư mục" }).fill("ghi-chu");
  await create.getByRole("button", { name: "Tạo" }).click();
  await expect(create).toBeHidden();
  await table.getByRole("link", { name: "ghi-chu" }).click();
  await expect(page.getByText("Thư mục này trống")).toBeVisible();

  await page.locator('input[type="file"]').setInputFiles({
    name: "nhat-ky.txt",
    mimeType: "text/plain",
    buffer: Buffer.from("Ngày đầu tiên trên server.\n", "utf8"),
  });
  await expect(page.getByText("Đã tải lên 1 file.")).toBeVisible({ timeout: 15_000 });
  await expect(table.getByRole("link", { name: "nhat-ky.txt" })).toBeVisible();

  await table.getByRole("button", { name: "Thao tác khác cho nhat-ky.txt" }).click();
  await page.getByRole("menuitem", { name: "Đổi tên" }).click();
  const rename = page.getByRole("dialog", { name: "Đổi tên" });
  await rename.getByRole("textbox", { name: "Tên tệp" }).fill("nhat-ky-cu.txt");
  await rename.getByRole("button", { name: "Đổi tên" }).click();
  await expect(table.getByRole("link", { name: "nhat-ky-cu.txt" })).toBeVisible();
  await expect(table.getByRole("link", { name: "nhat-ky.txt", exact: true })).toBeHidden();

  await table.getByRole("button", { name: "Thao tác khác cho nhat-ky-cu.txt" }).click();
  await page.getByRole("menuitem", { name: "Xóa" }).click();
  const confirm = page.getByRole("dialog", { name: "Xóa nhat-ky-cu.txt?" });
  await confirm.getByRole("button", { name: "Xóa" }).click();
  await expect(page.getByText("Đã chuyển vào thùng rác")).toBeVisible();
  await expect(page.getByText("Thư mục này trống")).toBeVisible();
});

// Spec P2, mục 6.6: sửa `server.properties` trong trình sửa, lưu, mở lại thấy nội dung mới.
test("sửa server.properties rồi mở lại thấy nội dung mới", async ({ page, request }) => {
  const id = await createServer(request, "Sửa tệp e2e");
  serverId = id;
  await page.goto(`/servers/${id}/files`);
  await page.getByRole("link", { name: "server.properties" }).click();

  const editor = page.getByRole("textbox", { name: "Nội dung server.properties" });
  await expect(editor).toHaveValue(/motd=/);
  const original = await editor.inputValue();
  const edited = original.replace(/^motd=.*$/m, "motd=Đã sửa từ e2e");
  await editor.fill(edited);
  await expect(page.getByText("Có thay đổi chưa lưu.")).toBeVisible();
  await page.getByRole("button", { name: "Lưu" }).click();
  await expect(page.getByText("Đã lưu.")).toBeVisible();

  // Quay lại danh sách rồi mở lại, và tải lại trang để chắc nội dung đọc từ đĩa chứ không từ bộ nhớ đệm.
  await page.getByRole("button", { name: "Quay lại thư mục" }).click();
  await page.getByRole("link", { name: "server.properties" }).click();
  await expect(editor).toHaveValue(/^motd=Đã sửa từ e2e$/m);
  await page.reload();
  await expect(page.getByRole("textbox", { name: "Nội dung server.properties" })).toHaveValue(/^motd=Đã sửa từ e2e$/m);
});
