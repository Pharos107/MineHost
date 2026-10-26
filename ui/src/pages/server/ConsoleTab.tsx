import { useMutation } from "@tanstack/react-query";
import { Eraser, Pause, Play, SendHorizontal } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type ConsoleLine, type Server } from "../../api/client";
import { live } from "../../api/ws";
import {
  Badge,
  Button,
  Notice,
  SearchInput,
  Stack,
  Terminal,
  TextInput,
  type TerminalHandle,
  type TerminalTone,
} from "../../ui";
import { appendCapped, matchesFilter, type BufferedLine } from "./consoleBuffer";
import { emptyHistory, historyDown, historyUp, pushCommand } from "./history";

const TONE: Record<ConsoleLine["stream"], TerminalTone> = { stdout: "normal", stderr: "danger", stdin: "primary" };

/**
 * Log realtime và ô gửi lệnh. Mở trang là nhận lại cả bộ đệm 5.000 dòng rồi nhận tiếp dòng mới.
 * Lọc, tạm dừng và xóa màn hình chỉ làm phía trình duyệt (spec P2, mục 6.5); log trên server giữ nguyên.
 */
export function ConsoleTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const terminal = useRef<TerminalHandle>(null);
  const lines = useRef<BufferedLine[]>([]);
  const [filter, setFilter] = useState("");
  const [paused, setPaused] = useState(false);
  const [queued, setQueued] = useState(0);
  // Handler WebSocket đọc giá trị mới nhất qua ref, để không phải đăng ký lại topic mỗi lần gõ lọc.
  const view = useRef({ filter: "", paused: false });

  useEffect(() => {
    lines.current = [];
    terminal.current?.clear();
    return live.subscribe(`server.${server.id}.console`, (message) => {
      if (message.error) return;
      const raw = message.data as ConsoleLine;
      const line = { text: raw.stream === "stdin" ? `> ${raw.text}` : raw.text, tone: TONE[raw.stream] };
      appendCapped(lines.current, line);
      if (view.current.paused) setQueued((n) => n + 1);
      else if (matchesFilter(line.text, view.current.filter)) terminal.current?.write(line.text, line.tone);
    });
  }, [server.id]);

  function redraw() {
    terminal.current?.clear();
    for (const line of lines.current) {
      if (matchesFilter(line.text, view.current.filter)) terminal.current?.write(line.text, line.tone);
    }
    setQueued(0);
  }

  function changeFilter(value: string) {
    setFilter(value);
    view.current.filter = value;
    redraw();
  }

  function togglePause() {
    const next = !paused;
    setPaused(next);
    view.current.paused = next;
    // Tiếp tục thì vẽ lại cả những dòng đến trong lúc tạm dừng.
    if (!next) redraw();
  }

  function clearScreen() {
    lines.current = [];
    redraw();
  }

  return (
    <Stack gap={3}>
      <Stack direction="row" gap={2} align="center" wrap>
        <SearchInput value={filter} onChange={changeFilter} label={t("console.filter")} />
        <Button variant="ghost" size="sm" icon={paused ? Play : Pause} aria-pressed={paused} onClick={togglePause}>
          {paused ? t("console.resume") : t("console.pause")}
        </Button>
        <Button variant="ghost" size="sm" icon={Eraser} onClick={clearScreen}>
          {t("console.clear")}
        </Button>
        {paused && queued > 0 && <Badge tone="info">{t("console.newLines", { count: queued })}</Badge>}
      </Stack>
      <Terminal ref={terminal} label={t("console.label")} />
      <CommandInput server={server} />
    </Stack>
  );
}

function CommandInput({ server }: { server: Server }) {
  const { t } = useTranslation();
  const [value, setValue] = useState("");
  const [history, setHistory] = useState(emptyHistory);
  const send = useMutation({ mutationFn: (command: string) => api.sendCommand(server.id, command) });
  // Backend chỉ nhận lệnh khi server có process; bấm Gửi lúc khác chỉ nhận lỗi SERVER_BUSY.
  const online = server.state === "running" || server.state === "starting";

  function submit() {
    const command = value.trim();
    if (command === "" || !online) return;
    send.mutate(command, {
      onSuccess: () => {
        setHistory((current) => pushCommand(current, command));
        setValue("");
      },
    });
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    const step = e.key === "ArrowUp" ? historyUp(history, value) : historyDown(history, value);
    setHistory(step.history);
    setValue(step.value);
  }

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <Stack gap={2}>
        <Stack direction="row" gap={2}>
          <TextInput
            aria-label={t("console.command")}
            placeholder={online ? t("console.placeholder") : t("console.offline")}
            value={value}
            disabled={!online}
            autoComplete="off"
            spellCheck={false}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={onKeyDown}
          />
          <Button type="submit" icon={SendHorizontal} disabled={!online || value.trim() === ""} loading={send.isPending}>
            {t("console.send")}
          </Button>
        </Stack>
        {send.isError && <Notice tone="danger">{errorMessage(t, send.error)}</Notice>}
      </Stack>
    </form>
  );
}
