import { blockSlice, PLAINS } from "./blockSlice";
import styles from "./BlockBanner.module.css";
import { cx } from "./cx";

interface BlockBannerProps {
  /** Chuỗi sinh hình, thường là id server. */
  seed: string;
  /** `banner`: dải ngang đầu thẻ. `mark`: ô vuông nhỏ cạnh tên server. `logo`: như `mark`, luôn là đất có cỏ. */
  variant?: "banner" | "mark" | "logo";
}

const SIZE = { banner: { columns: 40, rows: 6 }, mark: { columns: 4, rows: 4 }, logo: { columns: 4, rows: 4 } };

/** Hoa văn khối riêng của mỗi server (spec làm mới giao diện). Chỉ để trang trí, trình đọc màn hình bỏ qua. */
export function BlockBanner({ seed, variant = "banner" }: BlockBannerProps) {
  const { columns, rows } = SIZE[variant];
  const grid = blockSlice(seed, columns, rows, { biome: variant === "logo" ? PLAINS : undefined, flat: variant !== "banner" });
  return (
    <svg
      className={cx(styles.root, variant === "banner" ? styles.banner : styles.mark)}
      viewBox={`0 0 ${columns} ${rows}`}
      preserveAspectRatio="none"
      shapeRendering="crispEdges"
      aria-hidden
      focusable="false"
    >
      {grid.flatMap((row, y) => row.map((fill, x) => fill && <rect key={`${x}-${y}`} x={x} y={y} width={1.02} height={1.02} fill={fill} />))}
    </svg>
  );
}
