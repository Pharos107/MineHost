import { useMutation } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";

import { api, errorMessage, type Server } from "../../api/client";
import { Button, Card, ConfirmDialog, Notice } from "../../ui";
import { serverName } from "../ServerListPage";

/** Trạng thái được xóa: không có process. Backend kiểm tra lại, đây chỉ để khóa nút sớm. */
const DELETABLE: Server["state"][] = ["stopped", "crashed", "broken"];

/** Xóa là chuyển thư mục server vào `trash/`; phải gõ lại tên server mới cho bấm. */
export function DeleteServer({ server }: { server: Server }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [open, setOpen] = useState(false);
  const name = serverName(server);
  const trash = useMutation({
    mutationFn: () => api.trashServer(server.id, name),
    onSuccess: (result) => void navigate("/", { state: { trashed: result.trash_path } }),
  });
  const allowed = DELETABLE.includes(server.state) && !server.restart_pending;

  return (
    <Card
      title={t("delete.title")}
      description={allowed ? t("delete.description") : t("delete.stopFirst")}
      actions={
        <Button variant="danger" icon={Trash2} disabled={!allowed} onClick={() => setOpen(true)}>
          {t("delete.button")}
        </Button>
      }
    >
      <ConfirmDialog
        open={open}
        title={t("delete.confirmTitle", { name })}
        confirmLabel={t("delete.button")}
        requireText={name}
        loading={trash.isPending}
        onCancel={() => setOpen(false)}
        onConfirm={() => trash.mutate()}
      >
        <p>{t("delete.confirmBody")}</p>
        {trash.isError && <Notice tone="danger">{errorMessage(t, trash.error)}</Notice>}
      </ConfirmDialog>
    </Card>
  );
}
