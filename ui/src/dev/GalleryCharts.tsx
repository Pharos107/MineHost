import { Clock, Cpu, HardDrive, MemoryStick, Users } from "lucide-react";

import { Card, LineChart, Stack, StatGrid, StatTile, type ChartPoint } from "../ui";

const NOW = Date.UTC(2026, 9, 27, 3, 0);
const STEP = 5_000;

// 1 giờ mẫu mỗi 5 giây; có một khoảng 10 phút server tắt để thấy nét bị ngắt.
const CPU: ChartPoint[] = Array.from({ length: 720 }, (_, i) => ({
  t: NOW - (719 - i) * STEP,
  v: 18 + 10 * Math.sin(i / 40) + (i % 17 === 0 ? 25 : 0),
})).filter((_, i) => i < 300 || i > 420);

const time = (t: number) =>
  new Date(t).toLocaleTimeString("vi-VN", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" });

export function ChartSection() {
  return (
    <Card title="StatGrid / StatTile / LineChart">
      <Stack gap={4}>
        <StatGrid>
          <StatTile label="CPU" value="23,4" unit="%" icon={Cpu} detail="toàn máy" />
          <StatTile label="RAM" value="3,1" unit="GB" icon={MemoryStick} detail="trên 4 GB đã đặt" />
          <StatTile label="Người chơi" value="5" unit="/ 20" icon={Users} />
          <StatTile label="Thời gian chạy" value="2 giờ 14 phút" icon={Clock} />
          <StatTile label="Dung lượng" value="—" icon={HardDrive} detail="Đang tính…" />
        </StatGrid>
        <LineChart
          label="CPU trong 1 giờ qua"
          points={CPU}
          start={NOW - 3_600_000}
          end={NOW}
          yMax={100}
          formatValue={(v) => `${Math.round(v)} %`}
          formatTime={time}
        />
      </Stack>
    </Card>
  );
}
