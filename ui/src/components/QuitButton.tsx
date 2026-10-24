import { useMutation, useQuery } from "@tanstack/react-query";
import { Power } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../api/client";
import { SERVERS_KEY } from "../api/live";
import { Button, ConfirmDialog, Dialog, DialogBody, Notice, ProgressBar } from "../ui";

const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

function runningIds(servers: Server[] | undefined): string[] {
  return (servers ?? []).filter((s) => WITH_PROCESS.includes(s.state) || s.restart_pending).map((s) => s.id);
}

/**
 * Nút "Thoát" (spec P1, mục 2.5): còn server chạy thì hỏi lại, rồi hiện tiến độ tắt từng server
 * cho tới khi MineHost tắt hẳn. Đóng tab trình duyệt thì không ảnh hưởng gì.
 */
export function QuitButton() {
  const { t } = useTranslation();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const [confirming, setConfirming] = useState(false);
  // Danh sách server cần chờ, chụp lúc bấm Thoát.
  const [stopping, setStopping] = useState<string[] | null>(null);
  const quit = useMutation({ mutationFn: api.shutdown, onSuccess: () => setConfirming(false) });
  const running = runningIds(servers.data);

  function start() {
    setStopping(running);
    quit.mutate();
  }

  return (
    <>
      {/* Chưa biết có server nào chạy thì chưa cho bấm, để không thoát mà quên hỏi lại. */}
      <Button
        variant="ghost"
        size="sm"
        icon={Power}
        disabled={servers.isPending}
        onClick={() => (running.length > 0 ? setConfirming(true) : start())}
      >
        {t("quit.button")}
      </Button>
      <ConfirmDialog
        open={confirming}
        title={t("quit.title")}
        confirmLabel={t("quit.confirm")}
        tone="primary"
        loading={quit.isPending}
        onCancel={() => setConfirming(false)}
        onConfirm={start}
      >
        <p>{t("quit.body", { count: running.length })}</p>
        {quit.isError && <Notice tone="danger">{errorMessage(t, quit.error)}</Notice>}
      </ConfirmDialog>
      {quit.isSuccess && stopping && <ShutdownDialog ids={stopping} servers={servers.data ?? []} />}
    </>
  );
}

function ShutdownDialog({ ids, servers }: { ids: string[]; servers: Server[] }) {
  const { t } = useTranslation();
  // Hỏi `/health` mỗi giây: hết trả lời nghĩa là MineHost đã tắt hẳn.
  const health = useQuery({ queryKey: ["health"], queryFn: api.health, refetchInterval: 1000, retry: false });
  const stillRunning = runningIds(servers.filter((s) => ids.includes(s.id))).length;
  const done = ids.length - stillRunning;

  return (
    <Dialog open onClose={() => {}} title={t("quit.progressTitle")} dismissible={false}>
      <DialogBody>
        {health.isError ? (
          <Notice tone="success">{t("quit.done")}</Notice>
        ) : (
          <ProgressBar
            label={ids.length > 0 ? t("quit.progress", { done, total: ids.length }) : t("quit.closing")}
            value={ids.length > 0 ? (done / ids.length) * 100 : undefined}
          />
        )}
      </DialogBody>
    </Dialog>
  );
}
