import { useState } from "react";
import { MemoryRouter } from "react-router";

import { Breadcrumb, Card, Field, FileButton, FileDrop, Stack, TextArea } from "../ui";

/** Breadcrumb, FileDrop (hai biến thể), FileButton và TextArea. `MemoryRouter` vì Breadcrumb dùng `Link`. */
export function FilesSection() {
  const [picked, setPicked] = useState<string[]>([]);
  return (
    <Card title="Breadcrumb / FileDrop / FileButton / TextArea">
      <MemoryRouter>
        <Stack gap={4}>
          <Breadcrumb
            label="Đường dẫn"
            items={[{ label: "Thư mục server", to: "/" }, { label: "world", to: "/world" }, { label: "region" }]}
          />
          <FileDrop label="Tải file lên" hint="hoặc kéo thả file vào đây" onFiles={(files) => setPicked(files.map((f) => f.name))}>
            {picked.length > 0 && <span>Đã chọn: {picked.join(", ")}</span>}
          </FileDrop>
          <FileDrop
            variant="overlay"
            label="Tải lên"
            hint="Thả để thêm vào server"
            onFiles={(files) => setPicked(files.map((f) => f.name))}
          >
            <Stack gap={2}>
              <FileButton label="Tải lên .jar" accept=".jar" onFiles={(files) => setPicked(files.map((f) => f.name))} />
              <span>Kéo file vào khung này để thấy lớp phủ của biến thể overlay.</span>
            </Stack>
          </FileDrop>
          <Field label="TextArea thường">
            <TextArea rows={3} defaultValue="Ghi chú cho bản sao lưu" />
          </Field>
          <TextArea variant="code" aria-label="TextArea code" defaultValue={"motd=A Minecraft Server\npvp=true\n\tview-distance=10"} />
        </Stack>
      </MemoryRouter>
    </Card>
  );
}
