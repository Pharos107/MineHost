import { useState } from "react";

import { Card, Stack, SuggestBox, TextInput } from "../ui";

const ITEMS = [
  { value: "gamemode", label: "gamemode <survival|creative> [người chơi]", detail: "Đổi chế độ chơi" },
  { value: "gamerule", label: "gamerule <luật> [giá trị]", detail: "Xem hoặc đổi luật game" },
];

/** Danh sách gợi ý mở lên trên ô nhập, như ô lệnh console. */
export function SuggestSection() {
  const [value, setValue] = useState("gam");
  const items = ITEMS.filter((item) => value !== "" && item.value.startsWith(value));
  return (
    <Card title="SuggestBox">
      <Stack gap={8}>
        <span />
        <SuggestBox id="gallery-suggest" label="Gợi ý lệnh" items={items} active={0} onPick={(v) => setValue(`${v} `)}>
          <TextInput aria-label="Lệnh" value={value} onChange={(e) => setValue(e.target.value)} />
        </SuggestBox>
      </Stack>
    </Card>
  );
}
