import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage } from "../../api/client";
import { DOWNLOAD_CACHE_KEY, settingsApi } from "../../api/settings";
import { formatBytes } from "../../format";
import { Button, Card, ConfirmDialog, Facts, Notice, Spinner, Stack } from "../../ui";

/** Mục "Bộ nhớ đệm tải về" (spec P3, mục 3.4, 6.5): dung lượng và nút xóa. */
export function DownloadCacheCard() {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState(false);
  const size = useQuery({ queryKey: DOWNLOAD_CACHE_KEY, queryFn: settingsApi.downloadCache });
  const clear = useMutation({
    mutationFn: settingsApi.clearDownloadCache,
    onSuccess: () => {
      setConfirming(false);
      void queryClient.invalidateQueries({ queryKey: DOWNLOAD_CACHE_KEY });
    },
  });
  const bytes = size.data?.size_bytes;
  return (
    <Card title={t("appSettings.cacheTitle")}>
      <p>{t("appSettings.cacheHint")}</p>
      {size.isPending && <Spinner label={t("appSettings.cacheLoading")} />}
      {size.isError && <Notice tone="danger">{errorMessage(t, size.error)}</Notice>}
      {bytes !== undefined && <Facts items={[{ label: t("appSettings.cacheSize"), value: formatBytes(bytes, i18n.language) }]} />}
      {clear.isSuccess && (
        <Notice tone="success">{t("appSettings.cacheCleared", { size: formatBytes(clear.data.freed_bytes, i18n.language) })}</Notice>
      )}
      {clear.isError && !confirming && <Notice tone="danger">{errorMessage(t, clear.error)}</Notice>}
      <Stack direction="row" justify="end">
        <Button icon={Trash2} variant="danger" disabled={!bytes} onClick={() => setConfirming(true)}>
          {t("appSettings.cacheClear")}
        </Button>
      </Stack>
      <ConfirmDialog
        open={confirming}
        title={t("appSettings.cacheClearTitle")}
        confirmLabel={t("appSettings.cacheClear")}
        loading={clear.isPending}
        onCancel={() => {
          setConfirming(false);
          clear.reset();
        }}
        onConfirm={() => clear.mutate()}
      >
        <p>{t("appSettings.cacheClearBody")}</p>
        {clear.isError && <Notice tone="danger">{errorMessage(t, clear.error)}</Notice>}
      </ConfirmDialog>
    </Card>
  );
}

/** Mục "Giới hạn tài nguyên" (spec P3, mục 3.8, 6.5): máy này có giới hạn được RAM của server không. */
export function ResourceLimitsCard() {
  const { t } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  const enforced = system.data?.resource_limits.enforced;
  return (
    <Card title={t("appSettings.limitsTitle")}>
      {enforced === true && <Notice tone="success">{t("appSettings.limitsOn")}</Notice>}
      {enforced === false && (
        <Notice tone="warning" title={t("appSettings.limitsOffTitle")}>
          {t("appSettings.limitsOff")}
        </Notice>
      )}
    </Card>
  );
}
