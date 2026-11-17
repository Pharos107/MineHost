import { useState } from "react";
import { MemoryRouter } from "react-router";

import { Breadcrumb, Button, Card, CodeEditor, Field, FileButton, FileDrop, Stack, TextArea, toast } from "../ui";

const SAMPLE_YAML = `# Cấu hình plugin
settings:
  enabled: true
  max-homes: 5
  prefix: "&a[Home]"
worlds:
  - world
  - world_nether
`;

/** Breadcrumb, FileDrop (hai biến thể), FileButton, TextArea, CodeEditor và Toast. `MemoryRouter` vì Breadcrumb dùng `Link`. */
export function FilesSection() {
  const [picked, setPicked] = useState<string[]>([]);
  return (
    <Card title="Breadcrumb / FileDrop / FileButton / TextArea / CodeEditor">
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
          <CodeSample />
          <Stack direction="row" gap={2} wrap>
            <Button size="sm" onClick={() => toast.show({ tone: "success", title: "Đã nén xong." })}>
              Toast thường (5 giây)
            </Button>
            <Button size="sm" onClick={() => toast.show({ tone: "danger", title: "Tải lên không xong", body: "Ổ đĩa đầy." })}>
              Toast lỗi
            </Button>
          </Stack>
        </Stack>
      </MemoryRouter>
    </Card>
  );
}

function CodeSample() {
  const [text, setText] = useState(SAMPLE_YAML);
  const [cursor, setCursor] = useState("1:1");
  return (
    <CodeEditor
      label="CodeEditor YAML"
      value={text}
      onChange={setText}
      language="yaml"
      onCursor={(line, column) => setCursor(`${line}:${column}`)}
      status={
        <>
          <span>{text === SAMPLE_YAML ? "Đã lưu" : "Chưa lưu"}</span>
          <span>Dòng:Cột {cursor}</span>
          <span>YAML</span>
        </>
      }
    />
  );
}
