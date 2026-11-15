import { useMutation } from "@tanstack/react-query";
import { Play, RotateCw, Skull, Square } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, ApiError, errorMessage, type Server, type ServerAction } from "../../api/client";
import { Button, ConfirmDialog, Dialog, DialogBody, DialogFooter, Menu, Notice, Stack } from "../../ui";

const EULA_URL = "https://aka.ms/MinecraftEULA";
const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

function isEulaError(error: unknown): boolean {
  return error instanceof ApiError && error.code === "EULA_NOT_ACCEPTED";
}

/** Nút Bật / Tắt / Khởi động lại theo trạng thái; "Buộc dừng" nằm trong menu "…" và phải xác nhận. */
export function ServerActions({ server }: { server: Server }) {
  const { t } = useTranslation();
  const [eulaOpen, setEulaOpen] = useState(false);
  const [killOpen, setKillOpen] = useState(false);
  const run = useMutation({
    mutationFn: (action: ServerAction) => api.action(server.id, action),
    // Trạng thái EULA trên trang có thể cũ hơn backend: hỏi lại thay vì chỉ báo lỗi.
    onError: (error) => isEulaError(error) && setEulaOpen(true),
    onSettled: () => setKillOpen(false),
  });
  const accept = useMutation({
    mutationFn: () => api.acceptEula(server.id),
    onSuccess: () => {
      setEulaOpen(false);
      run.mutate("start");
    },
  });

  const { state } = server;
  const pending = (action: ServerAction) => run.isPending && run.variables === action;
  const busy = run.isPending || state === "stopping";
  const start = () => (server.eula_accepted ? run.mutate("start") : setEulaOpen(true));
  const hideStaleStartError = run.variables === "start" && WITH_PROCESS.includes(state);

  return (
    <Stack gap={2} align="end">
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
        <Menu
          label={t("server.more")}
          items={[
            { label: t("server.kill"), icon: Skull, tone: "danger", disabled: !WITH_PROCESS.includes(state), onSelect: () => setKillOpen(true) },
          ]}
        />
      </Stack>
      {run.isError && !isEulaError(run.error) && !hideStaleStartError && (
        <Notice tone="danger">{errorMessage(t, run.error)}</Notice>
      )}
      <ConfirmDialog
        open={killOpen}
        title={t("server.killTitle")}
        confirmLabel={t("server.kill")}
        loading={pending("kill")}
        onCancel={() => setKillOpen(false)}
        onConfirm={() => run.mutate("kill")}
      >
        <p>{t("server.killBody")}</p>
      </ConfirmDialog>
      <Dialog open={eulaOpen} onClose={() => setEulaOpen(false)} title={t("eula.title")} dismissible={!accept.isPending}>
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
          <Button variant="ghost" disabled={accept.isPending} onClick={() => setEulaOpen(false)}>
            {t("ui.cancel")}
          </Button>
          <Button variant="primary" loading={accept.isPending} onClick={() => accept.mutate()}>
            {t("eula.accept")}
          </Button>
        </DialogFooter>
      </Dialog>
    </Stack>
  );
}
