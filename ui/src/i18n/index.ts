import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./en.json";
import vi from "./vi.json";

export const LANGUAGES = ["vi", "en"] as const;
export type Language = (typeof LANGUAGES)[number];

const STORAGE_KEY = "minehost.language";

function initialLanguage(): Language {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "vi" || saved === "en") return saved;
  } catch {
    // Trình duyệt chặn localStorage: dùng ngôn ngữ của trình duyệt.
  }
  return navigator.language.toLowerCase().startsWith("vi") ? "vi" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { vi: { translation: vi }, en: { translation: en } },
  lng: initialLanguage(),
  fallbackLng: "vi",
  interpolation: { escapeValue: false },
});

i18n.on("languageChanged", (lang) => {
  document.documentElement.lang = lang;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch {
    // Không lưu được thì lần sau dùng lại ngôn ngữ của trình duyệt.
  }
});

export default i18n;
