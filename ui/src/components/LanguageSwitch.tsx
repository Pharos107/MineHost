import { useTranslation } from "react-i18next";

import { LANGUAGES, type Language } from "../i18n";
import { Select } from "../ui";

export function LanguageSwitch() {
  const { t, i18n } = useTranslation();

  return (
    <Select
      aria-label={t("app.language")}
      value={i18n.resolvedLanguage}
      onChange={(e) => void i18n.changeLanguage(e.target.value as Language)}
    >
      {LANGUAGES.map((lang) => (
        <option key={lang} value={lang}>
          {t(`language.${lang}`)}
        </option>
      ))}
    </Select>
  );
}
