import { useMutation } from "@tanstack/react-query";
import { SendHorizontal } from "lucide-react";
import { useId, useState, type KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../../api/client";
import { Button, Notice, Stack, SuggestBox, TextInput } from "../../../ui";
import { emptyHistory, historyDown, historyUp, pushCommand } from "../history";
import { matchCommands } from "./commandHints";

/**
 * Ô gửi lệnh: mũi tên lên / xuống gọi lại lệnh cũ; đang gõ tên lệnh thì có danh sách gợi ý, mũi tên
 * chọn gợi ý, Tab điền vào, Esc đóng. Enter luôn gửi đúng chữ đang có.
 */
export function CommandInput({ server }: { server: Server }) {
  const { t } = useTranslation();
  const listId = useId();
  const [value, setValue] = useState("");
  const [history, setHistory] = useState(emptyHistory);
  const [active, setActive] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const send = useMutation({ mutationFn: (command: string) => api.sendCommand(server.id, command) });
  // Backend chỉ nhận lệnh khi server có process; bấm Gửi lúc khác chỉ nhận lỗi SERVER_BUSY.
  const online = server.state === "running" || server.state === "starting";
  const matches = dismissed ? [] : matchCommands(value);
  const items = matches.map((c) => ({ value: c.name, label: t(`console.hints.${c.key}.usage`), detail: t(`console.hints.${c.key}.desc`) }));
  const current = Math.min(active, Math.max(items.length - 1, 0));

  function change(next: string) {
    setValue(next);
    setActive(0);
    setDismissed(false);
  }

  function pick(name: string) {
    change(`${name} `);
  }

  function submit() {
    const command = value.trim();
    if (command === "" || !online) return;
    send.mutate(command, {
      onSuccess: () => {
        setHistory((h) => pushCommand(h, command));
        change("");
      },
    });
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (items.length > 0 && handleSuggestionKey(e)) return;
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    const step = e.key === "ArrowUp" ? historyUp(history, value) : historyDown(history, value);
    setHistory(step.history);
    setValue(step.value);
  }

  /** Phím khi danh sách gợi ý đang mở; trả về `true` nếu đã xử lý. */
  function handleSuggestionKey(e: KeyboardEvent<HTMLInputElement>): boolean {
    const step = { ArrowDown: 1, ArrowUp: -1 }[e.key as "ArrowDown" | "ArrowUp"];
    if (step !== undefined) setActive((current + step + items.length) % items.length);
    else if (e.key === "Tab") pick(items[current]?.value ?? "");
    else if (e.key === "Escape") setDismissed(true);
    else return false;
    e.preventDefault();
    return true;
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
          <SuggestBox id={listId} label={t("console.hintsLabel")} items={items} active={current} onPick={pick}>
            <TextInput
              role="combobox"
              aria-label={t("console.command")}
              aria-autocomplete="list"
              aria-expanded={items.length > 0}
              aria-controls={items.length > 0 ? listId : undefined}
              aria-activedescendant={items.length > 0 ? `${listId}-${current}` : undefined}
              placeholder={online ? t("console.placeholder") : t("console.offline")}
              value={value}
              disabled={!online}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => change(e.target.value)}
              onKeyDown={onKeyDown}
              onBlur={() => setDismissed(true)}
            />
          </SuggestBox>
          <Button type="submit" icon={SendHorizontal} disabled={!online || value.trim() === ""} loading={send.isPending}>
            {t("console.send")}
          </Button>
        </Stack>
        {send.isError && <Notice tone="danger">{errorMessage(t, send.error)}</Notice>}
      </Stack>
    </form>
  );
}
