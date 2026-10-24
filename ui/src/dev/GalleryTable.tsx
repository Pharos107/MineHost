import { Play, Plus, RotateCw, Server, Skull, Square, SquareTerminal, Trash2 } from "lucide-react";
import { useState } from "react";

import type { ServerState } from "../api/client";
import { Badge, Button, Card, CopyField, DataTable, EmptyState, Stack, StatusBadge, type Column, type RowAction } from "../ui";

interface Row {
  id: string;
  name: string;
  version: string;
  state: ServerState;
  address: string;
  memoryMb: number;
}

const ROWS: Row[] = [
  { id: "1", name: "Survival", version: "1.21.4", state: "running", address: "192.168.1.10:25565", memoryMb: 4096 },
  { id: "2", name: "Sáng tạo cùng Đức", version: "1.21.4", state: "stopped", address: "192.168.1.10:25566", memoryMb: 2048 },
  { id: "3", name: "Server 10", version: "1.20.6", state: "starting", address: "192.168.1.10:25567", memoryMb: 3072 },
  { id: "4", name: "Server 2", version: "1.21.3", state: "crashed", address: "192.168.1.10:25568", memoryMb: 6144 },
];

const COLUMNS: Column<Row>[] = [
  { id: "state", header: "Trạng thái", cell: (row) => <StatusBadge state={row.state} />, sortValue: (row) => row.state, width: "md" },
  { id: "name", header: "Tên", cell: (row) => <strong>{row.name}</strong>, sortValue: (row) => row.name },
  { id: "type", header: "Loại", cell: (row) => <Badge>Paper {row.version}</Badge>, sortValue: (row) => row.version },
  { id: "address", header: "Địa chỉ", cell: (row) => <CopyField value={row.address} label="Chép địa chỉ" /> },
  { id: "memory", header: "RAM", cell: (row) => `${row.memoryMb / 1024} GB`, sortValue: (row) => row.memoryMb, align: "end", width: "sm" },
];

const isOn = (row: Row) => row.state === "running" || row.state === "starting";

const ACTIONS: RowAction<Row>[] = [
  { label: "Console", icon: SquareTerminal, onSelect: () => undefined },
  { label: "Bật", icon: Play, onSelect: () => undefined, hidden: isOn },
  { label: "Tắt", icon: Square, onSelect: () => undefined, hidden: (row) => !isOn(row) },
  { label: "Khởi động lại", icon: RotateCw, onSelect: () => undefined, disabled: (row) => !isOn(row) },
  { label: "Buộc dừng", icon: Skull, tone: "danger", onSelect: () => undefined, disabled: (row) => !isOn(row) },
  { label: "Xóa", icon: Trash2, tone: "danger", onSelect: () => undefined },
];

type Mode = "rows" | "loading" | "empty";

export function TableSection() {
  const [mode, setMode] = useState<Mode>("rows");
  return (
    <Card
      title="DataTable"
      description="Mọi bảng chỉ khai báo cột và thao tác. Thu hẹp cửa sổ dưới 560px để xem dạng thẻ."
      actions={
        <Stack direction="row" gap={1}>
          {(["rows", "loading", "empty"] as const).map((value) => (
            <Button key={value} size="sm" variant={mode === value ? "primary" : "ghost"} onClick={() => setMode(value)}>
              {value}
            </Button>
          ))}
        </Stack>
      }
    >
      <DataTable
        label="Danh sách server"
        columns={COLUMNS}
        rows={mode === "empty" ? [] : ROWS}
        loading={mode === "loading"}
        rowKey={(row) => row.id}
        rowLabel={(row) => row.name}
        actions={ACTIONS}
        searchText={(row) => `${row.name} ${row.version}`}
        defaultSort={{ columnId: "name", direction: "asc" }}
        toolbar={<Button variant="primary" size="sm" icon={Plus}>Tạo server</Button>}
        empty={<EmptyState icon={Server} title="Chưa có server nào" action={<Button variant="primary" icon={Plus}>Tạo server đầu tiên</Button>} />}
      />
    </Card>
  );
}
