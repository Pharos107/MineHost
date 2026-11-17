import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useImperativeHandle, useState, type Ref } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Server } from "../../api/client";
import type { TerminalHandle } from "../../ui";
import { ConsoleTab } from "./ConsoleTab";

type Handler = (message: { data: unknown; error?: unknown }) => void;
const handlers: Handler[] = [];

vi.mock("../../api/ws", () => ({
  live: {
    subscribe: (_topic: string, handler: Handler) => {
      handlers.push(handler);
      return () => {};
    },
    getStatus: () => "open",
    onStatus: () => () => {},
  },
}));

// xterm cần canvas mà jsdom không có: thay bằng danh sách dòng để đọc lại trong test.
vi.mock("../../ui/Terminal", () => ({
  Terminal: function FakeTerminal({ label, ref }: { label: string; ref?: Ref<TerminalHandle> }) {
    const [shown, setShown] = useState<string[]>([]);
    useImperativeHandle(ref, () => ({
      write: (text: string) => setShown((s) => [...s, text]),
      clear: () => setShown([]),
    }));
    return (
      <ul aria-label={label}>
        {shown.map((text, i) => (
          <li key={i}>{text}</li>
        ))}
      </ul>
    );
  },
}));

const server: Server = { id: "01JB", name: "Survival", state: "running", eula_accepted: true, restart_pending: false, slow_start: false, ignored_jvm_args: false, preflight_blocking: false };

function emit(text: string) {
  act(() => handlers.at(-1)?.({ data: { seq: 1, stream: "stdout", text } }));
}

function shownLines(): string[] {
  return screen.getAllByRole("listitem").map((li) => li.textContent ?? "");
}

function renderConsole() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <ConsoleTab server={server} />
    </QueryClientProvider>,
  );
  emit("[INFO]: Steve joined the game");
  emit("[INFO]: Done (3.1s)!");
  emit("[INFO]: Alex joined the game");
}

