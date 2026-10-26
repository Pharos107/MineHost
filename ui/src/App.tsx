import { useTranslation } from "react-i18next";
import { BrowserRouter, Route, Routes } from "react-router";

import { useServersLive } from "./api/live";
import { SidebarLayout, TopLayout } from "./components/AppLayout";
import { ComingSoon } from "./components/ComingSoon";
import { CreateServerPage } from "./pages/create/CreateServerPage";
import { HomePage } from "./pages/home/HomePage";
import { JobsPage } from "./pages/jobs/JobsPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { ServerListPage } from "./pages/ServerListPage";
import { ConsoleRoute, GameSettingsRoute, ServerComingSoon, ServerSettingsRoute } from "./pages/server/routes";
import { ServerLayout } from "./pages/server/ServerLayout";

/** Các trang theo spec P2, mục 2. Backend trả `index.html` cho mọi đường dẫn lạ. */
export function AppRoutes() {
  const { t } = useTranslation();

  return (
    <Routes>
      <Route
        path="/new"
        element={
          <TopLayout>
            <CreateServerPage />
          </TopLayout>
        }
      />
      <Route element={<SidebarLayout />}>
        <Route path="/" element={<HomePage />} />
        <Route path="/servers" element={<ServerListPage />} />
        <Route path="/jobs" element={<JobsPage />} />
        <Route path="/settings" element={<ComingSoon title={t("nav.settings")} />} />
        <Route path="/servers/:id" element={<ServerLayout />}>
          <Route index element={<ServerComingSoon />} />
          <Route path="console" element={<ConsoleRoute />} />
          <Route path="files" element={<ServerComingSoon />} />
          <Route path="players" element={<ServerComingSoon />} />
          <Route path="backups" element={<ServerComingSoon />} />
          <Route path="game" element={<GameSettingsRoute />} />
          <Route path="settings" element={<ServerSettingsRoute />} />
        </Route>
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}

export function App() {
  useServersLive();

  return (
    <BrowserRouter>
      <AppRoutes />
    </BrowserRouter>
  );
}
