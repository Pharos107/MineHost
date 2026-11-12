import { BrowserRouter, Route, Routes } from "react-router";

import { useServersLive } from "./api/live";
import { BareLayout, SidebarLayout, TopLayout } from "./components/AppLayout";
import { AuthGate } from "./components/AuthGate";
import { CreateServerPage } from "./pages/create/CreateServerPage";
import { LoginPage } from "./pages/auth/LoginPage";
import { HomePage } from "./pages/home/HomePage";
import { JobsPage } from "./pages/jobs/JobsPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { ServerListPage } from "./pages/ServerListPage";
import { ConsoleRoute, GameSettingsRoute, ServerComingSoon, ServerSettingsRoute } from "./pages/server/routes";
import { FileEditorPage } from "./pages/server/files/FileEditorPage";
import { FilesPage } from "./pages/server/files/FilesPage";
import { OverviewPage } from "./pages/server/overview/OverviewPage";
import { ServerLayout } from "./pages/server/ServerLayout";
import { SettingsPage } from "./pages/settings/SettingsPage";

/** Các trang theo spec P2, mục 2. Backend trả `index.html` cho mọi đường dẫn lạ. */
export function AppRoutes() {
  return (
    <Routes>
      <Route
        path="/login"
        element={
          <BareLayout>
            <LoginPage />
          </BareLayout>
        }
      />
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
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="/servers/:id" element={<ServerLayout />}>
          <Route index element={<OverviewPage />} />
          <Route path="console" element={<ConsoleRoute />} />
          <Route path="files" element={<FilesPage />} />
          <Route path="files/edit" element={<FileEditorPage />} />
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
      <AuthGate>
        <AppRoutes />
      </AuthGate>
    </BrowserRouter>
  );
}
