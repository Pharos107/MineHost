import { expect, test, type APIRequestContext } from "@playwright/test";

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/** Đóng gói file zip không nén để tải trực tiếp một `.jar` hợp lệ trong test. */
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

async function createFabricServer(request: APIRequestContext, name: string): Promise<string> {
  const created = await request.post("/api/v1/servers", {
    headers: { "X-MineHost": "1" },
    data: { name, loader: "fabric", minecraft: "1.21.4", port: 25575 },
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

const NAME = "Preflight e2e";
let serverId: string | null = null;

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  await request.post(`/api/v1/servers/${serverId}/actions/kill`, { headers: { "X-MineHost": "1" } }).catch(() => null);
  await expect
    .poll(
      async () => ((await (await request.get(`/api/v1/servers/${serverId}`)).json()) as { state: string }).state,
      { timeout: 10_000 },
    )
    .toBe("stopped");
  const deleted = await request.delete(`/api/v1/servers/${serverId}?confirm_name=${encodeURIComponent(NAME)}`, {
    headers: { "X-MineHost": "1" },
  });
  expect(deleted.ok()).toBe(true);
  serverId = null;
});

// Kế hoạch P3 PR-21: mod thiếu dependency chặn bật -> bấm Bật thấy lỗi -> bấm "Vẫn bật" -> Đang chạy.
test("preflight chặn bật khi thiếu dependency và Vẫn bật bỏ qua được", async ({ page, request }) => {
  const id = await createFabricServer(request, NAME);
  serverId = id;

  const jar = buildStoredZip(
    "fabric.mod.json",
    JSON.stringify({
      schemaVersion: 1,
      id: "broken-demo",
      name: "Broken Demo",
      version: "1.0.0",
      depends: { "absent-lib": ">=1.2.0" },
    }),
  );
  const upload = await request.post(`/api/v1/servers/${id}/content/upload`, {
    headers: { "X-MineHost": "1" },
    multipart: {
      file: {
        name: "broken-demo.jar",
        mimeType: "application/java-archive",
        buffer: jar,
      },
    },
  });
  expect(upload.ok()).toBe(true);

  await page.goto(`/servers/${id}`);
  await expect(page.getByText(/Thiếu absent-lib >=1\.2\.0 \(do Broken Demo yêu cầu\)/)).toBeVisible({
    timeout: 15_000,
  });

  await page.getByRole("button", { name: "Bật server" }).click();
  await expect(page.getByText(/Còn thiếu mod bắt buộc: absent-lib/)).toBeVisible({ timeout: 10_000 });

  await page.getByRole("button", { name: "Vẫn bật" }).click();
  await expect(page.getByText("Đang chạy", { exact: true })).toBeVisible({ timeout: 30_000 });
});
