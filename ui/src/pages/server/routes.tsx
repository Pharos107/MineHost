import { ComingSoon } from "../../components/ComingSoon";
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

/** Tổng quan, Tệp, Người chơi, Sao lưu làm ở các PR sau của P2. */
export function ServerComingSoon() {
  return <ComingSoon />;
}
