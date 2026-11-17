import { useMutation } from "@tanstack/react-query";
import { SendHorizontal } from "lucide-react";
import { useId, useMemo, useState, type KeyboardEvent } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../../api/client";
import type { Player } from "../../../api/players";
import { Button, CommandSyntax, Hint, Notice, Stack, SuggestBox, TextInput, type Suggestion } from "../../../ui";
import { emptyHistory, historyDown, historyUp, pushCommand } from "../history";
import { usePlayers } from "../players/usePlayers";
import { analyzeConsole, type ConsoleHint, type ConsoleSyntax } from "./commandHints";
import { buildRegistryIndex } from "./registrySearch";
import { useServerCommands, useServerRegistry } from "./ScanCommands";

function hintItem(t: TFunction, hint: ConsoleHint): Suggestion {
  if (hint.group === "command") {
    const c = hint.command;
    if (c.key) {
      return {
        value: c.name,
        label: t(`console.hints.${c.key}.usage`),
        detail: t(`console.hints.${c.key}.desc`),
        tag: t("console.tags.command"),
      };
    }
    return {
      value: c.name,
      label: c.usage ?? c.name,
      detail: c.description ?? t("console.scan.fromServer"),
      tag: t("console.tags.server"),
    };
  }
  if (hint.group === "player") {
    const detail = hint.kind === "selector" ? t(`console.selector.${hint.value.slice(1)}`) : t(`console.player.${hint.kind}`);
    const tag = hint.kind === "selector" ? t("console.tags.selector") : t("console.tags.player");
    return { value: hint.value, label: hint.value, detail, tag };
  }
  if (hint.group === "registry") {
    return {
      value: hint.id,
      label: hint.id,
      detail: hint.label || undefined,
      tag: hint.isMod ? t("console.tags.server") : t(`console.tags.${hint.kind}`),
    };
  }
  return {
    value: hint.value,
    label: hint.value,
    detail: hint.detailKey ? t(`console.args.${hint.detailKey}`) : undefined,
    tag: t(`console.tags.${hint.kind}`),
  };
}

function renderSyntax(t: TFunction, syntax: ConsoleSyntax | null) {
  if (!syntax) return undefined;
  const parts = syntax.nodes.map((node) => ({
    text: node.paramKey ? t(`console.params.${node.paramKey}`) : node.param,
    active: node.active,
    optional: node.optional,
  }));
  const description = syntax.descriptionKey ? t(syntax.descriptionKey) : syntax.descriptionText;
  return <CommandSyntax command={syntax.command} parts={parts} description={description} />;
}

/**
 * Ô gửi lệnh: mũi tên lên / xuống duyệt gợi ý khi đang mở hoặc gọi lại lịch sử khi đóng;
 * Tab điền gợi ý (giữ nguyên `/`) và mở tiếp gợi ý cho đối số kế tiếp; Esc đóng danh sách; Enter luôn gửi.
 */
export function CommandInput({ server }: { server: Server }) {
  const { t, i18n } = useTranslation();
  const listId = useId();
  const [value, setValue] = useState("");
  const [history, setHistory] = useState(emptyHistory);
  const [active, setActive] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const send = useMutation({ mutationFn: (command: string) => api.sendCommand(server.id, command) });
  // Backend chỉ nhận lệnh khi server có process; bấm Gửi lúc khác chỉ nhận lỗi SERVER_BUSY.
  const online = server.state === "running" || server.state === "starting";
  const scanned = useServerCommands(server.id).data?.commands;
  const serverRegistry = useServerRegistry(server.id).data;
  const registryIndex = useMemo(() => buildRegistryIndex(serverRegistry, i18n.language), [serverRegistry, i18n.language]);
  const players: Player[] = usePlayers(server.id, server.state).data?.players ?? [];
  const analysis = analyzeConsole(value, scanned, players, registryIndex);
  const items = dismissed ? [] : analysis.hints.map((h) => hintItem(t, h));
  const current = Math.min(active, Math.max(items.length - 1, 0));

  function change(next: string) {
    setValue(next);
    setActive(0);
    setDismissed(false);
  }

  function pick(picked: string) {
    change(analysis.fill(picked));
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
        <Stack direction="row" gap={2} align="end">
          <SuggestBox
            id={listId}
            label={t("console.hintsLabel")}
            items={items}
            active={current}
            onPick={pick}
            syntax={renderSyntax(t, analysis.syntax)}
          >
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
        <Hint>{t("console.shortcutHint")}</Hint>
        {send.isError && <Notice tone="danger">{errorMessage(t, send.error)}</Notice>}
      </Stack>
    </form>
  );
}
