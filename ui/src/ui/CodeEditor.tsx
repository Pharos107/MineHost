import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import styles from "./CodeEditor.module.css";
import type { CodeLanguage } from "./codeLanguage";
import { TextArea } from "./inputs";
import { Spinner } from "./Spinner";

type CodeMirror = typeof import("./codemirror");
type View = InstanceType<CodeMirror["EditorView"]>;

interface CodeEditorProps {
  /** Tên vùng soạn cho trình đọc màn hình. */
  label: string;
  value: string;
  onChange: (text: string) => void;
  language: CodeLanguage;
  /** Tự xuống dòng khi dòng dài hơn khung. */
  wrap?: boolean;
  /** Ctrl+S (Cmd+S) khi đang gõ trong trình sửa. */
  onSave?: () => void;
  onCursor?: (line: number, column: number) => void;
  /** Thanh trạng thái dưới trình sửa; mục đầu nằm bên trái, còn lại bên phải. */
  status?: ReactNode;
}

/** Câu chữ của CodeMirror (khóa là câu gốc tiếng Anh) → khóa i18n. */
const PHRASES: Record<string, string> = {
  Find: "find",
  Replace: "replace",
  next: "next",
  previous: "previous",
  all: "all",
  "match case": "matchCase",
  regexp: "regexp",
  "by word": "byWord",
  replace: "replaceOne",
  "replace all": "replaceAll",
  close: "close",
  "Go to line": "goToLine",
  go: "go",
  "Fold line": "foldLine",
  "Unfold line": "unfoldLine",
  "folded code": "foldedCode",
  unfold: "unfold",
};

/**
 * Trình sửa code dựa trên CodeMirror 6 (spec trang Tệp, mục 1): số dòng, tô màu theo ngôn ngữ,
 * tìm và thay, gập khối. CodeMirror tải riêng khi component hiện; tải lỗi thì dùng ô văn bản thường.
 */
export function CodeEditor({ label, value, onChange, language, wrap = false, onSave, onCursor, status }: CodeEditorProps) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<View | null>(null);
  const cm = useRef<CodeMirror | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");
  // Hàm mới nhất của props; CodeMirror giữ callback lúc dựng nên đọc qua ref.
  const latest = useRef({ onChange, onSave, onCursor });
  useEffect(() => {
    latest.current = { onChange, onSave, onCursor };
  });
  const initial = useRef({ value, wrap });

  useEffect(() => {
    let disposed = false;
    import("./codemirror")
      .then((mod) => {
        if (disposed || !host.current) return;
        const phrases = Object.fromEntries(Object.entries(PHRASES).map(([en, key]) => [en, t(`ui.codeEditor.${key}`)]));
        const editorState = mod.createState(
          {
            doc: initial.current.value,
            language,
            label,
            phrases,
            onChange: (text) => latest.current.onChange(text),
            onCursor: (line, column) => latest.current.onCursor?.(line, column),
            onSave: () => latest.current.onSave?.(),
          },
          initial.current.wrap,
        );
        cm.current = mod;
        view.current = new mod.EditorView({ state: editorState, parent: host.current });
        setState("ready");
      })
      .catch(() => {
        if (!disposed) setState("failed");
      });
    return () => {
      disposed = true;
      view.current?.destroy();
      view.current = null;
    };
    // Dựng lại khi đổi ngôn ngữ hoặc nhãn; nội dung và xuống dòng cập nhật ở hai effect dưới.
  }, [language, label, t]);

  // Nội dung đổi từ ngoài (tải lại bản mới trên đĩa): thay cả văn bản.
  useEffect(() => {
    const current = view.current;
    if (current && current.state.doc.toString() !== value) {
      current.dispatch({ changes: { from: 0, to: current.state.doc.length, insert: value } });
    }
    initial.current.value = value;
  }, [value]);

  useEffect(() => {
    const mod = cm.current;
    view.current?.dispatch({ effects: mod?.wrapCompartment.reconfigure(mod.wrapExtension(wrap)) });
    initial.current.wrap = wrap;
  }, [wrap, state]);

  const bar = status && <div className={styles.status}>{status}</div>;
  if (state === "failed") {
    return (
      <div>
        <TextArea
          variant="code"
          aria-label={label}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
              e.preventDefault();
              onSave?.();
            }
          }}
        />
        {bar}
      </div>
    );
  }
  return (
    <div className={styles.frame}>
      {state === "loading" && (
        <div className={styles.loading}>
          <Spinner label={t("ui.loading")} />
        </div>
      )}
      <div ref={host} className={styles.host} />
      {bar}
    </div>
  );
}
