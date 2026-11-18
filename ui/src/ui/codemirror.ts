// Phần CodeMirror của `CodeEditor`. Chỉ được import động để nằm trong chunk riêng (spec trang Tệp,
// tiêu chí 2): trang danh sách server, wizard không phải tải trình sửa.
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  HighlightStyle,
  indentOnInput,
  indentUnit,
  StreamLanguage,
  syntaxHighlighting,
  type LanguageSupport,
} from "@codemirror/language";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";

import type { CodeLanguage } from "./codeLanguage";

export { EditorView };

/** Màu lấy thẳng từ biến CSS nên tự đổi theo giao diện sáng / tối, không cần dựng lại trình sửa. */
const theme = EditorView.theme({
  "&": { color: "var(--mh-text)", backgroundColor: "var(--mh-input-bg)", height: "100%", fontSize: "var(--mh-text-sm)" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--mh-font-mono)", lineHeight: "1.6" },
  ".cm-content": { caretColor: "var(--mh-text)", padding: "var(--mh-space-2) 0" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--mh-text)" },
  ".cm-gutters": { backgroundColor: "var(--mh-surface-raised)", color: "var(--mh-text-muted)", border: "none" },
  ".cm-activeLine": { backgroundColor: "color-mix(in srgb, var(--mh-primary) 7%, transparent)" },
  ".cm-activeLineGutter": { backgroundColor: "var(--mh-surface-hover)", color: "var(--mh-text)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "color-mix(in srgb, var(--mh-primary) 28%, transparent)",
  },
  ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--mh-info) 20%, transparent)" },
  ".cm-matchingBracket": { backgroundColor: "color-mix(in srgb, var(--mh-success) 25%, transparent)", outline: "none" },
  ".cm-searchMatch": { backgroundColor: "color-mix(in srgb, var(--mh-warning) 30%, transparent)" },
  ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: "color-mix(in srgb, var(--mh-warning) 55%, transparent)" },
  ".cm-panels": { backgroundColor: "var(--mh-surface)", color: "var(--mh-text)", borderColor: "var(--mh-border)" },
  ".cm-panel.cm-search": { padding: "var(--mh-space-2)", fontFamily: "var(--mh-font-sans)" },
  ".cm-textfield": {
    backgroundColor: "var(--mh-input-bg)",
    color: "var(--mh-text)",
    border: "1px solid var(--mh-border-strong)",
    borderRadius: "var(--mh-radius-sm)",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--mh-surface-raised)",
    color: "var(--mh-text)",
    border: "1px solid var(--mh-border)",
    borderRadius: "var(--mh-radius-sm)",
  },
  ".cm-foldPlaceholder": { backgroundColor: "var(--mh-surface-raised)", color: "var(--mh-text-muted)", border: "none" },
});

const highlight = HighlightStyle.define([
  { tag: [tags.comment, tags.lineComment, tags.blockComment], color: "var(--mh-text-muted)", fontStyle: "italic" },
  { tag: [tags.propertyName, tags.definition(tags.propertyName), tags.attributeName], color: "var(--mh-primary)" },
  { tag: [tags.string, tags.special(tags.string)], color: "var(--mh-info)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom], color: "var(--mh-warning)" },
  { tag: [tags.keyword, tags.operatorKeyword, tags.heading], color: "var(--mh-danger)" },
  { tag: [tags.variableName, tags.labelName], color: "var(--mh-success)" },
  { tag: tags.invalid, color: "var(--mh-danger)", textDecoration: "underline" },
]);

function languageExtension(language: CodeLanguage): LanguageSupport | Extension {
  switch (language) {
    case "json":
      return json();
    case "yaml":
      return yaml();
    case "toml":
      return StreamLanguage.define(toml);
    case "properties":
      return StreamLanguage.define(properties);
    case "shell":
      return StreamLanguage.define(shell);
    case "text":
      return [];
  }
}

export interface EditorSetup {
  doc: string;
  language: CodeLanguage;
  label: string;
  readOnly?: boolean;
  /** Câu chữ của hộp tìm / thay, đã dịch. */
  phrases: Record<string, string>;
  onChange: (text: string) => void;
  onCursor: (line: number, column: number) => void;
  onSave: () => void;
}

/** Chỗ đổi được sau khi dựng: xuống dòng. */
export const wrapCompartment = new Compartment();

export function wrapExtension(wrap: boolean): Extension {
  return wrap ? EditorView.lineWrapping : [];
}

export function createState(setup: EditorSetup, wrap: boolean): EditorState {
  return EditorState.create({
    doc: setup.doc,
    extensions: [
      lineNumbers(),
      highlightActiveLineGutter(),
      foldGutter(),
      highlightSpecialChars(),
      history(),
      drawSelection(),
      indentOnInput(),
      bracketMatching(),
      closeBrackets(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      search({ top: true }),
      indentUnit.of("  "),
      EditorState.tabSize.of(2),
      EditorState.phrases.of(setup.phrases),
      EditorView.contentAttributes.of({ "aria-label": setup.label }),
      keymap.of([
        { key: "Mod-s", preventDefault: true, run: () => (setup.onSave(), true) },
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        ...foldKeymap,
        indentWithTab,
      ]),
      languageExtension(setup.language),
      syntaxHighlighting(highlight),
      theme,
      setup.readOnly ? [EditorState.readOnly.of(true), EditorView.editable.of(false)] : [],
      wrapCompartment.of(wrapExtension(wrap)),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) setup.onChange(update.state.doc.toString());
        if (update.docChanged || update.selectionSet) {
          const head = update.state.selection.main.head;
          const line = update.state.doc.lineAt(head);
          setup.onCursor(line.number, head - line.from + 1);
        }
      }),
    ],
  });
}
