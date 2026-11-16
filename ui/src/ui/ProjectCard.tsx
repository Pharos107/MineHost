import { Download, ExternalLink, Package, X } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Avatar } from "./Avatar";
import { Badge } from "./Badge";
import { IconButton } from "./Button";
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
  /** Nhãn trạng thái bổ sung (ví dụ "Đã chọn"). */
  badge?: ReactNode;
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
  badge,
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
        {(loaders?.length || version || badge) && (
          <span className={styles.tags}>
            {loaders?.map((loader) => (
              <Badge key={loader}>{loader}</Badge>
            ))}
            {version && <Badge tone="primary">{version}</Badge>}
            {badge}
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

export interface ProjectChipProps {
  title: string;
  iconUrl?: string | null;
  version?: string | null;
  removeLabel: string;
  onRemove: () => void;
}

/** Thẻ viên thuốc gọn hiển thị mod đã chọn trong danh sách chờ cài. */
export function ProjectChip({ title, iconUrl, version, removeLabel, onRemove }: ProjectChipProps) {
  return (
    <span className={styles.chip}>
      <Avatar name={title} imageUrl={iconUrl} icon={Package} size="sm" />
      <strong className={styles.chipTitle}>{title}</strong>
      {version && <Badge>{version}</Badge>}
      <IconButton icon={X} size="sm" label={removeLabel} onClick={onRemove} />
    </span>
  );
}

export interface ProjectLinkItem {
  key: string;
  label: string;
  url: string;
}

export interface ProjectDetailHeaderProps {
  title: string;
  description: string;
  iconUrl?: string | null;
  pageUrl?: string | null;
  badges?: ReactNode;
  links?: ProjectLinkItem[];
}

/** Phần đầu khung chi tiết mod / modpack: ảnh, tiêu đề, mô tả, nhãn và link ngoài. */
export function ProjectDetailHeader({ title, description, iconUrl, pageUrl, badges, links }: ProjectDetailHeaderProps) {
  return (
    <header className={styles.detailHeader}>
      <div className={styles.detailTop}>
        <Avatar name={title} imageUrl={iconUrl} icon={Package} size="lg" />
        <div className={styles.detailMeta}>
          <div className={styles.detailTitle}>
            {pageUrl ? (
              <a className={styles.detailTitleLink} href={pageUrl} target="_blank" rel="noopener noreferrer">
                {title}
                <ExternalLink size={14} aria-hidden />
              </a>
            ) : (
              title
            )}
          </div>
          <p className={styles.detailDescription}>{description}</p>
          {badges && <div className={styles.detailBadges}>{badges}</div>}
        </div>
      </div>
      {links && links.length > 0 && (
        <div className={styles.detailLinks}>
          {links.map((link) => (
            <a key={link.key} className={styles.linkPill} href={link.url} target="_blank" rel="noopener noreferrer">
              {link.label}
              <ExternalLink size={12} aria-hidden />
            </a>
          ))}
        </div>
      )}
    </header>
  );
}
