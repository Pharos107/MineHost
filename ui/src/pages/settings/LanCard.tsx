import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { ACCESS_KEY, authApi, type Session } from "../../api/auth";
import { errorMessage } from "../../api/client";
import { Card, CopyField, Notice, Spinner, Stack, Switch } from "../../ui";

/** Mục "Truy cập từ LAN": công tắc, chỉ bật được khi đã có mật khẩu (spec P2, mục 3.4 và 6.10). */
export function LanCard({ session }: { session: Session }) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const access = useQuery({ queryKey: ACCESS_KEY, queryFn: authApi.access });
  const toggle = useMutation({
    mutationFn: authApi.setAccess,
    onSuccess: (data) => queryClient.setQueryData(ACCESS_KEY, data),
  });
  const on = access.data?.lan ?? false;
  const urls = on ? (access.data?.lan_urls ?? []) : [];

  return (
    <Card title={t("appSettings.lan")}>
      <Stack gap={4}>
        <Notice tone="warning">{t("appSettings.lanWarning")}</Notice>
        {access.isPending ? (
          <Spinner label={t("appSettings.loading")} />
        ) : (
          <Switch
            label={t("appSettings.lanSwitch")}
            description={session.password_set ? t("appSettings.lanHint") : t("appSettings.lanNeedsPassword")}
            checked={on}
            disabled={!session.password_set || toggle.isPending}
            onChange={(lan) => toggle.mutate(lan)}
          />
        )}
        {access.isError && <Notice tone="danger">{errorMessage(t, access.error)}</Notice>}
        {toggle.isError && <Notice tone="danger">{errorMessage(t, toggle.error)}</Notice>}
        {on && urls.length === 0 && <Notice tone="info">{t("appSettings.lanNoAddress")}</Notice>}
        {urls.map((url) => (
          <CopyField key={url} value={url} label={t("appSettings.lanUrl")} />
        ))}
      </Stack>
    </Card>
  );
}
