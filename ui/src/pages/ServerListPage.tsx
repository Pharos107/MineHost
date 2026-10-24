import { useQuery } from "@tanstack/react-query";
import { Plus, RotateCw, Server as ServerIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Link, useNavigate } from "react-router";

import { api, errorMessage, type Server } from "../api/client";
import { SERVERS_KEY } from "../api/live";
import { SystemCard } from "../components/SystemCard";
import { formatMemoryMb } from "../format";
import { Button, DataTable, EmptyState, Notice, PageHeader, StatusBadge, type Column } from "../ui";

export function serverName(server: Server): string {
  return server.name ?? server.id;
}

export function ServerListPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const rows = servers.data ?? [];
  const goCreate = () => void navigate("/new");

  const columns: Column<Server>[] = [
    {
      id: "name",
      header: t("servers.name"),
      cell: (server) => <Link to={`/servers/${server.id}`}>{serverName(server)}</Link>,
      sortValue: serverName,
    },
    { id: "state", header: t("servers.state"), cell: (server) => <StatusBadge state={server.state} />, width: "md" },
    {
      id: "version",
      header: t("servers.version"),
      cell: (server) => (server.minecraft ? `${t(`loaders.${server.loader ?? "paper"}`)} ${server.minecraft}` : "—"),
      width: "md",
    },
    {
      id: "memory",
      header: t("servers.memory"),
      cell: (server) => (server.memory_mb ? formatMemoryMb(server.memory_mb.max, i18n.language) : "—"),
      sortValue: (server) => server.memory_mb?.max ?? 0,
      align: "end",
      width: "sm",
    },
    { id: "port", header: t("servers.port"), cell: (server) => server.port ?? "—", align: "end", width: "sm" },
  ];

  return (
    <>
      <PageHeader
        title={t("servers.heading")}
        description={t("app.tagline")}
        actions={
          rows.length > 0 && (
            <Button variant="primary" icon={Plus} onClick={goCreate}>
              {t("servers.create")}
            </Button>
          )
        }
      />
      {servers.isError ? (
        <Notice
          tone="danger"
          action={
            <Button size="sm" icon={RotateCw} onClick={() => void servers.refetch()}>
              {t("common.retry")}
            </Button>
          }
        >
          {errorMessage(t, servers.error)}
        </Notice>
      ) : (
        <DataTable
          label={t("servers.heading")}
          columns={columns}
          rows={rows}
          rowKey={(server) => server.id}
          rowLabel={serverName}
          searchText={(server) => `${serverName(server)} ${server.minecraft ?? ""}`}
          loading={servers.isPending}
          empty={
            <EmptyState
              icon={ServerIcon}
              title={t("servers.emptyTitle")}
              description={t("servers.emptyDescription")}
              action={
                <Button variant="primary" icon={Plus} onClick={goCreate}>
                  {t("servers.createFirst")}
                </Button>
              }
            />
          }
        />
      )}
      <SystemCard />
    </>
  );
}
