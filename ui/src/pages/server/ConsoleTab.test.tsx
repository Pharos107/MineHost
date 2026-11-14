import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useImperativeHandle, useState, type Ref } from "react";
import { describe, expect, it, vi } from "vitest";

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
});
