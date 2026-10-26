import { ArrowLeft, House, LayoutDashboard, ListChecks, Server, Settings, Terminal } from "lucide-react";
import { MemoryRouter } from "react-router";

import { Card, NavItem, NavSection, Stack } from "../ui";

/**
 * Thanh bên mẫu. Trang `#/ui` không nằm trong router của app nên tự bọc `MemoryRouter`; mục
 * "Máy chủ" và "Console" sáng vì đường dẫn giả đang ở đó. Thanh bên thật xem ở các trang của app.
 */
export function NavSectionDemo() {
  return (
    <Card title="NavSection / NavItem">
      <Stack direction="row" gap={6} wrap>
        <MemoryRouter initialEntries={["/servers"]}>
          <NavSection>
            <NavItem to="/" end icon={House} label="Trang chủ" />
            <NavItem to="/servers" icon={Server} label="Máy chủ" />
            <NavItem to="/jobs" icon={ListChecks} label="Tác vụ" />
            <NavItem to="/settings" icon={Settings} label="Cài đặt" />
          </NavSection>
        </MemoryRouter>
        <MemoryRouter initialEntries={["/servers/1/console"]}>
          <NavSection>
            <NavItem to="/servers" end icon={ArrowLeft} label="Tất cả máy chủ" />
          </NavSection>
          <NavSection title="Survival của nhóm">
            <NavItem to="/servers/1" end icon={LayoutDashboard} label="Tổng quan" />
            <NavItem to="/servers/1/console" icon={Terminal} label="Console" />
          </NavSection>
        </MemoryRouter>
      </Stack>
    </Card>
  );
}
