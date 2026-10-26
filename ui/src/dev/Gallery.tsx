// Trang `#/ui` chỉ dùng khi phát triển, không có trong bản build. Chuỗi mẫu ghi thẳng ở đây vì
// người dùng không bao giờ thấy trang này; chuỗi của chính component vẫn đi qua i18n.
import { Play, Plus, Power, RotateCw, Server, Square, Trash2 } from "lucide-react";
import { useState } from "react";

import type { ServerState } from "../api/client";
import { LanguageSwitch } from "../components/LanguageSwitch";
import {
  AppShell,
  Badge,
  Button,
  Card,
  EmptyState,
  Facts,
  IconButton,
  Notice,
  PageHeader,
  Spinner,
  Stack,
  StatusBadge,
} from "../ui";
import { NavSectionDemo } from "./GalleryNav";
import { ChartSection } from "./GalleryCharts";
import { FilesSection } from "./GalleryFiles";
import { FormSection, OverlaySection, ProgressSection } from "./GallerySections";
import { TableSection } from "./GalleryTable";

const STATES: ServerState[] = ["installing", "stopped", "starting", "running", "stopping", "crashed", "broken"];

export function Gallery() {
  return (
    <AppShell actions={<LanguageSwitch />}>
      <PageHeader
        title="Bộ component"
        description="Mọi trang chỉ ghép từ những thứ ở đây. Đổi chế độ sáng/tối của hệ điều hành để xem cả hai."
        actions={<Button variant="primary" icon={Plus}>Tạo server</Button>}
      />
      <Stack gap={6}>
        <ButtonSection />
        <BadgeSection />
        <NoticeSection />
        <CardSection />
        <TableSection />
        <FormSection />
        <ProgressSection />
        <OverlaySection />
        <FilesSection />
        <ChartSection />
        <NavSectionDemo />
      </Stack>
    </AppShell>
  );
}

function ButtonSection() {
  const [loading, setLoading] = useState(false);
  return (
    <Card title="Button" description="primary cho việc chính của trang (chỉ một), secondary cho việc phụ, danger cho việc phá hủy.">
      <Stack direction="row" gap={2} wrap align="center">
        <Button variant="primary" icon={Play}>Bật server</Button>
        <Button icon={Square}>Tắt</Button>
        <Button icon={RotateCw}>Khởi động lại</Button>
        <Button variant="danger" icon={Trash2}>Xóa</Button>
        <Button variant="ghost">Hủy</Button>
        <Button disabled>Không bấm được</Button>
        <Button
          variant="primary"
          loading={loading}
          onClick={() => {
            setLoading(true);
            window.setTimeout(() => setLoading(false), 1500);
          }}
        >
          Bấm để thử loading
        </Button>
      </Stack>
      <Stack direction="row" gap={2} wrap align="center">
        <Button size="sm" variant="primary">Nhỏ</Button>
        <Button size="sm">Nhỏ</Button>
        <IconButton icon={Power} label="Thoát" />
        <IconButton icon={Trash2} label="Xóa" variant="danger" size="sm" />
        <Spinner label="Đang tải…" />
      </Stack>
    </Card>
  );
}

function BadgeSection() {
  return (
    <Card title="Badge / StatusBadge">
      <Stack direction="row" gap={2} wrap>
        {STATES.map((state) => (
          <StatusBadge key={state} state={state} />
        ))}
      </Stack>
      <Stack direction="row" gap={2} wrap>
        <Badge>Paper</Badge>
        <Badge tone="primary">1.21.4</Badge>
        <Badge tone="warning">Bản thử nghiệm</Badge>
      </Stack>
    </Card>
  );
}

function NoticeSection() {
  return (
    <Card title="Notice">
      <Notice title="Có hiệu lực sau khi khởi động lại">Server đang chạy, thay đổi sẽ được áp dụng ở lần bật sau.</Notice>
      <Notice tone="success">Đã tạo server “Server của tôi”.</Notice>
      <Notice tone="warning">RAM chọn vượt quá RAM đang trống (6 GB). Máy có thể chạy chậm.</Notice>
      <Notice tone="danger" title="Không tải được Paper" action={<Button size="sm" icon={RotateCw}>Thử lại</Button>}>
        Không kết nối được Internet. Kiểm tra mạng rồi thử lại.
      </Notice>
    </Card>
  );
}

function CardSection() {
  return (
    <Card title="Card / Facts / EmptyState" description="Card có tiêu đề, mô tả và nút ở góc." actions={<Button size="sm">Sửa</Button>}>
      <Facts
        items={[
          { label: "Phiên bản", value: "Paper 1.21.4" },
          { label: "RAM", value: "4 GB" },
          { label: "Thư mục", value: <code>C:\Users\an\MineHost\servers\01J9Z</code> },
        ]}
      />
      <EmptyState
        icon={Server}
        title="Chưa có server nào"
        description="Tạo server đầu tiên, MineHost sẽ tự tải Java và Paper cho bạn."
        action={<Button variant="primary" icon={Plus}>Tạo server đầu tiên</Button>}
      />
    </Card>
  );
}
