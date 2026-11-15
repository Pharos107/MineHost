import { Download, Package } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "./Avatar";
import { Badge } from "./Badge";
import { cx } from "./cx";
import styles from "./ProjectCard.module.css";

export interface ProjectCardProps {
  title: string;
  description: string;
  iconUrl?: string | null;
  downloads: number;
  /** Nhãn loader hỗ trợ (ví dụ `["Fabric", "Quilt"]`). */
  loaders?: string[];
  /** Phiên bản Minecraft hiển thị cạnh nhãn loader. */
  version?: string;
  /** Có thì cả thẻ bấm được để chọn mod/modpack. */
  onSelect?: () => void;
  selected?: boolean;
  /** Nút ở chân thẻ khi không dùng chế độ bấm cả thẻ. */
  actions?: ReactNode;
}

/** Thẻ kết quả tìm kiếm mod hoặc modpack (spec P3, mục 6.6). */
export function ProjectCard({
  title,
  description,
  iconUrl,
  downloads,
  loaders,
  version,
  onSelect,
  selected,
  actions,
}: ProjectCardProps) {
  const { t, i18n } = useTranslation();
  const downloadsText = new Intl.NumberFormat(i18n.language, {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(downloads);

  const inner = (
    <span className={styles.row}>
      <Avatar name={title} imageUrl={iconUrl} icon={Package} size="lg" />
      <span className={styles.body}>
        <span className={styles.top}>
          <span className={styles.title}>{title}</span>
          <span className={styles.downloads} aria-label={t("ui.downloads", { value: downloadsText })}>
            <Download size={14} aria-hidden />
            {downloadsText}
          </span>
        </span>
        <span className={styles.description} title={description}>
          {description}
        </span>
        {(loaders?.length || version) && (
          <span className={styles.tags}>
            {loaders?.map((loader) => (
              <Badge key={loader}>{loader}</Badge>
            ))}
            {version && <Badge tone="primary">{version}</Badge>}
          </span>
        )}
      </span>
    </span>
  );

  if (onSelect) {
    return (
      <button
        type="button"
        className={cx(styles.card, styles.interactive, selected && styles.selected)}
        aria-pressed={selected}
        onClick={onSelect}
      >
        {inner}
      </button>
    );
  }

  return (
    <article className={styles.card}>
      {inner}
      {actions && <footer className={styles.actions}>{actions}</footer>}
    </article>
  );
}

/** Lưới thẻ dự án, tự xuống hàng theo độ rộng. */
export function ProjectCardGrid({ children }: { children: ReactNode }) {
  return <div className={styles.grid}>{children}</div>;
}
