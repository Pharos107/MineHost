import { ConsoleTab } from "./ConsoleTab";
import { GameSettingsTab } from "./GameSettingsTab";
import { useServer } from "./ServerLayout";
import { ServerSettingsTab } from "./ServerSettingsTab";

// Mỗi trang con lấy server từ `ServerLayout` qua `Outlet`, không gọi API lần nữa.

export function ConsoleRoute() {
  return <ConsoleTab server={useServer()} />;
}

export function GameSettingsRoute() {
  return <GameSettingsTab server={useServer()} />;
}

export function ServerSettingsRoute() {
  return <ServerSettingsTab server={useServer()} />;
}
