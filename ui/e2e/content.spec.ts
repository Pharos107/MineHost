import { expect, test, type APIRequestContext } from "@playwright/test";

const HEADERS = { "X-MineHost": "1" };
const NAME = "Mod e2e";
let serverId: string | null = null;

function crc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of data) {
    crc ^= byte;
    for (let i = 0; i < 8; i += 1) {
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** Tạo file `.jar` (ZIP không nén) chứa một file metadata để kiểm tra luồng tải lên thật. */
function buildJarBuffer(entryName: string, content: string): Buffer {
  const name = Buffer.from(entryName, "utf8");
  const body = Buffer.from(content, "utf8");
  const crc = crc32(body);

  const local = Buffer.alloc(30 + name.length);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4);
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(body.length, 18);
  local.writeUInt32LE(body.length, 22);
  local.writeUInt16LE(name.length, 26);
  name.copy(local, 30);

  const cdOffset = local.length + body.length;
  const cd = Buffer.alloc(46 + name.length);
  cd.writeUInt32LE(0x02014b50, 0);
  cd.writeUInt16LE(20, 4);
  cd.writeUInt16LE(20, 6);
  cd.writeUInt32LE(crc, 16);
  cd.writeUInt32LE(body.length, 20);
  cd.writeUInt32LE(body.length, 24);
  cd.writeUInt16LE(name.length, 28);
  name.copy(cd, 46);

  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(cd.length, 12);
  eocd.writeUInt32LE(cdOffset, 16);

  return Buffer.concat([local, body, cd, eocd]);
}

async function createFabricServer(request: APIRequestContext): Promise<string> {
  const created = await request.post("/api/v1/servers", {
    headers: HEADERS,
    data: { name: NAME, loader: "fabric", minecraft: "1.21.4" },
  });
  expect(created.status()).toBe(202);
  const { server_id: id, job_id: job } = (await created.json()) as { server_id: string; job_id: number };
  await expect
    .poll(async () => ((await (await request.get(`/api/v1/jobs/${job}`)).json()) as { state: string }).state, { timeout: 30_000 })
    .toBe("succeeded");
  return id;
}

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  const deleted = await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(NAME)}`, { headers: HEADERS });
  expect(deleted.ok()).toBe(true);
});

// Spec P3, kế hoạch PR-19: tạo server Fabric từ catalog giả, vào trang Mod, tải lên jar, tắt, bật, gỡ.
test("tải lên, tắt, bật và gỡ mod trên server Fabric", async ({ page, request }) => {
  const id = await createFabricServer(request);
  serverId = id;

  await page.goto(`/servers/${id}`);
  await page.getByRole("link", { name: "Mod" }).click();
  await expect(page).toHaveURL(`/servers/${id}/content`);
  await expect(page.getByText("Chưa có mod nào")).toBeVisible();

  const jar = buildJarBuffer(
    "fabric.mod.json",
    JSON.stringify({ schemaVersion: 1, id: "demo-mod", version: "1.2.3", name: "Demo Mod", environment: "*" }),
  );
  await page.locator('input[type="file"]').setInputFiles({
    name: "demo-mod-1.2.3.jar",
    mimeType: "application/java-archive",
    buffer: jar,
  });

  await expect(page.getByText("Đã tải lên 1 file.")).toBeVisible();
  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("Demo Mod")).toBeVisible();
  await expect(table.getByText("1.2.3")).toBeVisible();
  await expect(table.getByText("Thêm tay")).toBeVisible();

  const toggle = table.getByRole("switch", { name: "Bật Demo Mod" });
  await expect(toggle).toBeChecked();

  await toggle.locator("..").click();
  await expect(toggle).not.toBeChecked();
  await page.reload();
  const toggleAfterReload = page.getByRole("switch", { name: "Bật Demo Mod" });
  await expect(toggleAfterReload).not.toBeChecked();

  await toggleAfterReload.locator("..").click();
  await expect(toggleAfterReload).toBeChecked();

  await page.getByRole("button", { name: "Thao tác khác cho Demo Mod" }).click();
  await page.getByRole("menuitem", { name: "Gỡ" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByText(/thùng rác/)).toBeVisible();
  await dialog.getByRole("button", { name: "Gỡ" }).click();

  await expect(page.getByText("Chưa có mod nào")).toBeVisible();
});
