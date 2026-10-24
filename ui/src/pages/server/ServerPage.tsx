import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams } from "react-router";

import { api, errorMessage, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { Button, CopyField, Notice, PageHeader, Spinner, Stack, StatusBadge, Tabs } from "../../ui";
import { serverName } from "../ServerListPage";
import { ConsoleTab } from "./ConsoleTab";
import { DeleteServer } from "./DeleteServer";
import { GameSettingsTab } from "./GameSettingsTab";
import { ServerActions } from "./ServerActions";
import { ServerNotices } from "./ServerNotices";
import { ServerSettingsTab } from "./ServerSettingsTab";

/** Trang chi tiết server (spec P1, mục 2.4). Trạng thái tự cập nhật qua topic `servers`. */
export function ServerPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const id = useParams().id ?? "";
  const server = useQuery({ queryKey: [...SERVERS_KEY, id], queryFn: () => api.server(id) });
  const back = (
    <Button variant="ghost" icon={ArrowLeft} onClick={() => void navigate("/")}>
      {t("servers.back")}
    </Button>
  );

  if (server.isPending) return <Spinner label={t("ui.loading")} />;
  if (server.isError) {
    return (
      <Notice tone="danger" action={back}>
        {errorMessage(t, server.error)}
      </Notice>
    );
  }
  return <ServerView server={server.data} back={back} />;
}

function ServerView({ server, back }: { server: Server; back: ReactNode }) {
  const { t } = useTranslation();
  const [tab, setTab] = useState("console");
  const ready = server.state !== "installing" && server.state !== "broken";
  const tabs = [
    { id: "console", label: t("server.tabs.console"), panel: <ConsoleTab server={server} /> },
    { id: "game", label: t("server.tabs.game"), panel: <GameSettingsTab server={server} /> },
    { id: "settings", label: t("server.tabs.settings"), panel: <ServerSettingsTab server={server} /> },
  ];

  return (
    <Stack gap={5}>
      <div>{back}</div>
      <PageHeader
        title={serverName(server)}
        description={
          <Stack direction="row" gap={3} align="center" wrap>
            <StatusBadge state={server.state} />
            <LanAddresses port={server.port} />
          </Stack>
        }
        actions={ready && <ServerActions server={server} />}
      />
      <ServerNotices server={server} />
      {ready && <Tabs label={t("server.tabsLabel")} value={tab} onChange={setTab} items={tabs} />}
      {server.state === "broken" && <DeleteServer server={server} />}
    </Stack>
  );
}

/** Địa chỉ để bạn bè trong cùng mạng vào chơi, ví dụ `192.168.1.5:25565`. */
function LanAddresses({ port }: { port?: number | null }) {
  const { t } = useTranslation();
  const system = useQuery({ queryKey: ["system"], queryFn: api.system });
  if (port == null || !system.data) return null;
  return (
    <>
      {system.data.lan_addresses.map((ip) => (
        <CopyField key={ip} value={`${ip}:${port}`} label={t("server.copyAddress")} />
      ))}
    </>
  );
}
