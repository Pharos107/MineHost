import DOMPurify from "dompurify";
import { marked } from "marked";
import { useMemo } from "react";

import styles from "./RichText.module.css";

// Dùng instance riêng để hook dưới đây không ảnh hưởng chỗ khác gọi DOMPurify.
const purifier = DOMPurify(window);

purifier.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    const href = node.getAttribute("href") ?? "";
    if (!/^https?:\/\//i.test(href)) node.removeAttribute("href");
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noopener noreferrer");
  }
  // Ảnh chỉ nhận https: ảnh http làm trang báo nội dung không an toàn, data: có thể rất lớn.
  if (node.tagName === "IMG") {
    const src = node.getAttribute("src") ?? "";
    if (!/^https:\/\//i.test(src)) node.remove();
    else node.setAttribute("loading", "lazy");
  }
});

const FORBID_TAGS = ["style", "form", "input", "button", "textarea", "select", "iframe", "object", "embed", "video", "audio"];

/** Lọc HTML của mô tả mod: chỉ giữ thẻ định dạng, bảng, ảnh https và link http(s). */
export function sanitizeHtml(html: string): string {
  return purifier.sanitize(html, { FORBID_TAGS, FORBID_ATTR: ["style", "class", "id"] });
}

/** Markdown (Modrinth) sang HTML đã lọc. */
export function markdownToHtml(markdown: string): string {
  return sanitizeHtml(marked.parse(markdown, { async: false, gfm: true }));
}

interface RichTextProps {
  source: string;
  /** Modrinth trả markdown, CurseForge trả HTML. */
  format: "markdown" | "html";
  label?: string;
}

/** Mô tả đầy đủ của mod hoặc modpack, đã lọc HTML, theo kiểu chữ của bộ component. */
export function RichText({ source, format, label }: RichTextProps) {
  const html = useMemo(() => (format === "markdown" ? markdownToHtml(source) : sanitizeHtml(source)), [source, format]);
  // HTML đã qua sanitizeHtml ở trên nên chèn trực tiếp được.
  return <div className={styles.rich} aria-label={label} dangerouslySetInnerHTML={{ __html: html }} />;
}
