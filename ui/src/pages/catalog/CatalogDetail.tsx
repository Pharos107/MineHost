import { useQuery } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../api/client";
import { contentApi, type CatalogScope, type ContentVersion, type SearchHit } from "../../api/content";
import { Badge, Card, Field, Notice, ProjectDetailHeader, RichText, Select, Spinner, Stack } from "../../ui";
import { hitLoaders, versionOptionLabel } from "../server/content/content";

export interface CatalogActionContext {
  hit: SearchHit;
  /** Bản đang chọn; `null` khi chưa tải xong hoặc không có bản hợp. */
  version: ContentVersion | null;
}

interface CatalogDetailProps {
  hit: SearchHit;
  scope: CatalogScope;
  loader?: string | null;
  /** Modpack: ghi kèm Minecraft và loader của từng bản. */
  showTarget: boolean;
  renderAction: (ctx: CatalogActionContext) => ReactNode;
}

/** Khung chi tiết: thông tin, chọn bản, nút hành động, link và mô tả đầy đủ (spec trình duyệt mod, mục 3). */
export function CatalogDetail({ hit, scope, loader, showTarget, renderAction }: CatalogDetailProps) {
  const { t, i18n } = useTranslation();
  const [versionId, setVersionId] = useState<string | null>(null);
  const project = useQuery({
    queryKey: ["catalog-project", hit.source, hit.project_id],
    queryFn: () => contentApi.project(hit.source, hit.project_id),
  });
  const versions = useQuery({
    queryKey: ["catalog-versions", hit.source, hit.project_id, scope],
    queryFn: () => contentApi.versions(hit.source, hit.project_id, scope),
  });
  const list = versions.data ?? [];
  const version = list.find((v) => v.id === versionId) ?? list[0] ?? null;
  const date = new Intl.DateTimeFormat(i18n.language, { dateStyle: "short" });
  const downloads = new Intl.NumberFormat(i18n.language, { notation: "compact", maximumFractionDigits: 1 }).format(hit.downloads);
  const details = project.data;
  const links = details
    ? [
        { key: "source", url: details.links.source },
        { key: "wiki", url: details.links.wiki },
        { key: "issues", url: details.links.issues },
        { key: "discord", url: details.links.discord },
      ].filter((l): l is { key: string; url: string } => typeof l.url === "string" && /^https?:\/\//i.test(l.url))
    : [];

  const label = (v: ContentVersion, index: number) => {
    const base = `${versionOptionLabel(v)} · ${date.format(new Date(v.date_published))}`;
    const target = showTarget ? ` · ${[v.game_versions[0], ...hitLoaders(v.loaders)].filter(Boolean).join(" ")}` : "";
    return index === 0 ? t("catalog.latest", { version: base + target }) : base + target;
  };

  return (
    <Stack gap={4}>
      <ProjectDetailHeader
        title={hit.title}
        description={hit.description}
        iconUrl={hit.icon_url}
        pageUrl={details?.links.page}
        badges={
          <>
            <Badge>{t("ui.downloads", { value: downloads })}</Badge>
            {hitLoaders(details?.loaders.length ? details.loaders : hit.categories, loader ?? scope.loader).map((item) => (
              <Badge key={item}>{item}</Badge>
            ))}
            {hit.server_side === "unsupported" && <Badge tone="warning">{t("content.clientOnly")}</Badge>}
          </>
        }
        links={links.map((link) => ({
          key: link.key,
          label: t(`catalog.links.${link.key}`),
          url: link.url,
        }))}
      />
      <Card>
        {versions.isPending ? (
          <Spinner label={t("ui.loading")} />
        ) : versions.isError ? (
          <Notice tone="danger">{errorMessage(t, versions.error)}</Notice>
        ) : list.length === 0 ? (
          <Notice tone="warning">{t("catalog.noVersions")}</Notice>
        ) : (
          <Field label={t("catalog.version")}>
            <Select value={version?.id ?? ""} onChange={(e) => setVersionId(e.target.value)}>
              {list.map((v, index) => (
                <option key={v.id} value={v.id}>
                  {label(v, index)}
                </option>
              ))}
            </Select>
          </Field>
        )}
        {renderAction({ hit, version })}
      </Card>
      {project.isPending ? (
        <Spinner label={t("ui.loading")} />
      ) : project.isError ? (
        <Notice tone="danger">{errorMessage(t, project.error)}</Notice>
      ) : details?.body ? (
        <RichText
          source={details.body}
          format={hit.source === "curseforge" ? "html" : "markdown"}
          label={t("catalog.description")}
        />
      ) : null}
    </Stack>
  );
}
