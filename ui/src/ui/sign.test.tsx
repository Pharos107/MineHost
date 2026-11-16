import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BlockWordmark, HangingSign } from ".";
import { cellsPath, crackPaths, extrusionLayers, noiseTile, WORDMARK_ROWS, wordmarkCells } from "./wordmark";

describe("wordmarkCells", () => {
  it("đánh dấu cạnh lộ ra để vẽ gờ sáng trên và gờ tối dưới", () => {
    const { cells, columns } = wordmarkCells(["T"]);
    expect(columns).toBe(12);
    const at = (x: number, y: number) => cells.find((c) => c.x === x && c.y === y);
    expect(at(0, 0)?.topEdge).toBe(true);
    expect(at(5, 5)?.topEdge).toBe(false);
    expect(at(5, WORDMARK_ROWS - 1)?.bottomEdge).toBe(true);
  });

  it("chữ cách nhau một ô, hai từ cách nhau ba ô; chữ O khoét mặt Creeper ở giữa; ký tự lạ bị bỏ qua", () => {
    expect(wordmarkCells(["MI"]).columns).toBe(14 + 1 + 5);
    expect(wordmarkCells(["MI"]).letterStarts).toEqual([0, 15]);
    expect(wordmarkCells(["I", "I"]).columns).toBe(5 + 3 + 5);
    expect(wordmarkCells(["I?"]).columns).toBe(5);
    const oCells = wordmarkCells(["O"]).cells;
    expect(oCells.some((c) => c.x === 4 && c.y === 4)).toBe(false);
    expect(oCells.some((c) => c.x === 6 && c.y === 4)).toBe(true);
  });
});

describe("hình vẽ logo", () => {
  it("gộp ô liền nhau trong một hàng thành một dải", () => {
    expect(cellsPath([{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 3, y: 0 }])).toBe("M0 0h2v1h-2zM3 0h1v1h-1z");
  });

  it("texture đá ổn định giữa các lần gọi và phủ kín ô", () => {
    const a = noiseTile(6, 1, ["#000", "#111", "#222"], 3);
    expect(noiseTile(6, 1, ["#000", "#111", "#222"], 3)).toEqual(a);
    const area = a.flatMap(({ d }) => [...d.matchAll(/h(\d+)/g)].map((m) => Number(m[1]))).reduce((s, w) => s + w, 0);
    expect(area).toBe(36);
  });

  it("khối 3D vẽ từ lớp xa (co nhiều, tối hơn) tới lớp gần mặt chữ", () => {
    const layers = extrusionLayers();
    const first = layers[0];
    const last = layers[layers.length - 1];
    expect(first && last && first.scale < last.scale && last.scale < 1).toBe(true);
    expect(first && last && first.shade > last.shade).toBe(true);
  });

  it("chỉ vẽ vết nứt nằm trọn trên mặt chữ", () => {
    const { cells, letterStarts } = wordmarkCells(["MineHost"]);
    expect(crackPaths(cells, letterStarts).dark).not.toBe("");
    const narrow = wordmarkCells(["I"]);
    // Chữ I đứng đầu dùng vết nứt của chữ M; hai nét ở cột 10–11 nằm ngoài chữ I nên bị bỏ.
    expect(crackPaths(narrow.cells, narrow.letterStarts).dark).toBe("M0 9.35h2v0.4h-2zM2.35 9h0.4v3h-0.4z");
  });
});

describe("BlockWordmark và HangingSign", () => {
  it("logo là ảnh có nhãn; bảng hiệu có tiêu đề đúng cấp và chân bảng", () => {
    render(
      <HangingSign logo={<BlockWordmark label="MineHost" />} title="Đăng nhập" titleAs="h1" footer={<p>chân</p>}>
        <p>thân</p>
      </HangingSign>,
    );
    expect(screen.getByRole("img", { name: "MineHost" })).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Đăng nhập" })).toBeTruthy();
    expect(screen.getByText("chân").closest("footer")).toBeTruthy();
  });
});
