import { BrowserRouter, Route, Routes } from "react-router";

import { useServersLive } from "./api/live";
import { LanguageSwitch } from "./components/LanguageSwitch";
import { CreateServerPage } from "./pages/create/CreateServerPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { ServerListPage } from "./pages/ServerListPage";
import { ServerPage } from "./pages/ServerPage";
import { AppShell } from "./ui";

/** Các trang theo spec P1, mục 6. Backend trả `index.html` cho mọi đường dẫn lạ. */
export function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<ServerListPage />} />
      <Route path="/new" element={<CreateServerPage />} />
      <Route path="/servers/:id" element={<ServerPage />} />
      <Route path="*" element={<NotFoundPage />} />
    </Routes>
  );
}

export function App() {
  useServersLive();

  return (
    <BrowserRouter>
      <AppShell actions={<LanguageSwitch />}>
        <AppRoutes />
      </AppShell>
    </BrowserRouter>
  );
}
