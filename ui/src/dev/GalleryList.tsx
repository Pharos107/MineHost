import { Gavel, ShieldCheck, UserPlus } from "lucide-react";
import { useState } from "react";

import { Avatar, Badge, Button, Card, DetailHeader, DetailSection, SearchInput, SelectList, SelectListGroup, SelectListItem, Stack, Switch } from "../ui";

const PEOPLE = [
  { name: "Steve", online: true, sub: "Đang chơi", badges: ["Op"] },
  { name: "Alex", online: false, sub: "Vào 2 giờ trước", badges: ["Whitelist"] },
  { name: "Griefer", online: false, sub: "Chưa vào", badges: ["Bị cấm"] },
];

/** Danh sách chọn một mục và khung chi tiết đi kèm (trang Người chơi). */
export function ListSection() {
  const [picked, setPicked] = useState("Steve");
  const [query, setQuery] = useState("");
  const [op, setOp] = useState(true);
  return (
    <Card title="SelectList / DetailHeader / DetailSection">
      <SelectList
        label="Người chơi mẫu"
        search={<SearchInput label="Tìm theo tên" value={query} onChange={setQuery} />}
        actions={
          <Button variant="primary" icon={UserPlus}>
            Thêm
          </Button>
        }
      >
        <SelectListGroup title="Đang chơi" count={1}>
          {PEOPLE.filter((p) => p.online).map((p) => (
            <Item key={p.name} person={p} picked={picked} onPick={setPicked} />
          ))}
        </SelectListGroup>
        <SelectListGroup title="Không online" count={2}>
          {PEOPLE.filter((p) => !p.online).map((p) => (
            <Item key={p.name} person={p} picked={picked} onPick={setPicked} />
          ))}
        </SelectListGroup>
        <SelectListGroup title="Nhóm trống" count={0} empty="Chưa có ai." />
      </SelectList>
      <DetailHeader title={picked} media={<Avatar name={picked} size="xl" />} subtitle="Lần cuối vào 2 giờ trước">
        <Stack direction="row" gap={1}>
          <Badge tone="primary">Op</Badge>
        </Stack>
      </DetailHeader>
      <DetailSection title="Quyền" icon={ShieldCheck}>
        <Switch label="Op" description="Dùng được lệnh quản trị trong game." checked={op} onChange={setOp} />
      </DetailSection>
      <DetailSection title="Kỷ luật" icon={Gavel} tone="danger">
        <Stack direction="row" gap={2}>
          <Button size="sm">Đuổi</Button>
          <Button size="sm" variant="danger">
            Cấm
          </Button>
        </Stack>
      </DetailSection>
    </Card>
  );
}

function Item({ person, picked, onPick }: { person: (typeof PEOPLE)[number]; picked: string; onPick: (name: string) => void }) {
  return (
    <SelectListItem
      leading={<Avatar name={person.name} online={person.online} />}
      title={person.name}
      subtitle={person.sub}
      trailing={person.badges.map((b) => (
        <Badge key={b} tone={b === "Bị cấm" ? "danger" : "primary"}>
          {b}
        </Badge>
      ))}
      selected={picked === person.name}
      onSelect={() => onPick(person.name)}
    />
  );
}
