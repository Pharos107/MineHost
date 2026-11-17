import { Plus } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { Loader } from "../../api/client";
import { Badge, Button, Card, Notice, ProjectChip, Stack } from "../../ui";
import { CatalogBrowser } from "../catalog/CatalogBrowser";
import type { CatalogActionContext } from "../catalog/CatalogDetail";
import { MAX_QUEUE, type ContentQueue } from "../catalog/queue";
import { versionOptionLabel } from "../server/content/content";
import { isPluginLoader } from "./form";

interface ModStepProps {
  loader: Loader;
  minecraft: string;
  queue: ContentQueue;
  /** Đã xóa danh sách vì người dùng quay lại đổi loader hoặc Minecraft. */
  cleared: boolean;
  /** Gọi khi người dùng thêm mục mới để ẩn thông báo đã xóa cũ. */
  onAdded?: () => void;
}

/**
 * Bước "Mod" / "Plugin" khi tạo server (spec trình duyệt mod, mục 5): chọn mod/plugin hợp loader
 * và Minecraft vừa chọn; MineHost cài chúng ngay sau khi tạo server. Bước này không bắt buộc.
 */
export function ModStep({ loader, minecraft, queue, cleared, onAdded }: ModStepProps) {
  const { t } = useTranslation();
  const kind = isPluginLoader(loader) ? "plugin" : "mod";
  const prefix = kind === "plugin" ? "wizard.plugins" : "wizard.mods";

  const renderAction = ({ hit, version }: CatalogActionContext) => {
    if (version === null) return null;
    if (queue.has(hit.source, hit.project_id)) {
      return (
        <Stack direction="row" gap={2} align="center" wrap>
          <Badge tone="success">{t("catalog.queue.added")}</Badge>
          <Button size="sm" variant="ghost" onClick={() => queue.remove(hit.source, hit.project_id)}>
            {t("catalog.queue.undo")}
          </Button>
        </Stack>
      );
    }
    return (
      <div>
        <Button
          variant="primary"
          icon={Plus}
          disabled={queue.items.length >= MAX_QUEUE}
          onClick={() => {
            queue.add({
              source: hit.source,
              project_id: hit.project_id,
              version_id: version.id,
              title: hit.title,
              version_label: versionOptionLabel(version),
              icon_url: hit.icon_url,
            });
            onAdded?.();
          }}
        >
          {t("catalog.queue.add")}
        </Button>
      </div>
    );
  };

  return (
    <Stack gap={4}>
      <span>{t(`${prefix}.hint`)}</span>
      {cleared && queue.items.length === 0 && <Notice tone="info">{t(`${prefix}.cleared`)}</Notice>}
      <Card variant="subtle">
        <Stack direction="row" justify="between" align="center" gap={2} wrap>
          <Stack direction="row" align="center" gap={2} wrap>
            <strong>{t(`${prefix}.chosen`, { count: queue.items.length })}</strong>
            <Badge tone="primary">{t(`loaders.${loader}`)}</Badge>
            <Badge>{minecraft}</Badge>
          </Stack>
          {queue.items.length === 0 ? (
            <span>{t(`${prefix}.none`)}</span>
          ) : (
            <Button size="sm" variant="ghost" onClick={queue.clear}>
              {t("wizard.mods.clearAll")}
            </Button>
          )}
        </Stack>
        {queue.items.length > 0 && (
          <Stack direction="row" gap={2} wrap>
            {queue.items.map((item) => (
              <ProjectChip
                key={`${item.source}:${item.project_id}`}
                title={item.title}
                iconUrl={item.icon_url}
                version={item.version_label}
                removeLabel={t("catalog.queue.remove", { name: item.title })}
                onRemove={() => queue.remove(item.source, item.project_id)}
              />
            ))}
          </Stack>
        )}
      </Card>
      <CatalogBrowser
        kind={kind}
        scope={{ loader, minecraft }}
        loader={loader}
        minecraft={minecraft}
        isQueued={(hit) => queue.has(hit.source, hit.project_id)}
        renderAction={renderAction}
      />
    </Stack>
  );
}
