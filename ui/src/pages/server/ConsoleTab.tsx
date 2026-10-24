import { useMutation } from "@tanstack/react-query";
import { SendHorizontal } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type ConsoleLine, type Server } from "../../api/client";
import { live } from "../../api/ws";
import { Button, Notice, Stack, Terminal, TextInput, type TerminalHandle, type TerminalTone } from "../../ui";
import { emptyHistory, historyDown, historyUp, pushCommand } from "./history";

const TONE: Record<ConsoleLine["stream"], TerminalTone> = { stdout: "normal", stderr: "danger", stdin: "primary" };

/** Log realtime và ô gửi lệnh. Mở tab là nhận lại cả bộ đệm 5.000 dòng rồi nhận tiếp dòng mới. */
export function ConsoleTab({ server }: { server: Server }) {
  const { t } = useTranslation();
  const terminal = useRef<TerminalHandle>(null);

  useEffect(() => {
    terminal.current?.clear();
    return live.subscribe(`server.${server.id}.console`, (message) => {
      if (message.error) return;
      const line = message.data as ConsoleLine;
      terminal.current?.write(line.stream === "stdin" ? `> ${line.text}` : line.text, TONE[line.stream]);
    });
  }, [server.id]);

  return (
    <Stack gap={3}>
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
