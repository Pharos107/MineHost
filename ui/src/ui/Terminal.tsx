import type { ITheme, Terminal as XTerm } from "@xterm/xterm";
import { useEffect, useImperativeHandle, useRef, type Ref } from "react";

import styles from "./Terminal.module.css";

export type TerminalTone = "normal" | "muted" | "primary" | "danger";

export interface TerminalHandle {
  /** Ghi một dòng; `tone` tô màu cả dòng (lệnh người dùng gõ, stderr…). */
  write: (text: string, tone?: TerminalTone) => void;
  clear: () => void;
}

const ANSI: Record<TerminalTone, string> = { normal: "", muted: "\x1b[90m", primary: "\x1b[36m", danger: "\x1b[31m" };
const RESET = "\x1b[0m";

interface TerminalProps {
  /** Tên vùng log cho trình đọc màn hình. */
  label: string;
  ref?: Ref<TerminalHandle>;
  /** Số dòng giữ lại để cuộn xem (spec P1: 5.000). */
  maxLines?: number;
}

/** Màu của xterm lấy từ token, để console theo đúng bảng màu sáng/tối của app. */
function themeFrom(element: HTMLElement): ITheme {
  const css = getComputedStyle(element);
  const token = (name: string) => css.getPropertyValue(name).trim();
  return {
    background: token("--mh-surface"),
    foreground: token("--mh-text"),
    cursor: token("--mh-surface"),
    selectionBackground: token("--mh-primary-soft"),
    brightBlack: token("--mh-text-muted"),
    cyan: token("--mh-primary"),
    red: token("--mh-danger"),
    yellow: token("--mh-warning"),
    green: token("--mh-success"),
  };
}

async function loadXterm() {
  // Tải riêng khi mở console, để trang danh sách và wizard không phải tải xterm.
  const [{ Terminal }, { FitAddon }] = await Promise.all([
    import("@xterm/xterm"),
    import("@xterm/addon-fit"),
    import("@xterm/xterm/css/xterm.css"),
  ]);
  return { Terminal, FitAddon };
}

/** Vùng log chỉ đọc dựa trên xterm.js: hiểu mã màu ANSI của server, cuộn mượt với hàng nghìn dòng. */
export function Terminal({ label, ref, maxLines = 5000 }: TerminalProps) {
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<XTerm | null>(null);
  // Dòng đến trước khi xterm tải xong được giữ lại rồi ghi một lượt.
  const pending = useRef<string[]>([]);

  useImperativeHandle(
    ref,
    () => ({
      write: (text, tone = "normal") => {
        const line = tone === "normal" ? text : `${ANSI[tone]}${text}${RESET}`;
        if (term.current) term.current.writeln(line);
        else pending.current.push(line);
      },
      clear: () => {
        pending.current = [];
        term.current?.clear();
      },
    }),
    [],
  );

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let cleanup: (() => void) | undefined;
    let cancelled = false;

    void loadXterm().then(({ Terminal: XTermClass, FitAddon }) => {
      if (cancelled) return;
      const xterm = new XTermClass({
        scrollback: maxLines,
        convertEol: true,
        disableStdin: true,
        fontFamily: getComputedStyle(element).getPropertyValue("--mh-font-mono").trim(),
        fontSize: 13,
        theme: themeFrom(element),
      });
      const fit = new FitAddon();
      xterm.loadAddon(fit);
      xterm.open(element);
      fit.fit();
      pending.current.splice(0).forEach((line) => xterm.writeln(line));
      term.current = xterm;

      const resize = new ResizeObserver(() => fit.fit());
      resize.observe(element);
      // Đổi chế độ sáng/tối của hệ điều hành khi đang mở trang thì đổi màu theo.
      const scheme = window.matchMedia("(prefers-color-scheme: dark)");
      const retheme = () => (xterm.options.theme = themeFrom(element));
      scheme.addEventListener("change", retheme);
      cleanup = () => {
        scheme.removeEventListener("change", retheme);
        resize.disconnect();
        xterm.dispose();
        term.current = null;
      };
    });
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [maxLines]);

  return <div className={styles.terminal} ref={host} role="log" aria-label={label} />;
}
