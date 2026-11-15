import { expect, test } from "@playwright/test";

import { createServer, removeServer, startServer } from "./helpers";

let serverId: string | null = null;

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  await removeServer(request, id);
});

// Spec P2, mục 6.5: gửi lệnh từ ô nhập, lọc dòng theo từ khóa, xóa màn hình chỉ phía trình duyệt.
test("gửi lệnh và lọc dòng trong console", async ({ page, request }) => {
  const id = await createServer(request, "Console e2e", { eula: true });
  serverId = id;
  await startServer(request, id);
  await page.goto(`/servers/${id}/console`);

  // Mở trang là nhận lại bộ đệm log từ lúc server bật.
  const log = page.getByRole("log", { name: "Log của server" });
  await expect(log).toContainText("Starting minecraft server version fake", { timeout: 15_000 });

  const command = page.getByRole("textbox", { name: "Lệnh" });
  await command.fill("say hello from e2e");
  await command.press("Enter");
  // `fake-mc` in lại nội dung của `say`; dòng lệnh người dùng gõ hiện với dấu `>`.
  await expect(log).toContainText("[Server] hello from e2e", { timeout: 15_000 });
  await expect(log).toContainText("> say hello from e2e");
  await expect(command).toHaveValue("");

  const filter = page.getByRole("searchbox", { name: "Lọc dòng" });
  await filter.fill("HELLO FROM");
  await expect(log).toContainText("[Server] hello from e2e");
  await expect(log).not.toContainText("Starting minecraft server version fake");

  await filter.fill("");
  await expect(log).toContainText("Starting minecraft server version fake");

  await page.getByRole("button", { name: "Xóa màn hình" }).click();
  await expect(log).not.toContainText("hello from e2e");
  // Log trên server giữ nguyên: mở lại trang là thấy lại.
  await page.reload();
  await expect(page.getByRole("log", { name: "Log của server" })).toContainText("[Server] hello from e2e", { timeout: 15_000 });
});
