import { expect, test } from "@playwright/test";

const HEADERS = { "X-MineHost": "1" };
const createdServerIds: string[] = [];

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

/** Tạo file `.mrpack` (ZIP không nén) gồm `modrinth.index.json` và một file override. */
function buildMrpackBuffer(entries: Array<{ name: string; content: string | Buffer }>): Buffer {
  const localParts: Buffer[] = [];
  const cdParts: Buffer[] = [];
  let offset = 0;

  for (const entry of entries) {
    const name = Buffer.from(entry.name, "utf8");
    const body = typeof entry.content === "string" ? Buffer.from(entry.content, "utf8") : entry.content;
    const crc = crc32(body);

    const local = Buffer.alloc(30 + name.length);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(body.length, 22);
    local.writeUInt16LE(name.length, 26);
    name.copy(local, 30);

    const cd = Buffer.alloc(46 + name.length);
    cd.writeUInt32LE(0x02014b50, 0);
    cd.writeUInt16LE(20, 4);
    cd.writeUInt16LE(20, 6);
    cd.writeUInt32LE(crc, 16);
    cd.writeUInt32LE(body.length, 20);
    cd.writeUInt32LE(body.length, 24);
    cd.writeUInt16LE(name.length, 28);
    name.copy(cd, 46);
    cd.writeUInt32LE(offset, 42);

    localParts.push(local, body);
    cdParts.push(cd);
    offset += local.length + body.length;
  }

  const cdBuffer = Buffer.concat(cdParts);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(entries.length, 8);
  eocd.writeUInt16LE(entries.length, 10);
  eocd.writeUInt32LE(cdBuffer.length, 12);
  eocd.writeUInt32LE(offset, 16);

  return Buffer.concat([...localParts, cdBuffer, eocd]);
}

test.afterEach(async ({ request }) => {
  while (createdServerIds.length > 0) {
    const id = createdServerIds.pop();
    if (!id) continue;
    const res = await request.get(`/api/v1/servers/${id}`);
    if (!res.ok()) continue;
    const srv = (await res.json()) as { name?: string };
    if (srv.name) {
      await request.delete(`/api/v1/servers/${id}?confirm_name=${encodeURIComponent(srv.name)}`, { headers: HEADERS });
    }
  }
});

// Spec P3, kế hoạch PR-24: cài modpack từ nguồn giả tới trang server và thấy tóm tắt.
test("cài modpack từ nguồn giả tới trang server và thấy tóm tắt", async ({ page }) => {
  await page.goto("/new");
  const modpackRadio = page.getByRole("radio", { name: /^Từ modpack/ });
  await expect(modpackRadio).toBeEnabled();
  await modpackRadio.click();
  await page.getByRole("button", { name: "Tiếp" }).click();

  await expect(page).toHaveURL("/new/modpack");
  await expect(page.getByRole("radio", { name: "CurseForge" })).toBeDisabled();
  await expect(page.getByText("Modpack CurseForge sắp có.")).toBeVisible();

  await page.getByRole("searchbox", { name: "Tìm modpack trên Modrinth…" }).fill("E2E");
  await page.getByRole("button", { name: /E2E Fabric Pack/ }).click();

  await expect(page.getByRole("combobox", { name: "Phiên bản modpack" })).toHaveValue("e2e-fabric-pack-1.0.0");
  await page.getByRole("textbox", { name: "Tên server" }).fill("Modpack E2E");
  await page.getByRole("button", { name: "Tạo server" }).click();

  await expect(page.getByText("Đã cài xong modpack")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText("mods/iris-1.8.0.jar")).toBeVisible();

  await page.getByRole("button", { name: "Mở trang server" }).click();
  await expect(page).toHaveURL(/\/servers\/[0-9A-HJKMNP-TV-Z]+$/);
  const id = page.url().split("/").pop() ?? "";
  createdServerIds.push(id);
  await expect(page.getByRole("heading", { level: 1, name: "Modpack E2E" })).toBeVisible();

  await page.getByRole("link", { name: "Mod" }).click();
  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("Fabric API")).toBeVisible();
  await expect(table.getByText("Lithium")).toBeVisible();
});

test("cài modpack từ file .mrpack kéo thả", async ({ page }) => {
  // Hash của jar fabric-api giả trong chế độ e2e; giữ khớp với test `fake_fabric_api_jar_bytes_are_stable`
  // (crates/mh-app/src/e2e/pack.rs). Jar giả được dựng tất định nên hash giống nhau trên mọi máy.
  const indexJson = JSON.stringify({
    formatVersion: 1,
    game: "minecraft",
    versionId: "1.0.0",
    name: "Drop E2E Pack",
    dependencies: {
      minecraft: "1.21.4",
      "fabric-loader": "0.16.9",
    },
    files: [
      {
        path: "mods/fabric-api-0.110.0.jar",
        hashes: {
          sha1: "f59ecf18223ba86ae1e5407a5d3152a55ade48db",
          sha512:
            "88b8d16393f6645ca4316c533f2a35995052fa4dc6e64a59e9a0b1490c6584cea1a4a2a107bdd14e597420e0b2420eec35a897d4d5b82d5c86786a936a7737ad",
        },
        downloads: [
          "https://cdn.modrinth.com/data/e2e-fabric-api/versions/e2e-fabric-api-0.110.0/fabric-api-0.110.0.jar",
        ],
        fileSize: 223,
      },
    ],
  });

  const mrpackBuffer = buildMrpackBuffer([
    { name: "modrinth.index.json", content: indexJson },
    { name: "overrides/config/drop-pack.txt", content: "ok\n" },
  ]);

  await page.goto("/new/modpack");
  await page.locator('input[type="file"]').setInputFiles({
    name: "drop-e2e-pack.mrpack",
    mimeType: "application/zip",
    buffer: mrpackBuffer,
  });

  await expect(page.getByText("drop-e2e-pack.mrpack")).toBeVisible();
  await page.getByRole("textbox", { name: "Tên server" }).fill("Drop Pack E2E");
  await page.getByRole("button", { name: "Tạo server" }).click();

  await expect(page.getByText("Đã cài xong modpack")).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "Mở trang server" }).click();
  await expect(page).toHaveURL(/\/servers\/[0-9A-HJKMNP-TV-Z]+$/);
  const id = page.url().split("/").pop() ?? "";
  createdServerIds.push(id);
  await expect(page.getByRole("heading", { level: 1, name: "Drop Pack E2E" })).toBeVisible();

  await page.getByRole("link", { name: "Mod" }).click();
  const table = page.getByRole("table", { name: "Danh sách mod" });
  await expect(table.getByText("Fabric API")).toBeVisible();
});
