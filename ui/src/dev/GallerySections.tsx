import { Power, RotateCw, Skull } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { formatBytes } from "../format";
import {
  Button,
  Card,
  ConfirmDialog,
  CopyField,
  Dialog,
  DialogBody,
  DialogFooter,
  Field,
  Menu,
  ProgressBar,
  Select,
  Slider,
  Stack,
  Steps,
  Switch,
  Tabs,
  TextInput,
} from "../ui";

const MB = 1024 * 1024;

export function FormSection() {
  const { i18n } = useTranslation();
  const [name, setName] = useState("Server của tôi");
  const [memory, setMemory] = useState(4096);
  const [autoStart, setAutoStart] = useState(true);

  return (
    <Card title="Field / TextInput / Select / Switch / Slider">
      <Field label="Tên server" hint="1–64 ký tự." error={name.trim() === "" ? "Tên không được để trống." : undefined}>
        <TextInput value={name} onChange={(e) => setName(e.target.value)} maxLength={64} />
      </Field>
      <Field label="Phiên bản">
        <Select defaultValue="1.21.4">
          <option value="1.21.4">1.21.4</option>
          <option value="1.21.3">1.21.3</option>
          <option value="1.20.6">1.20.6</option>
        </Select>
      </Field>
      <Field label="RAM tối đa" hint="Vượt quá RAM đang trống thì máy có thể chạy chậm.">
        <Slider
          value={memory}
          onChange={setMemory}
          min={1024}
          max={14336}
          step={512}
          recommended={4096}
          formatValue={(mb) => formatBytes(mb * MB, i18n.language)}
        />
      </Field>
      <Switch
        label="Tự bật khi mở MineHost"
        description="Server sẽ chạy ngay khi bạn mở MineHost."
        checked={autoStart}
        onChange={setAutoStart}
      />
    </Card>
  );
}

export function ProgressSection() {
  const [step, setStep] = useState(1);
  return (
    <Card title="Steps / ProgressBar">
      <Steps steps={["Loại server", "Phiên bản", "Tên và RAM"]} current={step} />
      <Stack direction="row" gap={2}>
        <Button size="sm" disabled={step === 0} onClick={() => setStep(step - 1)}>Quay lại</Button>
        <Button size="sm" variant="primary" disabled={step === 2} onClick={() => setStep(step + 1)}>Tiếp</Button>
      </Stack>
      <ProgressBar label="Tải Java 21 (48/61 MB)" value={79} />
      <ProgressBar label="Chuẩn bị file" />
      <ProgressBar label="Tải Paper bị lỗi" value={35} tone="danger" />
    </Card>
  );
}

export function OverlaySection() {
  const [tab, setTab] = useState("console");
  const [eula, setEula] = useState(false);
  const [confirm, setConfirm] = useState(false);

  return (
    <Card title="Tabs / Menu / CopyField / Dialog / ConfirmDialog">
      <Stack direction="row" gap={3} wrap align="center">
        <CopyField value="192.168.1.10:25565" label="Chép địa chỉ" />
        <Menu
          label="Thao tác khác"
          items={[
            { label: "Khởi động lại", icon: RotateCw, onSelect: () => undefined },
            { label: "Buộc dừng", icon: Skull, tone: "danger", onSelect: () => setConfirm(true) },
          ]}
        />
        <Button onClick={() => setEula(true)}>Mở hộp thoại EULA</Button>
      </Stack>
      <Tabs
        label="Trang server"
        value={tab}
        onChange={setTab}
        items={[
          { id: "console", label: "Console", panel: <p>Log realtime ở đây.</p> },
          { id: "game", label: "Cài đặt game", panel: <p>Các khóa của server.properties.</p> },
          { id: "server", label: "Cài đặt server", panel: <p>Tên, RAM, cổng.</p> },
        ]}
      />
      <Dialog open={eula} onClose={() => setEula(false)} title="Đồng ý EULA của Minecraft" dismissible={false}>
        <DialogBody>
          <p>Minecraft yêu cầu bạn đồng ý Điều khoản người dùng (EULA) của Mojang.</p>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setEula(false)}>Hủy</Button>
          <Button variant="primary" onClick={() => setEula(false)}>Tôi đồng ý</Button>
        </DialogFooter>
      </Dialog>
      <ConfirmDialog
        open={confirm}
        onCancel={() => setConfirm(false)}
        onConfirm={() => setConfirm(false)}
        title="Buộc dừng server?"
        confirmLabel="Buộc dừng"
        requireText="Server của tôi"
      >
        <p>Server sẽ bị tắt ngay, thay đổi chưa lưu của world có thể bị mất.</p>
      </ConfirmDialog>
      <Stack direction="row">
        <Button icon={Power} variant="ghost" onClick={() => setConfirm(true)}>Mở ConfirmDialog</Button>
      </Stack>
    </Card>
  );
}
