import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, Play, Plus, RotateCw, Server as ServerIcon, Square, SquareTerminal } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation, useNavigate } from "react-router";

import { api, errorMessage, type Server, type ServerAction } from "../api/client";
import { SERVERS_KEY } from "../api/live";
import { SystemCard } from "../components/SystemCard";
import { formatMemoryMb } from "../format";
import {
  Button,
  DataTable,
  EmptyState,
  Notice,
  PageHeader,
  Select,
  Stack,
  StatusBadge,
  type Column,
  type RowAction,
} from "../ui";
import { STATE_FILTERS, matchesState, playAddress, type StateFilter } from "./serverFilters";

export function serverName(server: Server): string {
  return server.name ?? server.id;
}

/** Trang Máy chủ (spec P2, mục 6.2): tìm, lọc theo trạng thái, sắp xếp và thao tác ngay trên từng dòng. */
export function ServerListPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const [stateFilter, setStateFilter] = useState<StateFilter>("all");
  const rows = (servers.data ?? []).filter((server) => matchesState(server, stateFilter));
  const goCreate = () => void navigate("/new");
  // Trang chi tiết chuyển về đây sau khi xóa, kèm thư mục trong thùng rác để người dùng biết chỗ lấy lại.
  const trashed = (useLocation().state as { trashed?: string } | null)?.trashed;
  const { actions, error } = useRowActions();
  const columns = useColumns();
  const hasServers = (servers.data ?? []).length > 0;
  const stateSelect = <StateSelect value={stateFilter} onChange={setStateFilter} />;

  return (
    <>
      <PageHeader
        title={t("servers.heading")}
        description={t("app.tagline")}
        actions={
          hasServers && (
            <Button variant="primary" icon={Plus} onClick={goCreate}>
              {t("servers.create")}
            </Button>
          )
        }
      />
      {trashed && (
        <Notice tone="success" title={t("delete.done")}>
          <code>{trashed}</code>
        </Notice>
      )}
      {error && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
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
          actions={actions}
          inlineActions={2}
          searchText={(server) => `${serverName(server)} ${server.minecraft ?? ""}`}
          toolbar={hasServers && stateSelect}
          loading={servers.isPending}
          empty={
            hasServers ? (
              // Bảng trống vì bộ lọc: vẫn giữ ô lọc để người dùng đổi lại được.
              <Stack gap={3}>
                <div>{stateSelect}</div>
                <EmptyState icon={ServerIcon} title={t("servers.noMatch")} />
              </Stack>
            ) : (
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
            )
          }
        />
      )}
      <SystemCard />
    </>
  );
}

function StateSelect({ value, onChange }: { value: StateFilter; onChange: (value: StateFilter) => void }) {
  const { t } = useTranslation();
  return (
    <Select aria-label={t("servers.filterState")} value={value} onChange={(e) => onChange(e.target.value as StateFilter)}>
      {STATE_FILTERS.map((filter) => (
        <option key={filter} value={filter}>
          {t(`servers.filters.${filter}`)}
        </option>
      ))}
    </Select>
  );
}

function useColumns(): Column<Server>[] {
  const { t, i18n } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  const lan = system.data?.lan_addresses;
  return [
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
      sortValue: (server) => server.minecraft ?? "",
      width: "md",
    },
    {
      id: "address",
      header: t("servers.address"),
      cell: (server) => playAddress(server, lan) ?? "—",
      sortValue: (server) => server.port ?? 0,
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
  ];
}

const WITH_PROCESS: Server["state"][] = ["starting", "running", "stopping"];

/**
 * Bật / Tắt / Khởi động lại trên từng dòng. Server chưa đồng ý EULA thì không có nút Bật ở đây:
 * hộp thoại EULA nằm ở trang server, để người dùng đọc trước khi đồng ý.
 */
function useRowActions(): { actions: RowAction<Server>[]; error: unknown } {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const run = useMutation({
    mutationFn: ({ id, action }: { id: string; action: ServerAction }) => api.action(id, action),
  });
  const act = (action: ServerAction) => (server: Server) => run.mutate({ id: server.id, action });
  const canStart = (s: Server) => (s.state === "stopped" || s.state === "crashed") && s.eula_accepted;

  const actions: RowAction<Server>[] = [
    { label: t("server.start"), icon: Play, onSelect: act("start"), hidden: (s) => !canStart(s) },
    { label: t("server.stop"), icon: Square, onSelect: act("stop"), hidden: (s) => !WITH_PROCESS.includes(s.state) },
    { label: t("server.restart"), icon: RotateCw, onSelect: act("restart"), hidden: (s) => s.state !== "running" },
    { label: t("servers.open"), icon: ExternalLink, onSelect: (s) => void navigate(`/servers/${s.id}`) },
    { label: t("nav.console"), icon: SquareTerminal, onSelect: (s) => void navigate(`/servers/${s.id}/console`) },
  ];
  return { actions, error: run.error };
}
