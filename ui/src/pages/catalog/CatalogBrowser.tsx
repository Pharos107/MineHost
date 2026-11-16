import { useInfiniteQuery, useQuery } from "@tanstack/react-query";
import { Package, RotateCw } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../api/client";
import { contentApi, type CatalogScope, type SearchHit, type SearchSort } from "../../api/content";
import { CURSEFORGE_KEY, settingsApi } from "../../api/settings";
import {
  Badge,
  Button,
  EmptyState,
  Notice,
  ProjectCard,
  SearchInput,
  SegmentedControl,
  Select,
  Spinner,
  SplitView,
  Stack,
} from "../../ui";
import { hitLoaders, hitVersion } from "../server/content/content";
import { CatalogDetail, type CatalogActionContext } from "./CatalogDetail";

type Source = "modrinth" | "curseforge";
export type CatalogKind = "mod" | "plugin" | "modpack";

interface CatalogBrowserProps {
  kind: CatalogKind;
  scope: CatalogScope;
  /** Loader và Minecraft ưu tiên hiện trên thẻ kết quả. */
  loader?: string | null;
  minecraft?: string | null;
  /** Trả về `true` nếu mục đã có trong danh sách chờ để hiện nhãn trên thẻ kết quả. */
  isQueued?: (hit: SearchHit) => boolean;
  renderAction: (ctx: CatalogActionContext) => ReactNode;
}

const SORTS: SearchSort[] = ["relevance", "downloads", "updated"];

function searchLabelKey(kind: CatalogKind, source: Source): string {
  const base = kind === "modpack" ? "modpack" : kind === "plugin" ? "content.plugins" : "content.mods";
  return `${base}.${source === "curseforge" ? "searchPlaceholderCurseforge" : "searchPlaceholder"}`;
}

/**
 * Trình duyệt catalog dùng chung (spec trình duyệt mod, mục 3): danh sách kết quả bên trái, chi tiết
 * bên phải. Nơi dùng quyết định nút hành động trong khung chi tiết.
 */
export function CatalogBrowser({ kind, scope, loader, minecraft, isQueued, renderAction }: CatalogBrowserProps) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<SearchHit | null>(null);

  return (
    <SplitView
      detailOpen={selected !== null}
      backLabel={t("catalog.backToResults")}
      onBack={() => setSelected(null)}
      list={
        <SearchPane
          kind={kind}
          scope={scope}
          loader={loader}
          minecraft={minecraft}
          isQueued={isQueued}
          selected={selected}
          onSelect={setSelected}
        />
      }
      detail={
        selected ? (
          <CatalogDetail
            key={`${selected.source}:${selected.project_id}`}
            hit={selected}
            scope={scope}
            loader={loader ?? scope.loader}
            showTarget={kind === "modpack"}
            renderAction={renderAction}
          />
        ) : (
          <EmptyState icon={Package} title={t("catalog.pickHint")} />
        )
      }
    />
  );
}

interface SearchPaneProps {
  kind: CatalogKind;
  scope: CatalogScope;
  loader?: string | null;
  minecraft?: string | null;
  isQueued?: (hit: SearchHit) => boolean;
  selected: SearchHit | null;
  onSelect: (hit: SearchHit | null) => void;
}

function SearchPane({ kind, scope, loader, minecraft, isQueued, selected, onSelect }: SearchPaneProps) {
  const { t } = useTranslation();
  const [source, setSource] = useState<Source>("modrinth");
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");
  const [sort, setSort] = useState<SearchSort>("relevance");

  useEffect(() => {
    const trimmed = query.trim();
    const timer = window.setTimeout(() => setSubmitted(trimmed), trimmed === "" ? 0 : 300);
    return () => window.clearTimeout(timer);
  }, [query]);

  // Plugin chỉ có trên Modrinth trong P3.
  const allowCurseforge = kind !== "plugin";
  const cf = useQuery({ queryKey: CURSEFORGE_KEY, queryFn: settingsApi.curseforge, enabled: allowCurseforge });
  const cfAvailable = cf.data?.available === true;
  const active: Source = allowCurseforge && cfAvailable ? source : "modrinth";

  const search = useInfiniteQuery({
    queryKey: ["catalog-search", kind, active, sort, submitted, scope],
    queryFn: ({ pageParam }) => contentApi.search({ source: active, kind, q: submitted, page: pageParam, sort, ...scope }),
    initialPageParam: 1,
    getNextPageParam: (last) => (last.page * last.page_size < last.total ? last.page + 1 : undefined),
  });
  const hits = search.data?.pages.flatMap((page) => page.hits) ?? [];

  return (
    <Stack gap={3}>
      <Stack direction="row" justify="between" align="center" gap={2} wrap>
        {allowCurseforge && (
          <SegmentedControl
            label={t("content.addDialog.sourceLabel")}
            value={active}
            onChange={(next) => {
              const nextSource: Source = next === "curseforge" ? "curseforge" : "modrinth";
              if (nextSource !== source) onSelect(null);
              setSource(nextSource);
            }}
            options={[
              { value: "modrinth", label: t("content.addDialog.sourceModrinth") },
              {
                value: "curseforge",
                label: t("content.addDialog.sourceCurseforge"),
                disabled: !cfAvailable,
                title: cfAvailable ? undefined : t("content.addDialog.curseforgeUnavailableHint"),
              },
            ]}
          />
        )}
        <Select aria-label={t("catalog.sort")} value={sort} onChange={(e) => setSort(e.target.value as SearchSort)}>
          {SORTS.map((value) => (
            <option key={value} value={value}>
              {t(`catalog.sorts.${value}`)}
            </option>
          ))}
        </Select>
      </Stack>
      {allowCurseforge && cf.isSuccess && !cfAvailable && <Notice tone="info">{t("content.addDialog.curseforgeUnavailableHint")}</Notice>}
      <div
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            setSubmitted(query.trim());
          }
        }}
      >
        <SearchInput value={query} onChange={setQuery} label={t(searchLabelKey(kind, active))} autoFocus />
      </div>
      {search.isPending ? (
        <Spinner label={t("ui.loading")} />
      ) : search.isError ? (
        <Notice
          tone="danger"
          action={
            <Button size="sm" icon={RotateCw} onClick={() => void search.refetch()}>
              {t("common.retry")}
            </Button>
          }
        >
          {errorMessage(t, search.error)}
        </Notice>
      ) : hits.length === 0 ? (
        <EmptyState icon={Package} title={t("catalog.noResults")} description={t("catalog.noResultsHint")} />
      ) : (
        <Stack gap={2}>
          {hits.map((hit) => (
            <ProjectCard
              key={`${hit.source}:${hit.project_id}`}
              title={hit.title}
              description={hit.description}
              iconUrl={hit.icon_url}
              downloads={hit.downloads}
              loaders={hitLoaders(hit.categories, loader)}
              version={hitVersion(hit.game_versions, minecraft)}
              badge={isQueued?.(hit) ? <Badge tone="success">{t("catalog.queue.selected")}</Badge> : undefined}
              selected={selected?.source === hit.source && selected.project_id === hit.project_id}
              onSelect={() => onSelect(hit)}
            />
          ))}
          {search.hasNextPage && (
            <div>
              <Button variant="secondary" loading={search.isFetchingNextPage} onClick={() => void search.fetchNextPage()}>
                {t("content.addDialog.loadMore")}
              </Button>
            </div>
          )}
        </Stack>
      )}
    </Stack>
  );
}
