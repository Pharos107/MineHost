import { Download, Eraser, Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type { ConsoleLine, Server } from "../../api/client";
import { live } from "../../api/ws";
import { Badge, Button, SearchInput, SegmentedControl, Stack, Terminal, type TerminalHandle } from "../../ui";
import { serverName } from "../ServerListPage";
import { appendCapped, lineTone, matchesFilter, matchesLevel, type BufferedLine, type LevelFilter } from "./consoleBuffer";
import { CommandInput } from "./console/CommandInput";
import { ConnectionStatus } from "./console/ConnectionStatus";
import { downloadLog } from "./console/downloadLog";
import { QuickCommands } from "./console/QuickCommands";

interface View {
  filter: string;
  level: LevelFilter;
  paused: boolean;
}

const shows = (line: BufferedLine, view: View) => matchesFilter(line.text, view.filter) && matchesLevel(line.text, view.level);

/**
 * Log realtime và ô gửi lệnh. Mở trang là nhận lại cả bộ đệm 5.000 dòng rồi nhận tiếp dòng mới.
 * Lọc, tạm dừng, xóa màn hình và tải log chỉ làm phía trình duyệt (spec P2, mục 6.5); log trên server giữ nguyên.
 */
export function ConsoleTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const terminal = useRef<TerminalHandle>(null);
  const lines = useRef<BufferedLine[]>([]);
  const [view, setView] = useState<View>({ filter: "", level: "all", paused: false });
  const [queued, setQueued] = useState(0);
  // Handler WebSocket đọc giá trị mới nhất qua ref, để không phải đăng ký lại topic mỗi lần gõ lọc.
  const latest = useRef(view);

  useEffect(() => {
    lines.current = [];
    terminal.current?.clear();
    return live.subscribe(`server.${server.id}.console`, (message) => {
      if (message.error) return;
      const raw = message.data as ConsoleLine;
      const line = { text: raw.stream === "stdin" ? `> ${raw.text}` : raw.text, tone: lineTone(raw.stream, raw.text) };
      appendCapped(lines.current, line);
      if (latest.current.paused) setQueued((n) => n + 1);
      else if (shows(line, latest.current)) terminal.current?.write(line.text, line.tone);
    });
  }, [server.id]);

  function update(patch: Partial<View>) {
    const next = { ...latest.current, ...patch };
    latest.current = next;
    setView(next);
    // Tiếp tục sau khi tạm dừng cũng vẽ lại cả những dòng đến trong lúc đó.
    if (next.paused) return;
    terminal.current?.clear();
    for (const line of lines.current) if (shows(line, next)) terminal.current?.write(line.text, line.tone);
    setQueued(0);
  }

  function clearScreen() {
    lines.current = [];
    update({});
  }

  return (
    <Stack gap={3} grow>
      <Stack direction="row" gap={2} align="center" justify="between" wrap>
        <Stack direction="row" gap={2} align="center" wrap>
          <SearchInput value={view.filter} onChange={(filter) => update({ filter })} label={t("console.filter")} />
          <SegmentedControl
            label={t("console.level.label")}
            value={view.level}
            onChange={(level) => update({ level })}
            options={(["all", "warn", "error", "chat"] as const).map((value) => ({ value, label: t(`console.level.${value}`) }))}
          />
          <Button variant="ghost" size="sm" icon={view.paused ? Play : Pause} aria-pressed={view.paused} onClick={() => update({ paused: !view.paused })}>
            {view.paused ? t("console.resume") : t("console.pause")}
          </Button>
          <Button variant="ghost" size="sm" icon={Eraser} onClick={clearScreen}>
            {t("console.clear")}
          </Button>
          <Button variant="ghost" size="sm" icon={Download} onClick={() => downloadLog(serverName(server), lines.current.map((l) => l.text))}>
            {t("console.download")}
          </Button>
          {view.paused && queued > 0 && <Badge tone="info">{t("console.newLines", { count: queued })}</Badge>}
        </Stack>
        <ConnectionStatus />
      </Stack>
      <Terminal ref={terminal} label={t("console.label")} size="fill" />
      <QuickCommands server={server} />
      <CommandInput server={server} />
    </Stack>
  );
}
