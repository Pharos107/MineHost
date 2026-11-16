import { useQuery } from "@tanstack/react-query";
import { ExternalLink, Package } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { errorMessage } from "../../api/client";
import { contentApi, type CatalogScope, type ContentVersion, type SearchHit } from "../../api/content";
import { Avatar, Badge, Field, Notice, RichText, Select, Spinner, Stack } from "../../ui";
import { hitLoaders, versionOptionLabel } from "../server/content/content";

export interface CatalogActionContext {
  hit: SearchHit;
  /** Bản đang chọn; `null` khi chưa tải xong hoặc không có bản hợp. */
  version: ContentVersion | null;
}

interface CatalogDetailProps {
  hit: SearchHit;
  scope: CatalogScope;
  /** Modpack: ghi kèm Minecraft và loader của từng bản. */
  showTarget: boolean;
  renderAction: (ctx: CatalogActionContext) => ReactNode;
}

/** Khung chi tiết: thông tin, chọn bản, nút hành động, link và mô tả đầy đủ (spec trình duyệt mod, mục 3). */
export function CatalogDetail({ hit, scope, showTarget, renderAction }: CatalogDetailProps) {
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
      <Stack direction="row" gap={3} align="center">
        <Avatar name={hit.title} imageUrl={hit.icon_url} icon={Package} size="lg" />
        <Stack gap={1}>
          <strong>
            {details?.links.page ? (
              <a href={details.links.page} target="_blank" rel="noopener noreferrer">
                {hit.title} <ExternalLink size={14} aria-hidden />
              </a>
            ) : (
              hit.title
            )}
          </strong>
          <span>{hit.description}</span>
          <Stack direction="row" gap={2} wrap align="center">
            <Badge>{t("ui.downloads", { value: downloads })}</Badge>
            {hitLoaders(hit.categories).map((loader) => (
              <Badge key={loader}>{loader}</Badge>
            ))}
            {hit.server_side === "unsupported" && <Badge tone="warning">{t("content.clientOnly")}</Badge>}
          </Stack>
        </Stack>
      </Stack>
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
      {links.length > 0 && (
        <Stack direction="row" gap={3} wrap>
          {links.map((link) => (
            <a key={link.key} href={link.url} target="_blank" rel="noopener noreferrer">
              {t(`catalog.links.${link.key}`)}
            </a>
          ))}
        </Stack>
      )}
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
