import { expect, test, type APIRequestContext } from "@playwright/test";

/** Tạo server qua API (catalog giả của chế độ e2e) rồi chờ Job cài xong. */
async function createServer(request: APIRequestContext, name: string): Promise<string> {
  const created = await request.post("/api/v1/servers", {
    headers: { "X-MineHost": "1" },
    data: { name, loader: "paper", minecraft: "1.21.4" },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
  return id;
}

// Spec P2, kế hoạch PR-25: sao lưu, bảo vệ rồi thử xóa, khôi phục có xác nhận.
test("sao lưu, bảo vệ và khôi phục", async ({ page, request }) => {
  const id = await createServer(request, "Sao lưu e2e");
  await page.goto(`/servers/${id}/backups`);

  await page.getByRole("textbox", { name: "Ghi chú" }).fill("Trước khi thử");
  await page.getByText("Bảo vệ bản này", { exact: true }).first().click();
  await page.getByRole("button", { name: "Sao lưu", exact: true }).click();
  const table = page.getByRole("table");
  await expect(table.getByText("Trước khi thử")).toBeVisible({ timeout: 30_000 });

  // Bản được bảo vệ: Xóa bị tắt trong menu.
  await table.getByRole("button", { name: /Thao tác khác cho/ }).first().click();
  await expect(page.getByRole("menuitem", { name: "Xóa" })).toBeDisabled();
  await page.keyboard.press("Escape");

  await table.getByRole("button", { name: /Khôi phục/ }).first().click();
  const dialog = page.getByRole("dialog");
  const confirm = dialog.getByRole("button", { name: "Khôi phục" });
  await expect(confirm).toBeDisabled();
  await dialog.getByRole("checkbox", { name: "Tôi hiểu" }).check();
  await confirm.click();

  // Khôi phục tạo thêm một bản "Trước khôi phục".
  await expect(table.getByText("Trước khôi phục")).toBeVisible({ timeout: 30_000 });
});
