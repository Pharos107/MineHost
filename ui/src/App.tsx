import { useTranslation } from "react-i18next";

import { LanguageSwitch } from "./components/LanguageSwitch";
import { SystemCard } from "./components/SystemCard";

export function App() {
  const { t } = useTranslation();

  return (
    <div className="page">
      <header className="header">
        <h1>MineHost</h1>
        <LanguageSwitch />
      </header>
      <main>
        <p className="lead">{t("app.tagline")}</p>
        <SystemCard />
      </main>
    </div>
  );
}
