import { useMutation } from "@tanstack/react-query";
import { CloudOff, Megaphone, Save, Sun, Users, type LucideIcon } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api, errorMessage, type Server } from "../../../api/client";
import { Button, Dialog, DialogBody, DialogFooter, Field, Notice, Stack, TextInput } from "../../../ui";
import { ScanButton, useScanCommands } from "./ScanCommands";

interface Quick {
  key: string;
  icon: LucideIcon;
  command: string;
}

/** Lệnh vanilla có trên mọi loader, không cần đối số: bấm là gửi. */
const QUICK: Quick[] = [
  { key: "list", icon: Users, command: "list" },
  { key: "save", icon: Save, command: "save-all" },
  { key: "day", icon: Sun, command: "time set day" },
  { key: "clearWeather", icon: CloudOff, command: "weather clear" },
];

/** Hàng nút cho việc hay làm, để người không thuộc lệnh vẫn dùng được console. */
export function QuickCommands({ server }: { server: Server }) {
  const { t } = useTranslation();
  const [sayOpen, setSayOpen] = useState(false);
  const send = useMutation({ mutationFn: (command: string) => api.sendCommand(server.id, command) });
  const scan = useScanCommands(server.id);
  const error = send.error ?? scan.error;
  // Backend chỉ nhận lệnh khi server có process, giống ô lệnh.
  const online = server.state === "running" || server.state === "starting";

  return (
    <Stack gap={2}>
      <Stack direction="row" gap={2} wrap>
        {QUICK.map(({ key, icon, command }) => (
          <Button
            key={key}
            size="sm"
            icon={icon}
            disabled={!online}
            loading={send.isPending && send.variables === command}
            onClick={() => send.mutate(command)}
          >
            {t(`console.quick.${key}`)}
          </Button>
        ))}
        <Button size="sm" icon={Megaphone} disabled={!online} onClick={() => setSayOpen(true)}>
          {t("console.quick.say")}
        </Button>
        <ScanButton server={server} scan={scan} />
      </Stack>
      {error && <Notice tone="danger">{errorMessage(t, error)}</Notice>}
      <SayDialog server={server} open={sayOpen} onClose={() => setSayOpen(false)} />
    </Stack>
  );
}

/** Gửi `say <lời nhắn>`: hiện trong khung chat của mọi người đang chơi. */
function SayDialog({ server, open, onClose }: { server: Server; open: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const [message, setMessage] = useState("");
  const send = useMutation({
    mutationFn: (text: string) => api.sendCommand(server.id, `say ${text}`),
    onSuccess: () => {
      setMessage("");
      onClose();
    },
  });
  const text = message.trim();

  return (
    <Dialog open={open} onClose={onClose} title={t("console.say.title")} dismissible={!send.isPending}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (text !== "") send.mutate(text);
        }}
      >
        <DialogBody>
          <Field label={t("console.say.label")} hint={t("console.say.hint")}>
            <TextInput value={message} placeholder={t("console.say.placeholder")} maxLength={256} onChange={(e) => setMessage(e.target.value)} />
          </Field>
          {send.isError && <Notice tone="danger">{errorMessage(t, send.error)}</Notice>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" disabled={send.isPending} onClick={onClose}>
            {t("ui.cancel")}
          </Button>
          <Button type="submit" variant="primary" disabled={text === ""} loading={send.isPending}>
            {t("console.send")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}
