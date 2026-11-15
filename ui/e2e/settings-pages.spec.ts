import { expect, test } from "@playwright/test";

import { createServer, getServer, removeServer } from "./helpers";

let serverId: string | null = null;

test.afterEach(async ({ request }) => {
  if (serverId === null) return;
  const id = serverId;
  serverId = null;
  await removeServer(request, id);
});

// Spec P2, mục 6.9: Cài đặt game sửa `server.properties`, có mô tả ngắn cho khóa phổ biến.
test("cài đặt game lưu vào server.properties", async ({ page, request }) => {
  const id = await createServer(request, "Cài đặt game e2e");
  serverId = id;
  await page.goto(`/servers/${id}`);
  await page.getByRole("link", { name: "Cài đặt game" }).click();

  await expect(page.getByText("Hiện dưới tên server trong danh sách server của Minecraft.")).toBeVisible();
  await page.getByRole("textbox", { name: "Lời chào" }).fill("Chào mừng tới server e2e");
  await page.getByRole("combobox", { name: "Độ khó" }).selectOption("hard");
  const pvp = page.getByRole("switch", { name: /Cho người chơi đánh nhau/ });
  await expect(pvp).toBeChecked();
  // Ô checkbox thật bị ẩn sau phần vẽ công tắc; bấm vào nhãn như người dùng.
  await pvp.locator("..").click();
  await expect(pvp).not.toBeChecked();
  await page.getByRole("button", { name: "Lưu" }).click();
  await expect(page.getByText("Đã lưu.")).toBeVisible();

  await page.reload();
  await expect(page.getByRole("textbox", { name: "Lời chào" })).toHaveValue("Chào mừng tới server e2e");
  await expect(page.getByRole("combobox", { name: "Độ khó" })).toHaveValue("hard");
  await expect(page.getByRole("switch", { name: /Cho người chơi đánh nhau/ })).not.toBeChecked();

  // Giá trị nằm thật trong file, không chỉ trong form.
  const file = await request.get(`/api/v1/servers/${id}/files/content?path=server.properties`);
  expect(file.ok()).toBe(true);
  const text = ((await file.json()) as { text: string }).text;
  expect(text).toMatch(/^difficulty=hard$/m);
  expect(text).toMatch(/^pvp=false$/m);
});

// Spec P2, mục 6.9: Cài đặt server, gồm giờ khởi động lại hằng ngày thêm ở P2.
test("cài đặt server lưu tên, cổng và giờ khởi động lại", async ({ page, request }) => {
  const id = await createServer(request, "Cài đặt server e2e");
  serverId = id;
  await page.goto(`/servers/${id}`);
  await page.getByRole("link", { name: "Cài đặt server" }).click();

  await page.getByRole("textbox", { name: "Tên server" }).fill("Server đã đổi tên");
  await page.getByRole("spinbutton", { name: "Cổng" }).fill("25731");
  await expect(page.getByText("Chưa đặt giờ nào.")).toBeVisible();
  await page.getByRole("button", { name: "Thêm giờ" }).click();
  await page.getByLabel("Giờ khởi động lại 1").fill("05:30");
  await page.getByRole("button", { name: "Lưu" }).click();
  await expect(page.getByText("Đã lưu.")).toBeVisible();
  await expect(page.getByRole("heading", { level: 1, name: "Server đã đổi tên" })).toBeVisible();

  await page.reload();
  await expect(page.getByRole("textbox", { name: "Tên server" })).toHaveValue("Server đã đổi tên");
  await expect(page.getByRole("spinbutton", { name: "Cổng" })).toHaveValue("25731");
  await expect(page.getByLabel("Giờ khởi động lại 1")).toHaveValue("05:30");

  const saved = await getServer(request, id);
  expect(saved.port).toBe(25731);
});
