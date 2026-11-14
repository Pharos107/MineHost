import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Outlet, useNavigate, useOutletContext, useParams } from "react-router";

import { api, errorMessage, type Server } from "../../api/client";
import { SERVERS_KEY } from "../../api/live";
import { Button, CopyField, Notice, PageHeader, Spinner, Stack, StatusBadge } from "../../ui";
import { serverName } from "../ServerListPage";
import { DeleteServer } from "./DeleteServer";
import { PreflightBanner } from "./PreflightBanner";
import { ServerActions } from "./ServerActions";
import { ServerNotices } from "./ServerNotices";

/** Khóa query của một server; thanh bên dùng chung để hiện tên server mà không gọi API lần nữa. */
export function serverKey(id: string) {
  return [...SERVERS_KEY, id];
}

/**
 * Khung chung của mọi trang trong một server (spec P2, mục 2): đầu trang giữ như P1, phần dưới là
 * trang con theo đường dẫn. Trạng thái tự cập nhật qua topic `servers`.
 */
export function ServerLayout() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const id = useParams().id ?? "";
  const server = useQuery({ queryKey: serverKey(id), queryFn: () => api.server(id) });

  if (server.isPending) return <Spinner label={t("ui.loading")} />;
  if (server.isError) {
    return (
      <Notice
        tone="danger"
        action={
          <Button variant="ghost" icon={ArrowLeft} onClick={() => void navigate("/servers")}>
            {t("nav.allServers")}
          </Button>
        }
      >
        {errorMessage(t, server.error)}
      </Notice>
    );
  }
  return <ServerView server={server.data} />;
}

function ServerView({ server }: { server: Server }) {
  const ready = server.state !== "installing" && server.state !== "broken";

  return (
    <Stack gap={5}>
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
      <PreflightBanner server={server} />
      {ready && <Outlet context={server} />}
      {server.state === "broken" && <DeleteServer server={server} />}
    </Stack>
  );
}

/** Server đang mở, cho các trang con nằm trong `ServerLayout`. */
export function useServer(): Server {
  return useOutletContext<Server>();
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
