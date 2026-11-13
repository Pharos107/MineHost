import { useMutation, useQuery } from "@tanstack/react-query";
import { ExternalLink, LayoutGrid, Play, Plus, RotateCw, Rows3, Server as ServerIcon, Square, SquareTerminal } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Link, useLocation, useNavigate } from "react-router";

import { api, errorMessage, type Server, type ServerAction } from "../api/client";
import { SERVERS_KEY } from "../api/live";
import { formatMemoryMb } from "../format";
import {
  BlockBanner,
  Button,
  DataTable,
  EmptyState,
  filterRows,
  Notice,
  PageHeader,
  SearchInput,
  SegmentedControl,
  Select,
  ServerCardGrid,
  Stack,
  StatusBadge,
  type Column,
  type RowAction,
} from "../ui";
import { STATE_FILTERS, matchesState, playAddress, type StateFilter } from "./serverFilters";
import { serverName, ServerTile } from "./ServerTile";

export { serverName };

type View = "cards" | "table";
const VIEW_KEY = "minehost.servers.view";

/** Cách xem đã chọn lần trước, chỉ trên trình duyệt này. Không đọc được thì dùng thẻ. */
function savedView(): View {
  try {
    return localStorage.getItem(VIEW_KEY) === "table" ? "table" : "cards";
  } catch {
    return "cards";
  }
}

function saveView(view: View) {
  try {
    localStorage.setItem(VIEW_KEY, view);
  } catch {
    // Trình duyệt chặn lưu: lần sau quay về thẻ, không sao.
  }
}

/** Trang Máy chủ (spec P2, mục 6.2): tìm, lọc theo trạng thái, xem dạng thẻ hoặc bảng, thao tác ngay. */
export function ServerListPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: SERVERS_KEY, queryFn: api.servers });
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  const [stateFilter, setStateFilter] = useState<StateFilter>("all");
  const [query, setQuery] = useState("");
  const [view, setView] = useState<View>(savedView);
  const all = servers.data ?? [];
  const rows = all.filter((server) => matchesState(server, stateFilter));
  const goCreate = () => void navigate("/new");
  // Trang chi tiết chuyển về đây sau khi xóa, kèm thư mục trong thùng rác để người dùng biết chỗ lấy lại.
  const trashed = (useLocation().state as { trashed?: string } | null)?.trashed;
  const hasServers = all.length > 0;
  const changeView = (next: View) => {
    setView(next);
    saveView(next);
  };

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
      <Stack gap={5}>
        {trashed && (
          <Notice tone="success" title={t("delete.done")}>
            <code>{trashed}</code>
          </Notice>
        )}
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
        ) : !servers.isPending && !hasServers ? (
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
        ) : (
          <>
            <Stack direction="row" gap={3} justify="between" align="center" wrap>
              <Stack direction="row" gap={3} align="center" wrap>
                <StateSelect value={stateFilter} onChange={setStateFilter} />
                {view === "cards" && <SearchInput value={query} onChange={setQuery} label={t("ui.table.search")} />}
              </Stack>
              <SegmentedControl
                label={t("servers.view")}
                value={view}
                onChange={changeView}
                options={[
                  { value: "cards", label: t("servers.viewCards"), icon: LayoutGrid },
                  { value: "table", label: t("servers.viewTable"), icon: Rows3 },
                ]}
              />
            </Stack>
            {view === "cards" ? (
              <Cards servers={rows} query={query} lan={system.data?.lan_addresses} />
            ) : (
              <ServerTable servers={rows} loading={servers.isPending} />
            )}
          </>
        )}
      </Stack>
    </>
  );
}

function Cards({ servers, query, lan }: { servers: Server[]; query: string; lan: string[] | undefined }) {
  const { t } = useTranslation();
  const shown = filterRows(
    servers,
    query,
    (s) => `${serverName(s)} ${s.loader ? t(`loaders.${s.loader}`) : ""} ${s.minecraft ?? ""}`,
  );
  if (shown.length === 0) return <EmptyState icon={ServerIcon} title={t("servers.noMatch")} />;
  return (
    <ServerCardGrid>
      {shown.map((server) => (
        <ServerTile key={server.id} server={server} lan={lan} />
      ))}
    </ServerCardGrid>
  );
}

function ServerTable({ servers, loading }: { servers: Server[]; loading: boolean }) {
  const { t } = useTranslation();
  const { actions, error } = useRowActions();
  const columns = useColumns();
  return (
    <>
      {error && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
      <DataTable
        label={t("servers.heading")}
        columns={columns}
        rows={servers}
        rowKey={(server) => server.id}
        rowLabel={serverName}
        actions={actions}
        inlineActions={2}
        searchText={(server) => `${serverName(server)} ${server.loader ? t(`loaders.${server.loader}`) : ""} ${server.minecraft ?? ""}`}
        loading={loading}
        empty={<EmptyState icon={ServerIcon} title={t("servers.noMatch")} />}
      />
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
      cell: (server) => (
        <Stack direction="row" gap={2} align="center">
          <BlockBanner seed={server.id} variant="mark" />
          <Link to={`/servers/${server.id}`}>{serverName(server)}</Link>
        </Stack>
      ),
      sortValue: serverName,
      nowrap: true,
    },
    { id: "state", header: t("servers.state"), cell: (server) => <StatusBadge state={server.state} />, nowrap: true },
    {
      id: "version",
      header: t("servers.version"),
      cell: (server) => (server.minecraft ? `${t(`loaders.${server.loader ?? "paper"}`)} ${server.minecraft}` : "—"),
      sortValue: (server) => server.minecraft ?? "",
      nowrap: true,
    },
    {
      id: "address",
      header: t("servers.address"),
      cell: (server) => playAddress(server, lan) ?? "—",
      sortValue: (server) => server.port ?? 0,
      nowrap: true,
    },
    {
      id: "memory",
      header: t("servers.memory"),
      cell: (server) => (server.memory_mb ? formatMemoryMb(server.memory_mb.max, i18n.language) : "—"),
      sortValue: (server) => server.memory_mb?.max ?? 0,
      align: "end",
      nowrap: true,
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