/** Thay `fetch`, trả về danh sách lệnh đã gửi tới `/console`. */
function mockCommands() {
  const sent: string[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init?: RequestInit) => {
      if (url.endsWith("/console") && init?.body) sent.push((JSON.parse(String(init.body)) as { command: string }).command);
      return new Response(null, { status: 204 });
    }),
  );
  return sent;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("ConsoleTab", () => {
  it("lọc chỉ còn dòng khớp, xóa ô lọc thì hiện lại đủ", async () => {
    renderConsole();
    await userEvent.type(screen.getByRole("searchbox", { name: "Lọc dòng" }), "joined");
    expect(shownLines()).toEqual(["[INFO]: Steve joined the game", "[INFO]: Alex joined the game"]);
    emit("[INFO]: Nam joined the game");
    emit("[WARN]: Can't keep up!");
    expect(shownLines()).toHaveLength(3);
    await userEvent.clear(screen.getByRole("searchbox", { name: "Lọc dòng" }));
    expect(shownLines()).toHaveLength(5);
  });

  it("tạm dừng thì giữ màn hình và đếm dòng mới, tiếp tục thì vẽ ra", async () => {
    renderConsole();
    await userEvent.click(screen.getByRole("button", { name: "Tạm dừng" }));
    emit("[INFO]: Nam joined the game");
    emit("[INFO]: Nam left the game");
    expect(shownLines()).toHaveLength(3);
    expect(screen.getByText("2 dòng mới")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Tiếp tục" }));
    expect(shownLines()).toHaveLength(5);
    expect(screen.queryByText("2 dòng mới")).toBeNull();
  });

  it("xóa màn hình chỉ xóa phía trình duyệt, dòng sau đó vẫn hiện", async () => {
    renderConsole();
    await userEvent.click(screen.getByRole("button", { name: "Xóa màn hình" }));
    expect(screen.queryAllByRole("listitem")).toHaveLength(0);
    emit("[INFO]: Steve left the game");
    expect(shownLines()).toEqual(["[INFO]: Steve left the game"]);
  });

  it("nút lệnh nhanh gửi lệnh, Thông báo gửi say kèm lời nhắn; có trạng thái kết nối", async () => {
    const sent = mockCommands();
    renderConsole();
    expect(screen.getByRole("status").textContent).toBe("Đã kết nối");
    await userEvent.click(screen.getByRole("button", { name: "Lưu thế giới" }));
    await vi.waitFor(() => expect(sent).toEqual(["save-all"]));

    await userEvent.click(screen.getByRole("button", { name: "Thông báo…" }));
    await userEvent.type(screen.getByRole("textbox", { name: "Lời nhắn" }), "Bảo trì lúc 22h");
    await userEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Gửi" }));
    await vi.waitFor(() => expect(sent).toEqual(["save-all", "say Bảo trì lúc 22h"]));
  });

  it("lọc theo mức: Lỗi chỉ còn dòng ERROR", async () => {
    renderConsole();
    emit("[10:20:11 ERROR]: boom");
    await userEvent.click(screen.getByRole("radio", { name: "Lỗi" }));
    expect(shownLines()).toEqual(["[10:20:11 ERROR]: boom"]);
    await userEvent.click(screen.getByRole("radio", { name: "Tất cả" }));
    expect(shownLines()).toHaveLength(4);
  });

  it("gõ / giữ nguyên dấu /, bấm Tab điền liên hoàn và hiện thanh cú pháp", async () => {
    mockCommands();
    renderConsole();
    const input = screen.getByRole("combobox", { name: "Lệnh" });
    await userEvent.type(input, "/gamem");
    expect(screen.getByRole("option").textContent).toContain("Đổi chế độ chơi");
    await userEvent.keyboard("{Tab}");
    expect((input as HTMLInputElement).value).toBe("/gamemode ");
    // Sau khi chọn lệnh vẫn mở tiếp gợi ý cho tham số chế độ chơi và hiện thanh cú pháp.
    const options = screen.getAllByRole("option").map((o) => o.textContent ?? "");
    expect(options.some((t) => t.includes("survival"))).toBe(true);
    expect(options.some((t) => t.includes("creative"))).toBe(true);
    expect(screen.getByText("/gamemode")).toBeTruthy();

    await userEvent.type(input, "c");
    await userEvent.keyboard("{Tab}");
    expect((input as HTMLInputElement).value).toBe("/gamemode creative ");
    expect(screen.getAllByRole("option").some((o) => (o.textContent ?? "").includes("@a"))).toBe(true);

    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("gõ /eff bấm Tab ra /effect và mở gợi ý give / clear", async () => {
    mockCommands();
    renderConsole();
    const input = screen.getByRole("combobox", { name: "Lệnh" });
    await userEvent.type(input, "/eff");
    await userEvent.keyboard("{Tab}");
    expect((input as HTMLInputElement).value).toBe("/effect ");
    const options = screen.getAllByRole("option").map((o) => o.textContent ?? "");
    expect(options.some((t) => t.includes("give"))).toBe(true);
    expect(options.some((t) => t.includes("clear"))).toBe(true);
  });

  it("lấy lệnh của server rồi gợi ý cả lệnh của mod", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url.endsWith("/commands")) return Response.json({ scanned_at: null, commands: [] });
        if (url.endsWith("/commands/scan")) {
          return Response.json({
            scanned_at: "2026-11-16T05:34:47Z",
            commands: [{ name: "ftbquests", usage: "ftbquests open_book [<target>]", description: null }],
          });
        }
        return new Response(null, { status: 204 });
      }),
    );
    renderConsole();
    await userEvent.click(await screen.findByRole("button", { name: /^Lấy lệnh của server/ }));
    expect(await screen.findByRole("button", { name: /^Quét lại lệnh của server \(1 lệnh, quét lúc/ })).toBeTruthy();
    await userEvent.type(screen.getByRole("combobox", { name: "Lệnh" }), "ftb");
    const option = screen.getByRole("option");
    expect(option.textContent).toContain("ftbquests open_book [<target>]");
    expect(option.textContent).toContain("Lệnh của server");
  });

  it("gõ đối số thì gợi ý tên người chơi, Tab điền tên", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url.endsWith("/players")) {
          const player = { name: "Pharos_107", online: true, op: true, banned: false, frozen: false, whitelisted: true };
          return Response.json({ max_players: 20, online_mode: false, running: true, whitelist_enabled: true, players: [player] });
        }
        return new Response(null, { status: 204 });
      }),
    );
    renderConsole();
    const input = screen.getByRole("combobox", { name: "Lệnh" });
    await userEvent.type(input, "tp pha");
    expect((await screen.findByRole("option")).textContent).toContain("Đang chơi");
    await userEvent.keyboard("{Tab}");
    expect((input as HTMLInputElement).value).toBe("tp Pharos_107 ");
  });

  it("gợi ý hiệu ứng bằng tiếng Việt và vật phẩm quét từ mod", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (url: string) => {
        if (url.endsWith("/commands/registry")) {
          return Response.json({
            scanned_at: "2026-11-17T00:00:00Z",
            items: [{ id: "create:brass_ingot", label: "Brass Ingot" }],
            effects: [],
            entities: [],
            enchantments: [],
          });
        }
        return new Response(null, { status: 204 });
      }),
    );
    renderConsole();
    const input = screen.getByRole("combobox", { name: "Lệnh" });

    await userEvent.type(input, "/effect give Steve hoi phuc");
    const regenOption = await screen.findByRole("option", { name: /minecraft:regeneration/ });
    expect(regenOption.textContent).toContain("Hồi phục");
    await userEvent.keyboard("{Tab}");
    expect((input as HTMLInputElement).value).toBe("/effect give Steve minecraft:regeneration ");

    await userEvent.clear(input);
    await userEvent.type(input, "/give Steve brass");
    const modOption = await screen.findByRole("option", { name: /create:brass_ingot/ });
    expect(modOption.textContent).toContain("Brass Ingot");
  });
});
