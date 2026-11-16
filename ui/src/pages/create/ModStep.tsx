import { Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";

import type { Loader } from "../../api/client";
import { Avatar, Badge, Button, Card, IconButton, Stack } from "../../ui";
import { CatalogBrowser } from "../catalog/CatalogBrowser";
import type { CatalogActionContext } from "../catalog/CatalogDetail";
import { MAX_QUEUE, type ContentQueue } from "../catalog/queue";
import { versionOptionLabel } from "../server/content/content";

interface ModStepProps {
  loader: Loader;
  minecraft: string;
  queue: ContentQueue;
  /** Đã xóa danh sách vì người dùng quay lại đổi loader hoặc Minecraft. */
  cleared: boolean;
}

/**
 * Bước "Mod" khi tạo server có mod (spec trình duyệt mod, mục 5): chọn mod hợp loader và Minecraft
 * vừa chọn; MineHost cài chúng ngay sau khi tạo server. Bước này không bắt buộc.
 */
export function ModStep({ loader, minecraft, queue, cleared }: ModStepProps) {
  const { t } = useTranslation();

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
          onClick={() =>
            queue.add({
              source: hit.source,
              project_id: hit.project_id,
              version_id: version.id,
              title: hit.title,
              version_label: versionOptionLabel(version),
              icon_url: hit.icon_url,
            })
          }
        >
          {t("catalog.queue.add")}
        </Button>
      </div>
    );
  };

  return (
    <Stack gap={4}>
      <p>{t("wizard.mods.hint")}</p>
      {cleared && <p>{t("wizard.mods.cleared")}</p>}
      <Card title={t("wizard.mods.chosen", { count: queue.items.length })}>
        {queue.items.length === 0 ? (
          <p>{t("wizard.mods.none")}</p>
        ) : (
          <Stack gap={2}>
            {queue.items.map((item) => (
              <Stack key={`${item.source}:${item.project_id}`} direction="row" gap={2} align="center" wrap>
                <Avatar name={item.title} imageUrl={item.icon_url} size="sm" />
                <strong>{item.title}</strong>
                {item.version_label && <Badge>{item.version_label}</Badge>}
                <IconButton
                  icon={X}
                  size="sm"
                  label={t("catalog.queue.remove", { name: item.title })}
                  onClick={() => queue.remove(item.source, item.project_id)}
                />
              </Stack>
            ))}
          </Stack>
        )}
      </Card>
      <CatalogBrowser
        kind="mod"
        scope={{ loader, minecraft }}
        loader={loader}
        minecraft={minecraft}
        renderAction={renderAction}
      />
    </Stack>
  );
}
