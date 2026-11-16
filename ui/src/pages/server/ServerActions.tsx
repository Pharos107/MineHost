import { useMutation, type UseMutationResult } from "@tanstack/react-query";
import { Play, RotateCw, Skull, Square } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, ApiError, errorMessage, type Server, type ServerAction } from "../../api/client";
import { Button, ConfirmDialog, Dialog, DialogBody, DialogFooter, IconButton, Menu, Notice, Stack } from "../../ui";

const EULA_URL = "https://aka.ms/MinecraftEULA";
const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

function isEulaError(error: unknown): boolean {
  return error instanceof ApiError && error.code === "EULA_NOT_ACCEPTED";
}

interface ServerActionsProps {
  server: Server;
  /**
   * `full`: nút có chữ và menu "…" (thẻ server, đầu trang trên điện thoại). `compact`: chỉ icon, không
   * menu (thanh bên). `menu`: chỉ menu "…" với Buộc dừng (đầu trang khi thanh bên đã có nút bật / tắt).
   */
  variant?: "full" | "compact" | "menu";
}

/** Nút Bật / Tắt / Khởi động lại theo trạng thái; "Buộc dừng" nằm trong menu "…" và phải xác nhận. */
export function ServerActions({ server, variant = "full" }: ServerActionsProps) {
  const { t } = useTranslation();
  const [eulaOpen, setEulaOpen] = useState(false);
  const [killOpen, setKillOpen] = useState(false);
  const run = useMutation({
    mutationFn: (action: ServerAction) => api.action(server.id, action),
    // Trạng thái EULA trên trang có thể cũ hơn backend: hỏi lại thay vì chỉ báo lỗi.
    onError: (error) => isEulaError(error) && setEulaOpen(true),
    onSettled: () => setKillOpen(false),
  });

  const start = () => (server.eula_accepted ? run.mutate("start") : setEulaOpen(true));
  const hideStaleStartError = run.variables === "start" && WITH_PROCESS.includes(server.state);
  const buttons = { state: server.state, run, start, onKill: () => setKillOpen(true) };
  const failed = run.isError && !isEulaError(run.error) && !hideStaleStartError;

  return (
    <Stack gap={2} align={variant === "compact" ? "start" : "end"}>
      {variant === "compact" ? <IconButtons {...buttons} /> : variant === "menu" ? <MoreMenu {...buttons} /> : <FullButtons {...buttons} />}
      {/* Thanh bên quá hẹp cho thông báo dài (ví dụ danh sách mod thiếu): hiện trong hộp thoại. */}
      {failed && variant === "compact" && (
        <Dialog open onClose={() => run.reset()} title={t("server.actionFailed")}>
          <DialogBody>
            <Notice tone="danger">{errorMessage(t, run.error)}</Notice>
          </DialogBody>
        </Dialog>
      )}
      {failed && variant !== "compact" && (
        <Notice tone="danger">{errorMessage(t, run.error)}</Notice>
      )}
      <ConfirmDialog
        open={killOpen}
        title={t("server.killTitle")}
        confirmLabel={t("server.kill")}
        loading={run.isPending && run.variables === "kill"}
        onCancel={() => setKillOpen(false)}
        onConfirm={() => run.mutate("kill")}
      >
        <p>{t("server.killBody")}</p>
      </ConfirmDialog>
      <EulaDialog server={server} open={eulaOpen} onClose={() => setEulaOpen(false)} onAccepted={() => run.mutate("start")} />
    </Stack>
  );
}

interface ButtonsProps {
  state: Server["state"];
  run: UseMutationResult<unknown, Error, ServerAction>;
  start: () => void;
  onKill: () => void;
}

function FullButtons({ state, run, start, onKill }: ButtonsProps) {
  const { t } = useTranslation();
  const pending = (action: ServerAction) => run.isPending && run.variables === action;
  const busy = run.isPending || state === "stopping";

  return (
    <Stack direction="row" gap={2} wrap>
      {(state === "stopped" || state === "crashed") && (
        <Button variant="primary" icon={Play} loading={pending("start")} onClick={start}>
          {t("server.start")}
        </Button>
      )}
      {state === "running" && (
        <Button icon={RotateCw} disabled={busy} loading={pending("restart")} onClick={() => run.mutate("restart")}>
          {t("server.restart")}
        </Button>
      )}
      {WITH_PROCESS.includes(state) && (
        <Button icon={Square} disabled={busy} loading={state === "stopping" || pending("stop")} onClick={() => run.mutate("stop")}>
          {t("server.stop")}
        </Button>
      )}
      <MoreMenu state={state} run={run} start={start} onKill={onKill} />
    </Stack>
  );
}

function MoreMenu({ state, onKill }: ButtonsProps) {
  const { t } = useTranslation();
  return (
    <Menu
      label={t("server.more")}
      items={[{ label: t("server.kill"), icon: Skull, tone: "danger", disabled: !WITH_PROCESS.includes(state), onSelect: onKill }]}
    />
  );
}

/** Bản gọn ở thanh bên: chỉ icon, "Buộc dừng" vẫn chỉ có ở đầu trang. */
function IconButtons({ state, run, start }: ButtonsProps) {
  const { t } = useTranslation();
  const busy = run.isPending || state === "stopping";

  return (
    <Stack direction="row" gap={1}>
      {(state === "stopped" || state === "crashed") && (
        <IconButton icon={Play} size="sm" variant="primary" label={t("server.start")} disabled={run.isPending} onClick={start} />
      )}
      {state === "running" && (
        <IconButton icon={RotateCw} size="sm" label={t("server.restart")} disabled={busy} onClick={() => run.mutate("restart")} />
      )}
      {WITH_PROCESS.includes(state) && (
        <IconButton icon={Square} size="sm" label={t("server.stop")} disabled={busy} onClick={() => run.mutate("stop")} />
      )}
    </Stack>
  );
}

interface EulaDialogProps {
  server: Server;
  open: boolean;
  onClose: () => void;
  onAccepted: () => void;
}

function EulaDialog({ server, open, onClose, onAccepted }: EulaDialogProps) {
  const { t } = useTranslation();
  const accept = useMutation({
    mutationFn: () => api.acceptEula(server.id),
    onSuccess: () => {
      onClose();
      onAccepted();
    },
  });

  return (
    <Dialog open={open} onClose={onClose} title={t("eula.title")} dismissible={!accept.isPending}>
      <DialogBody>
        <p>{t("eula.body")}</p>
        <p>
          <a href={EULA_URL} target="_blank" rel="noreferrer">
            {t("eula.link")}
          </a>
        </p>
        {accept.isError && <Notice tone="danger">{errorMessage(t, accept.error)}</Notice>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" disabled={accept.isPending} onClick={onClose}>
          {t("ui.cancel")}
        </Button>
        <Button variant="primary" loading={accept.isPending} onClick={() => accept.mutate()}>
          {t("eula.accept")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
