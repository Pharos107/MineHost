import { ArrowRight, SquareTerminal } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import type { ConsoleLine, Server } from "../../../api/client";
import { live } from "../../../api/ws";
import { Button, Card, Terminal, type TerminalHandle, type TerminalTone } from "../../../ui";

const TONE: Record<ConsoleLine["stream"], TerminalTone> = { stdout: "normal", stderr: "danger", stdin: "primary" };
/** Chỉ để xem nhanh; muốn cuộn xa hơn thì mở trang Console. */
const KEEP_LINES = 200;

/** Vài dòng log cuối của server, chỉ đọc, kèm nút sang trang Console. */
export function ConsoleTail({ server }: { server: Server }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const terminal = useRef<TerminalHandle>(null);

  useEffect(() => {
    terminal.current?.clear();
    // Đăng ký là nhận lại bộ đệm của backend rồi tới từng dòng mới, như trang Console.
    return live.subscribe(`server.${server.id}.console`, (message) => {
      if (message.error) return;
      const line = message.data as ConsoleLine;
      terminal.current?.write(line.stream === "stdin" ? `> ${line.text}` : line.text, TONE[line.stream]);
    });
  }, [server.id]);

  return (
    <Card
      title={t("overview.recentConsole")}
      icon={SquareTerminal}
      actions={
        <Button variant="ghost" size="sm" icon={ArrowRight} onClick={() => void navigate(`/servers/${server.id}/console`)}>
          {t("overview.openConsole")}
        </Button>
      }
    >
      <Terminal ref={terminal} label={t("overview.recentConsole")} maxLines={KEEP_LINES} size="sm" />
    </Card>
  );
}
