import type { CSSProperties, ReactNode } from "react";

export type Space = 1 | 2 | 3 | 4 | 5 | 6 | 8;

const ALIGN = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch", baseline: "baseline" };
const JUSTIFY = { start: "flex-start", center: "center", end: "flex-end", between: "space-between" };

interface StackProps {
  direction?: "row" | "column";
  gap?: Space;
  align?: keyof typeof ALIGN;
  justify?: keyof typeof JUSTIFY;
  wrap?: boolean;
  children: ReactNode;
}

/** Xếp phần tử theo hàng hoặc cột với khoảng cách lấy từ token, để trang không tự viết flex. */
export function Stack({ direction = "column", gap = 4, align, justify, wrap = false, children }: StackProps) {
  const style: CSSProperties = {
    display: "flex",
    flexDirection: direction,
    gap: `var(--mh-space-${gap})`,
    alignItems: align ? ALIGN[align] : undefined,
    justifyContent: justify ? JUSTIFY[justify] : undefined,
    flexWrap: wrap ? "wrap" : undefined,
  };
  return <div style={style}>{children}</div>;
}
