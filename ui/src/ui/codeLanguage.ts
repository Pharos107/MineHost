/** Ngôn ngữ tô màu của `CodeEditor` (spec trang Tệp, mục 1). Không import CodeMirror để trang dùng được ngay. */
export type CodeLanguage = "json" | "yaml" | "toml" | "properties" | "shell" | "text";

const BY_EXTENSION: Record<string, CodeLanguage> = {
  json: "json",
  json5: "json",
  mcmeta: "json",
  yml: "yaml",
  yaml: "yaml",
  toml: "toml",
  properties: "properties",
  cfg: "properties",
  conf: "properties",
  ini: "properties",
  sh: "shell",
  bash: "shell",
};

/** Đoán theo đuôi file; không có đuôi hoặc đuôi lạ thì là văn bản thường. */
export function languageFor(path: string): CodeLanguage {
  const name = path.split("/").pop() ?? "";
  const dot = name.lastIndexOf(".");
  if (dot <= 0) return "text";
  return BY_EXTENSION[name.slice(dot + 1).toLowerCase()] ?? "text";
}
