import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ListRestart } from "lucide-react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server, type ServerCommands } from "../../../api/client";
import { Badge, Button, Notice, Stack } from "../../../ui";

export function commandsKey(id: string) {
  return ["server-commands", id] as const;
}

/** Lệnh đã quét của server; ô lệnh và nút quét dùng chung. */
export function useServerCommands(id: string) {
  return useQuery({ queryKey: commandsKey(id), queryFn: () => api.serverCommands(id) });
}

/**
 * Nút lấy danh sách lệnh của server bằng `help` (spec lenh-cua-server.md). Không tự quét vì `help` in
 * hàng trăm dòng vào console; đã quét thì hiện số lệnh và giờ quét.
 */
export function ScanCommands({ server }: { server: Server }) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const commands = useServerCommands(server.id);
  const scan = useMutation({
    mutationFn: () => api.scanCommands(server.id),
    onSuccess: (data) => queryClient.setQueryData<ServerCommands>(commandsKey(server.id), data),
  });
  const scannedAt = commands.data?.scanned_at;
  const time = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short", timeStyle: "short" });

  return (
    <Stack gap={2} align="end">
      <Stack direction="row" gap={2} align="center" wrap>
        {scannedAt && (
          <Badge>{t("console.scan.summary", { count: commands.data?.commands.length ?? 0, at: time.format(Date.parse(scannedAt)) })}</Badge>
        )}
        <Button
          variant="ghost"
          size="sm"
          icon={ListRestart}
          disabled={server.state !== "running"}
          loading={scan.isPending}
          onClick={() => scan.mutate()}
        >
          {scannedAt ? t("console.scan.again") : t("console.scan.first")}
        </Button>
      </Stack>
      {scan.isError && <Notice tone="danger">{errorMessage(t, scan.error)}</Notice>}
    </Stack>
  );
}
