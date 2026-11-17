import { fireEvent, render, screen } from "@testing-library/react";
import { Cpu } from "lucide-react";
import { describe, expect, it } from "vitest";

import { LineChart } from "./LineChart";
import { StatTile } from "./StatTile";

const points = [
  { t: 0, v: 10 },
  { t: 5_000, v: 42 },
  { t: 10_000, v: 37 },
];
const format = {
  formatValue: (v: number) => `${v} %`,
  formatTime: (t: number) => `${t / 1000}s`,
};

describe("LineChart", () => {
  it("nhãn cho trình đọc màn hình có số mới nhất", () => {
    render(<LineChart label="CPU" points={points} yMax={100} {...format} />);
    expect(screen.getByRole("img", { name: "CPU: 37 %" })).toBeTruthy();
  });

  it("phím mũi tên đi qua từng điểm và hiện giá trị", () => {
    render(<LineChart label="CPU" points={points} yMax={100} {...format} />);
    const chart = screen.getByRole("img");
    fireEvent.keyDown(chart, { key: "ArrowLeft" });
    expect(screen.getByRole("status").textContent).toBe("42 %5s");
    fireEvent.keyDown(chart, { key: "ArrowLeft" });
    fireEvent.keyDown(chart, { key: "ArrowLeft" });
    expect(screen.getByRole("status").textContent).toBe("10 %0s");
    fireEvent.blur(chart);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("không có điểm nào vẫn vẽ trục, không lỗi", () => {
    render(<LineChart label="RAM" points={[]} {...format} />);
    expect(screen.getByRole("img", { name: "RAM" })).toBeTruthy();
  });

  it("nới trần trục y khi expandYMax bật và vẽ đường mốc tham chiếu", () => {
    render(
      <LineChart
        label="RAM"
        points={[{ t: 0, v: 8 }, { t: 5_000, v: 12 }]}
        yMax={10}
        expandYMax
        referenceValue={10}
        referenceLabel="Đặt 10 GB"
        formatValue={(v) => `${Math.round(v)} GB`}
        formatTime={(t) => `${t / 1000}s`}
      />,
    );
    expect(screen.getByText("Đặt 10 GB")).toBeTruthy();
    expect(screen.getByText("20 GB")).toBeTruthy();
  });

  it("hiện cả giá trị đỉnh khi mẫu có vMax", () => {
    render(
      <LineChart
        label="CPU"
        points={[{ t: 0, v: 20, vMax: 85 }]}
        yMax={100}
        peakPrefix="Đỉnh"
        {...format}
      />,
    );
    const chart = screen.getByRole("img");
    fireEvent.keyDown(chart, { key: "ArrowLeft" });
    expect(screen.getByRole("status").textContent).toContain("Đỉnh: 85 %");
  });
});

describe("StatTile", () => {
  it("hiện nhãn, số, đơn vị và dòng phụ", () => {
    render(<StatTile label="CPU" value="12,5" unit="%" detail="16 luồng" icon={Cpu} />);
    expect(screen.getByText("CPU")).toBeTruthy();
    expect(screen.getByText("12,5")).toBeTruthy();
    expect(screen.getByText("%")).toBeTruthy();
    expect(screen.getByText("16 luồng")).toBeTruthy();
  });
});
