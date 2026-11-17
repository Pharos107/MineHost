import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Property, Server } from "../../api/client";
import { GameSettingsTab } from "./GameSettingsTab";

const server: Server = {
  id: "01JB",
  name: "Survival",
  state: "running",
  eula_accepted: true,
  restart_pending: false,
  slow_start: false,
  ignored_jvm_args: false,
  preflight_blocking: false,
};

const baseProperties: Property[] = [
  { key: "server-port", value: "25565" },
  { key: "difficulty", value: "easy" },
  { key: "view-distance", value: "10" },
];

function renderTab(target: Server, initialProps: Property[] = baseProperties) {
  const calls: { url: string; method: string; body?: string }[] = [];
  const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
    const method = init?.method ?? "GET";
    calls.push({ url, method, body: init?.body ? String(init.body) : undefined });
    if (method === "PUT" || method === "POST") return new Response(null, { status: 204 });
    if (url.includes("/files")) {
      return Response.json([{ name: "server-icon.png", kind: "file", size: 100, modified: "2026-11-18T00:00:00Z" }]);
    }
    return Response.json({ properties: initialProps });
  });
  vi.stubGlobal("fetch", fetchMock);
  render(
    <QueryClientProvider client={new QueryClient()}>
      <GameSettingsTab server={target} />
    </QueryClientProvider>,
  );
  return { calls, fetchMock };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("GameSettingsTab", () => {
  it("lưu cả file theo thứ tự cũ, ẩn khóa cổng do MineHost quản lý", async () => {
    const { calls } = renderTab(server);

    expect(await screen.findByText("Có hiệu lực sau khi khởi động lại server.")).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "server-port" })).toBeNull();
    expect(screen.getByRole("button", { name: "Lưu" })).toHaveProperty("disabled", true);

    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Độ khó" }), "hard");
    expect(screen.getByText("Có 1 thay đổi chưa lưu")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Lưu" }));

    await vi.waitFor(() => expect(calls.some((c) => c.method === "PUT")).toBe(true));
    const put = calls.find((c) => c.method === "PUT");
    expect(JSON.parse(put?.body ?? "{}").properties).toEqual([
      { key: "server-port", value: "25565" },
      { key: "difficulty", value: "hard" },
      { key: "view-distance", value: "10" },
    ]);
  });

  it("chuyển true/false trong bảng Nâng cao thành công tắc, lọc nhóm và giữ giá trị select tùy chỉnh", async () => {
    const props: Property[] = [
      ...baseProperties,
      { key: "allow-nether", value: "true" },
      { key: "custom-mod-flag", value: "false" },
      { key: "level-type", value: "biomesoplenty" },
      { key: "max-tick-time", value: "60000" },
      { key: "rcon.password", value: "secret" },
    ];
    const { calls } = renderTab(server, props);

    const netherSwitch = await screen.findByRole("switch", { name: "allow-nether" });
    expect(netherSwitch).toHaveProperty("checked", true);
    expect(screen.getByRole("switch", { name: "custom-mod-flag" })).toHaveProperty("checked", false);

    // Giữ tên khóa gốc và chỉ hiện giải thích khi rê chuột vào biểu tượng chấm than.
    const table = screen.getByRole("table", { name: "Nâng cao" });
    expect(within(table).getByText("allow-nether")).toBeTruthy();
    expect(screen.queryByRole("tooltip")).toBeNull();
    await userEvent.hover(screen.getByRole("button", { name: "Giải thích cho allow-nether" }));
    expect(screen.getByRole("tooltip").textContent).toContain("Nether");

    // Giữ giá trị modpack tùy chỉnh trong ô select của level-type.
    const levelType = screen.getByRole("combobox", { name: "level-type" }) as HTMLSelectElement;
    expect(levelType.value).toBe("biomesoplenty");

    // Khóa phổ biến (difficulty, view-distance) không bị lặp lại trong bảng Nâng cao.
    expect(within(table).queryByText("view-distance")).toBeNull();

    // Bấm công tắc trong bảng Nâng cao và lọc theo "Đã sửa".
    await userEvent.click(netherSwitch);
    expect(netherSwitch).toHaveProperty("checked", false);
    expect(screen.getByRole("radio", { name: "Đã sửa (1)" })).toBeTruthy();
    await userEvent.click(screen.getByRole("radio", { name: "Đã sửa (1)" }));
    expect(within(table).getByText("allow-nether")).toBeTruthy();
    expect(within(table).queryByText("max-tick-time")).toBeNull();

    // Lọc sang nhóm không có mục nào vẫn giữ thanh lọc để quay lại.
    await userEvent.click(screen.getByRole("radio", { name: "Mạng & Bảo mật" }));
    expect(screen.getByText("Không có khóa nào trong nhóm này.")).toBeTruthy();
    await userEvent.click(screen.getByRole("radio", { name: "Tất cả" }));
    expect(within(screen.getByRole("table", { name: "Nâng cao" })).getByText("max-tick-time")).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Lưu" }));
    await vi.waitFor(() => expect(calls.some((c) => c.method === "PUT")).toBe(true));
    const saved = JSON.parse(calls.find((c) => c.method === "PUT")?.body ?? "{}").properties as Property[];
    expect(saved.find((p) => p.key === "allow-nether")?.value).toBe("false");
  });

  it("thêm khóa mới, hoàn tác từng dòng và xem trước MOTD có mã màu", async () => {
    const props: Property[] = [
      { key: "motd", value: "§6§l✦ §e§lPROMINENCE II" },
      { key: "max-players", value: "150" },
      { key: "allow-nether", value: "false" },
    ];
    const { calls } = renderTab({ ...server, state: "stopped" }, props);

    const preview = await screen.findByLabelText("Xem trước lời chào");
    expect(preview.textContent).toContain("Survival");
    expect(preview.textContent).toContain("0/150");
    expect(preview.textContent).toContain("PROMINENCE II");
    expect(preview.textContent).not.toContain("§6");
    await vi.waitFor(() => {
      const icon = preview.querySelector("img");
      expect(icon?.getAttribute("src")).toContain("server-icon.png");
    });

    // Nút Lưu & khởi động lại không hiện khi server đang tắt.
    expect(screen.queryByRole("button", { name: "Lưu & khởi động lại" })).toBeNull();

    // Thêm khóa mới qua hộp thoại.
    await userEvent.click(screen.getByRole("button", { name: "Thêm khóa" }));
    const dialog = screen.getByRole("dialog", { name: "Thêm khóa vào server.properties" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Khóa" }), "sync-chunk-writes");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Giá trị" }), "true");
    await userEvent.click(within(dialog).getByRole("button", { name: "Thêm khóa" }));

    expect(await screen.findByRole("switch", { name: "sync-chunk-writes" })).toHaveProperty("checked", true);

    // Hoàn tác chính khóa vừa thêm sẽ gỡ nó khỏi draft.
    await userEvent.click(screen.getByRole("button", { name: "Hoàn tác khóa này: sync-chunk-writes" }));
    expect(screen.queryByRole("switch", { name: "sync-chunk-writes" })).toBeNull();

    // Reset allow-nether về mặc định của Minecraft (true).
    await userEvent.click(screen.getByRole("button", { name: "Về mặc định của Minecraft: allow-nether" }));
    expect(screen.getByRole("switch", { name: "allow-nether" })).toHaveProperty("checked", true);

    await userEvent.click(screen.getByRole("button", { name: "Lưu" }));
    await vi.waitFor(() => expect(calls.some((c) => c.method === "PUT")).toBe(true));
  });

  it("cảnh báo ngữ cảnh cho modpack chưa bật allow-flight, offline-mode và hỗ trợ Lưu & khởi động lại", async () => {
    const modServer: Server = { ...server, loader: "fabric", content_kind: "mods" };
    const props: Property[] = [
      { key: "allow-flight", value: "false" },
      { key: "online-mode", value: "false" },
      { key: "white-list", value: "false" },
    ];
    const { calls } = renderTab(modServer, props);

    expect(await screen.findByText(/Server đang chạy mod\/modpack nhưng chưa bật Cho phép bay/)).toBeTruthy();
    expect(screen.getByText(/Server đang tắt kiểm tra tài khoản Minecraft/)).toBeTruthy();

    await userEvent.click(screen.getByRole("button", { name: "Bật ngay" }));
    expect(screen.queryByText(/Server đang chạy mod\/modpack nhưng chưa bật Cho phép bay/)).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Lưu & khởi động lại" }));
    await vi.waitFor(() => expect(calls.some((c) => c.url.endsWith("/actions/restart"))).toBe(true));
  });

  it("giữ giá trị rỗng/tùy chỉnh cho Select, cho phép nhập âm ở max-tick-time, reset bộ lọc khi thêm khóa và xử lý lỗi khi restart", async () => {
    let failRestart = true;
    const props: Property[] = [
      { key: "motd", value: "Test" },
      { key: "level-type", value: "" },
      { key: "max-tick-time", value: "60000" },
      { key: "max-world-size", value: "50000000" },
    ];
    let currentProps = [...props];
    const fetchMock = vi.fn(async (url: string, init?: RequestInit) => {
      const method = init?.method ?? "GET";
      if (method === "PUT") {
        currentProps = JSON.parse(String(init?.body ?? "{}")).properties as Property[];
        return new Response(null, { status: 204 });
      }
      if (method === "POST" && url.endsWith("/actions/restart") && failRestart) {
        return Response.json({ code: "CONFLICT", params: {} }, { status: 409 });
      }
      if (url.includes("/files")) return Response.json([]);
      return Response.json({ properties: currentProps });
    });
    vi.stubGlobal("fetch", fetchMock);
    render(
      <QueryClientProvider client={new QueryClient()}>
        <GameSettingsTab server={server} />
      </QueryClientProvider>,
    );

    const levelType = (await screen.findByRole("combobox", { name: "level-type" })) as HTMLSelectElement;
    expect(levelType.value).toBe("");
    expect(screen.getByRole("spinbutton", { name: "max-tick-time" }).getAttribute("inputmode")).toBeNull();
    expect(screen.getByRole("spinbutton", { name: "max-world-size" }).getAttribute("inputmode")).toBe("numeric");

    // Bấm chuột vào InfoTip sau khi hover không làm tooltip bị tắt ngược.
    const tipButton = screen.getByRole("button", { name: "Giải thích cho level-type" });
    await userEvent.hover(tipButton);
    await userEvent.click(tipButton);
    expect(screen.getByRole("tooltip")).toBeTruthy();

    // Thêm khóa mới khi đang lọc nhóm khác -> tự quay về "Tất cả" để hiện khóa vừa thêm.
    await userEvent.click(screen.getByRole("radio", { name: "Mạng & Bảo mật" }));
    await userEvent.click(screen.getByRole("button", { name: "Thêm khóa" }));
    const dialog = screen.getByRole("dialog", { name: "Thêm khóa vào server.properties" });
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Khóa" }), "allow-nether");
    await userEvent.type(within(dialog).getByRole("textbox", { name: "Giá trị" }), "true");
    await userEvent.click(within(dialog).getByRole("button", { name: "Thêm khóa" }));
    expect(screen.getByRole("radio", { name: "Tất cả" }).getAttribute("aria-checked")).toBe("true");
    expect(screen.getByRole("switch", { name: "allow-nether" })).toBeTruthy();

    // Lưu & khởi động lại: giá trị max-world-size ngoài ngưỡng vẫn không chặn lưu; nếu restart lỗi thì form vẫn cập nhật bản đã lưu.
    await userEvent.click(screen.getByRole("button", { name: "Lưu & khởi động lại" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Lưu" })).toHaveProperty("disabled", true);
    failRestart = false;
  });
});
