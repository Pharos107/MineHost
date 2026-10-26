import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  Archive,
  FolderOpen,
  House,
  LayoutDashboard,
  ListChecks,
  Server as ServerIcon,
  Settings,
  SlidersHorizontal,
  Terminal,
  Users,
  Wrench,
} from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Outlet, useMatch } from "react-router";

import { api } from "../api/client";
import { serverName } from "../pages/ServerListPage";
import { serverKey } from "../pages/server/ServerLayout";
import { AppShell, NavItem, NavSection } from "../ui";
import { LanguageSwitch } from "./LanguageSwitch";
import { QuitButton } from "./QuitButton";

function ShellControls() {
  return (
    <>
      <LanguageSwitch />
      <QuitButton />
    </>
  );
}

/** Khung có thanh bên cho mọi trang trừ wizard (spec P2, mục 2). */
export function SidebarLayout() {
  const { t } = useTranslation();
  // Đang ở trong một server thì thanh bên chuyển sang các mục của server đó.
  const inServer = useMatch("/servers/:id/*");
  const id = inServer?.params.id;

  return (
    <AppShell
      variant="sidebar"
      navLabel={t("nav.label")}
      nav={id ? <ServerNav id={id} /> : <MainNav />}
      footer={<ShellControls />}
    >
      <Outlet />
    </AppShell>
  );
}

/** Khung không có thanh bên, cho wizard tạo server. */
export function TopLayout({ children }: { children: ReactNode }) {
  return <AppShell actions={<ShellControls />}>{children}</AppShell>;
}

function MainNav() {
  const { t } = useTranslation();

  return (
    <NavSection>
      <NavItem to="/" end icon={House} label={t("nav.home")} />
      <NavItem to="/servers" icon={ServerIcon} label={t("nav.servers")} />
      <NavItem to="/jobs" icon={ListChecks} label={t("nav.jobs")} />
      <NavItem to="/settings" icon={Settings} label={t("nav.settings")} />
    </NavSection>
  );
}

function ServerNav({ id }: { id: string }) {
  const { t } = useTranslation();
  // Cùng khóa với `ServerLayout` nên không gọi API thêm lần nào.
  const server = useQuery({ queryKey: serverKey(id), queryFn: () => api.server(id) });
  const base = `/servers/${id}`;

  return (
    <>
      <NavSection>
        <NavItem to="/servers" end icon={ArrowLeft} label={t("nav.allServers")} />
      </NavSection>
      <NavSection title={server.data ? serverName(server.data) : undefined}>
        <NavItem to={base} end icon={LayoutDashboard} label={t("nav.overview")} />
        <NavItem to={`${base}/console`} icon={Terminal} label={t("nav.console")} />
        <NavItem to={`${base}/files`} icon={FolderOpen} label={t("nav.files")} />
        <NavItem to={`${base}/players`} icon={Users} label={t("nav.players")} />
        <NavItem to={`${base}/backups`} icon={Archive} label={t("nav.backups")} />
        <NavItem to={`${base}/game`} icon={SlidersHorizontal} label={t("nav.game")} />
        <NavItem to={`${base}/settings`} icon={Wrench} label={t("nav.serverSettings")} />
      </NavSection>
    </>
  );
}
