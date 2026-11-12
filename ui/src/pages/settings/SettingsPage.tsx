import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { authApi, SESSION_KEY } from "../../api/auth";
import { errorMessage } from "../../api/client";
import { SystemCard } from "../../components/SystemCard";
import { Notice, PageHeader, Spinner, Stack } from "../../ui";
import { LanCard } from "./LanCard";
import { SecurityCard } from "./SecurityCard";

/** Trang Cài đặt (spec P2, mục 6.10): bảo mật, truy cập từ LAN, thông tin máy. */
export function SettingsPage() {
  const { t } = useTranslation();
  const session = useQuery({ queryKey: SESSION_KEY, queryFn: authApi.session });

  return (
    <>
      <PageHeader title={t("nav.settings")} description={t("appSettings.description")} />
      {session.isPending && <Spinner label={t("appSettings.loading")} />}
      {session.isError && <Notice tone="danger">{errorMessage(t, session.error)}</Notice>}
      {session.data && (
        <Stack gap={5}>
          <SecurityCard session={session.data} />
          <LanCard session={session.data} />
          <SystemCard />
        </Stack>
      )}
    </>
  );
}
