import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams } from "react-router";

import { api, errorMessage } from "../api/client";
import { SERVERS_KEY } from "../api/live";
import { Button, Notice, PageHeader, Spinner, StatusBadge } from "../ui";
import { serverName } from "./ServerListPage";

/** Đầu trang chi tiết server. Các nút, console và cài đặt thuộc PR-14. */
export function ServerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const id = useParams().id ?? "";
  const server = useQuery({ queryKey: [...SERVERS_KEY, id], queryFn: () => api.server(id) });
  const back = (
    <Button variant="ghost" icon={ArrowLeft} onClick={() => void navigate("/")}>
      {t("servers.back")}
    </Button>
  );

  if (server.isPending) return <Spinner label={t("ui.loading")} />;
  if (server.isError) {
    return (
      <Notice tone="danger" action={back}>
        {errorMessage(t, server.error)}
      </Notice>
    );
  }
  return <PageHeader title={serverName(server.data)} description={<StatusBadge state={server.data.state} />} actions={back} />;
}
