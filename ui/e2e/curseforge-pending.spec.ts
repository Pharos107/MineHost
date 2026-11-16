import { expect, test, type APIRequestContext, type Page } from "@playwright/test";

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** Đóng gói zip không nén khớp từng byte với `build_stored_jar` của `E2eCurseforge`. */
function buildStoredZip(name: string, content: string): Buffer {
  const nameBuf = Buffer.from(name, "utf8");
  const data = Buffer.from(content, "utf8");
  const sum = crc32(data);

  const local = Buffer.alloc(30 + nameBuf.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(sum, 14);
  local.writeUInt32LE(data.length, 18);
  local.writeUInt32LE(data.length, 22);
  local.writeUInt16LE(nameBuf.length, 26);
  nameBuf.copy(local, 30);

  const cd = Buffer.alloc(46 + nameBuf.length);
  cd.writeUInt32LE(0x02014b50, 0);
  cd.writeUInt16LE(20, 4);
  cd.writeUInt16LE(20, 6);
  cd.writeUInt32LE(sum, 16);
  cd.writeUInt32LE(data.length, 20);
  cd.writeUInt32LE(data.length, 24);
  cd.writeUInt16LE(nameBuf.length, 28);
  cd.writeUInt32LE(0, 42);
  nameBuf.copy(cd, 46);

  const cdOffset = local.length + data.length;
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(cd.length, 12);
  eocd.writeUInt32LE(cdOffset, 16);

  return Buffer.concat([local, data, cd, eocd]);
}

async function createFabricServer(request: APIRequestContext, name: string, port: number): Promise<string> {
  const created = await request.post("/api/v1/servers", {
    headers: { "X-MineHost": "1" },
    data: { name, loader: "fabric", minecraft: "1.21.4", port },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
  const eula = await request.post(`/api/v1/servers/${id}/eula`, {
    headers: { "X-MineHost": "1" },
    data: { accept: true },
  });
  expect(eula.ok()).toBe(true);
  return id;
}

async function installRestrictedMod(page: Page, serverId: string) {
  await page.goto(`/servers/${serverId}/content`);
  await page.getByRole("button", { name: "Thêm mod" }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByRole("radio", { name: "CurseForge" }).click();
  await dialog.getByRole("searchbox", { name: "Tìm mod trên CurseForge…" }).fill("Restricted");
  await dialog.getByRole("button", { name: /Restricted Mod/ }).click();
  await expect(dialog.getByText("Cần tải tay")).toBeVisible();
  await dialog.getByRole("button", { name: "Cài" }).click();
  await expect(dialog).toBeHidden({ timeout: 30_000 });
}

let serverId: string | null = null;
let serverName = "";

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  await request.post(`/api/v1/servers/${serverId}/actions/kill`, { headers: { "X-MineHost": "1" } }).catch(() => null);
  await expect
    .poll(
      async () => ((await (await request.get(`/api/v1/servers/${serverId}`)).json()) as { state: string }).state,
      { timeout: 10_000 },
    )
    .toBe("stopped");
  const deleted = await request.delete(`/api/v1/servers/${serverId}?confirm_name=${encodeURIComponent(serverName)}`, {
    headers: { "X-MineHost": "1" },
  });
  expect(deleted.ok()).toBe(true);
  serverId = null;
});

test("tải tay file CurseForge sai hash báo lỗi đúng tên, khớp SHA-1 chuyển Đã có và bật được", async ({
  page,
  request,
}) => {
  serverName = "CF Pending e2e";
  serverId = await createFabricServer(request, serverName, 25578);
  await installRestrictedMod(page, serverId);

  // Hộp chờ hiện ở đầu trang Mod kèm nút Mở trang tải và trạng thái Chờ.
  await expect(page.getByText("Cần tải tay 1 file CurseForge")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Restricted Mod")).toBeVisible();
  await expect(page.getByText("Chờ", { exact: true })).toBeVisible();
  const link = page.getByRole("link", { name: "Mở trang tải" });
  await expect(link).toHaveAttribute("target", "_blank");
  await expect(link).toHaveAttribute("href", /curseforge\.com\/minecraft\/mc-mods\/restricted-mod\/download\/5800999/);

  // Bấm Bật server khi còn mục chờ -> bị chặn với thông báo lỗi.
  await page.getByRole("main").getByRole("button", { name: "Bật server" }).click();
  await expect(page.getByText(/Còn file từ CurseForge cần tự tải/)).toBeVisible({ timeout: 10_000 });

  // Kéo thả file sai -> báo lỗi kèm tên file đúng đang chờ.
  const input = page.getByLabel("Chọn file CurseForge");
  await input.setInputFiles({
    name: "wrong-download.jar",
    mimeType: "application/java-archive",
    buffer: buildStoredZip("fabric.mod.json", JSON.stringify({ id: "wrong" })),
  });
  await expect(page.getByText(/wrong-download\.jar:.*cần file cf-restricted-1\.0\.0\.jar/)).toBeVisible({
    timeout: 15_000,
  });

  // Kéo thả file khớp SHA-1 (kể cả tên file tải về khác) -> dòng chuyển Đã có, mod hiện trong bảng, bật được.
  const matchingJar = buildStoredZip(
    "fabric.mod.json",
    JSON.stringify({
      schemaVersion: 1,
      id: "restricted-mod",
      version: "1.0.0",
      name: "Restricted Mod",
      environment: "*",
    }),
  );
  await input.setInputFiles({
    name: "Restricted Mod (1).jar",
    mimeType: "application/java-archive",
    buffer: matchingJar,
  });

  await expect(page.getByText("Đã có", { exact: true })).toBeVisible({ timeout: 15_000 });
  await expect(page.getByRole("table", { name: "Danh sách mod" }).getByText("Restricted Mod")).toBeVisible();

  await page.getByRole("main").getByRole("button", { name: "Bật server" }).click();
  await expect(page.getByRole("main").getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });
});

test("Bật mà không có các mod này chạy được server khi vẫn còn mục chờ CurseForge", async ({ page, request }) => {
  serverName = "CF Ignore e2e";
  serverId = await createFabricServer(request, serverName, 25579);
  await installRestrictedMod(page, serverId);

  await expect(page.getByText("Cần tải tay 1 file CurseForge")).toBeVisible({ timeout: 15_000 });
  await page.getByRole("button", { name: "Bật mà không có các mod này" }).click();
  await expect(page.getByRole("main").getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });
});
