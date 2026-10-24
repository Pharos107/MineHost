import { useTranslation } from "react-i18next";

import { LanguageSwitch } from "./components/LanguageSwitch";
import { SystemCard } from "./components/SystemCard";
import { AppShell, PageHeader } from "./ui";

export function App() {
  const { t } = useTranslation();

  return (
    <AppShell actions={<LanguageSwitch />}>
      <PageHeader title="MineHost" description={t("app.tagline")} />
      <SystemCard />
    </AppShell>
  );
}
