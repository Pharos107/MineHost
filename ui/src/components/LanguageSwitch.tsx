import { useTranslation } from "react-i18next";

import { LANGUAGES, type Language } from "../i18n";

export function LanguageSwitch() {
  const { t, i18n } = useTranslation();

  return (
    <label className="language">
      <span className="visually-hidden">{t("app.language")}</span>
      <select value={i18n.resolvedLanguage} onChange={(e) => void i18n.changeLanguage(e.target.value as Language)}>
        {LANGUAGES.map((lang) => (
          <option key={lang} value={lang}>
            {t(`language.${lang}`)}
          </option>
        ))}
      </select>
    </label>
  );
}
