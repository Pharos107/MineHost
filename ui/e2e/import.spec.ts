import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { getServer, removeServer } from "./helpers";

let serverId: string | null = null;
let source: string | null = null;

test.afterEach(async ({ request }) => {
  if (source !== null) rmSync(source, { recursive: true, force: true });
  source = null;
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  await removeServer(request, id);
});

/**
 * Thư mục giống server Paper đã chạy: jar, `version_history.json` Paper ghi lúc chạy, cổng riêng
 * trong `server.properties` và `eula=true`. Jar chỉ là vài byte vì Java giả không đọc nó.
 */
function paperFolder(): string {
  const dir = mkdtempSync(join(tmpdir(), "minehost-e2e-import-"));
  writeFileSync(join(dir, "paper-1.21.4-232.jar"), "fake jar");
  writeFileSync(join(dir, "version_history.json"), JSON.stringify({ currentVersion: "git-Paper-232 (MC: 1.21.4)" }));
  writeFileSync(join(dir, "server.properties"), "motd=Server cũ\nserver-port=25733\n");
  writeFileSync(join(dir, "eula.txt"), "eula=true\n");
  return dir;
}

// Spec P2, mục 6.11: dán đường dẫn, MineHost nhận loại và phiên bản, chép vào, giữ cổng, hỏi lại EULA.
test("nhập thư mục server có sẵn", async ({ page, request }) => {
  const dir = paperFolder();
  source = dir;
  await page.goto("/new");
  await page.getByRole("radio", { name: /^Nhập thư mục có sẵn/ }).click();
  await page.getByRole("button", { name: "Tiếp" }).click();

  await expect(page.getByRole("heading", { level: 1, name: "Nhập server có sẵn" })).toBeVisible();
  await page.getByRole("textbox", { name: "Thư mục server" }).fill(dir);
  await page.getByRole("button", { name: "Kiểm tra thư mục" }).click();
  await expect(page.getByText("paper-1.21.4-232.jar")).toBeVisible();
  await expect(page.getByText("1.21.4", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Tiếp" }).click();

  await page.getByRole("textbox", { name: "Tên server" }).fill("Server nhập e2e");
  await page.getByRole("button", { name: "Nhập server" }).click();

  // Nhập xong thì tự chuyển sang trang server.
  await expect(page.getByRole("heading", { level: 1, name: "Server nhập e2e" })).toBeVisible({ timeout: 30_000 });
  serverId = new URL(page.url()).pathname.split("/")[2] ?? null;
  expect(serverId).toBeTruthy();
  const server = await getServer(request, serverId ?? "");
  expect(server.port).toBe(25733);
  // Thư mục được chép, bản gốc giữ nguyên.
  expect(existsSync(join(dir, "paper-1.21.4-232.jar"))).toBe(true);

  // `eula=true` trong thư mục cũ không được tính: vẫn phải bấm đồng ý trong MineHost.
  await page.getByRole("main").getByRole("button", { name: "Bật server" }).click();
  await expect(page.getByRole("button", { name: "Tôi đồng ý" })).toBeVisible();
});
