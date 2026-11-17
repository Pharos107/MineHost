import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ListRestart } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, type Server, type ServerCommands } from "../../../api/client";
import { IconButton } from "../../../ui";

export function commandsKey(id: string) {
  return ["server-commands", id] as const;
}

/** Lệnh đã quét của server; ô lệnh và nút quét dùng chung. */
export function useServerCommands(id: string) {
  return useQuery({ queryKey: commandsKey(id), queryFn: () => api.serverCommands(id) });
}

/** Quét lệnh bằng `help`; hàng lệnh nhanh hiện lỗi chung với các nút khác. */
export function useScanCommands(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.scanCommands(id),
    onSuccess: (data) => queryClient.setQueryData<ServerCommands>(commandsKey(id), data),
  });
}

/**
 * Nút icon cuối hàng lệnh nhanh (spec lenh-cua-server.md): số lệnh và giờ quét nằm trong chú thích để
 * hàng gọn một dòng. Không tự quét vì `help` in hàng trăm dòng vào console.
 */
export function ScanButton({ server, scan }: { server: Server; scan: ReturnType<typeof useScanCommands> }) {
  const { t, i18n } = useTranslation();
  const commands = useServerCommands(server.id).data;
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" });
  const label = commands?.scanned_at
    ? t("console.scan.again", { count: commands.commands.length, at: time.format(Date.parse(commands.scanned_at)) })
    : t("console.scan.first");

  return (
    <IconButton
      icon={ListRestart}
      size="sm"
      label={label}
      disabled={server.state !== "running" || scan.isPending}
      aria-busy={scan.isPending || undefined}
      onClick={() => scan.mutate()}
    />
  );
}
