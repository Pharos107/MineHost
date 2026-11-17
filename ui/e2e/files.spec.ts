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

  // Trình sửa là CodeMirror (spec trang Tệp, mục 1): vùng `contenteditable`, không phải textarea.
  const editor = page.getByRole("textbox", { name: "Nội dung server.properties" });
  await expect(editor).toContainText("motd=");
  const original = await editor.innerText();
  const edited = original.replace(/^motd=.*$/m, "motd=Đã sửa từ e2e");
  await editor.fill(edited);
  await expect(page.getByText("Chưa lưu", { exact: true })).toBeVisible();
  await page.keyboard.press("Control+s");
  await expect(page.getByText("Đã lưu", { exact: true })).toBeVisible();

  // Quay lại danh sách rồi mở lại, và tải lại trang để chắc nội dung đọc từ đĩa chứ không từ bộ nhớ đệm.
  await page.getByRole("button", { name: "Quay lại thư mục" }).click();
  await page.getByRole("link", { name: "server.properties" }).click();
  await expect(editor).toContainText("motd=Đã sửa từ e2e");
  await page.reload();
  await expect(page.getByRole("textbox", { name: "Nội dung server.properties" })).toContainText("motd=Đã sửa từ e2e");
});

// Spec trang Tệp, mục 2: nén, giải nén, tìm trong thư mục con, chọn nhiều rồi xóa.
test("nén, giải nén, tìm và xóa nhiều mục", async ({ page, request }) => {
  const id = await createServer(request, "Zip e2e");
  serverId = id;
  await page.goto(`/servers/${id}/files`);
  const table = page.getByRole("table", { name: "Tệp trong thư mục" });

  await table.getByRole("button", { name: "Thao tác khác cho server.properties" }).click();
  await page.getByRole("menuitem", { name: "Nén thành zip" }).click();
  const compress = page.getByRole("dialog", { name: "Nén 1 mục" });
  await compress.getByRole("textbox", { name: "Tên file zip" }).fill("cau-hinh.zip");
  await compress.getByRole("button", { name: "Nén" }).click();
  await expect(table.getByText("cau-hinh.zip", { exact: true })).toBeVisible({ timeout: 15_000 });

  await table.getByRole("button", { name: "Thao tác khác cho cau-hinh.zip" }).click();
  await page.getByRole("menuitem", { name: "Giải nén" }).click();
  await page.getByRole("dialog", { name: "Giải nén cau-hinh.zip" }).getByRole("button", { name: "Giải nén" }).click();
  await expect(table.getByRole("link", { name: "cau-hinh", exact: true })).toBeVisible({ timeout: 15_000 });

  await page.getByRole("button", { name: "Tìm trong thư mục con" }).click();
  const search = page.getByRole("dialog", { name: "Tìm file" });
  await search.getByRole("textbox").fill("properties");
  await search.getByRole("button", { name: "Tìm" }).click();
  await expect(search.getByRole("link", { name: /cau-hinh\/server\.properties/ })).toBeVisible();
  await search.getByRole("button", { name: "Đóng" }).first().click();

  await table.getByRole("checkbox", { name: "Chọn cau-hinh", exact: true }).check();
  await table.getByRole("checkbox", { name: "Chọn cau-hinh.zip" }).check();
  await expect(page.getByText("Đã chọn 2")).toBeVisible();
  await page.getByRole("button", { name: "Xóa", exact: true }).click();
  await page.getByRole("dialog", { name: "Xóa 2 mục?" }).getByRole("button", { name: "Xóa" }).click();
  await expect(page.getByText("Đã chuyển vào thùng rác")).toBeVisible();
  await expect(table.getByText("cau-hinh.zip", { exact: true })).toBeHidden();
});
