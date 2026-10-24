import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ChoiceGroup, Field, ProgressBar, Slider, Steps, Switch, TextInput } from ".";

describe("Field", () => {
  it("nối nhãn, gợi ý và lỗi vào ô nhập", () => {
    render(
      <Field label="Tên server" hint="1–64 ký tự." error="Tên không được để trống.">
        <TextInput defaultValue="" />
      </Field>,
    );
    const input = screen.getByLabelText("Tên server");
    expect(input.getAttribute("aria-invalid")).toBe("true");

    const ids = input.getAttribute("aria-describedby")?.split(" ") ?? [];
    const texts = ids.map((id) => document.getElementById(id)?.textContent);
    expect(texts).toEqual(["Tên không được để trống.", "1–64 ký tự."]);
  });

  it("không có lỗi thì không đánh dấu aria-invalid", () => {
    render(
      <Field label="Cổng">
        <TextInput type="number" defaultValue="25565" />
      </Field>,
    );
    const input = screen.getByLabelText("Cổng");
    expect(input.hasAttribute("aria-invalid")).toBe(false);
    expect(input.hasAttribute("aria-describedby")).toBe(false);
  });
});

describe("Switch", () => {
  it("báo giá trị mới khi bấm", async () => {
    const onChange = vi.fn();
    render(<Switch label="Tự bật" checked={false} onChange={onChange} />);
    await userEvent.click(screen.getByRole("switch", { name: "Tự bật" }));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});

describe("Slider", () => {
  it("trả về số, đọc giá trị theo formatValue và hiện vạch đề xuất", () => {
    const onChange = vi.fn();
    render(
      <Field label="RAM">
        <Slider value={2048} onChange={onChange} min={1024} max={8192} step={512} recommended={4096} formatValue={(mb) => `${mb} MB`} />
      </Field>,
    );
    const range = screen.getByLabelText("RAM");
    expect(range.getAttribute("aria-valuetext")).toBe("2048 MB");
    expect(screen.getByText("Đề xuất: 4096 MB")).toBeTruthy();

    fireEvent.change(range, { target: { value: "3072" } });
    expect(onChange).toHaveBeenCalledWith(3072);
  });
});

describe("Steps", () => {
  it("đánh dấu bước hiện tại bằng aria-current", () => {
    render(<Steps steps={["Loại", "Phiên bản", "Tên"]} current={1} />);
    const current = screen.getByText("Phiên bản").closest("li");
    expect(current?.getAttribute("aria-current")).toBe("step");
    expect(screen.getByRole("list").getAttribute("aria-label")).toBe("Bước 2 trên 3");
  });
});

describe("ProgressBar", () => {
  it("kẹp giá trị trong 0–100", () => {
    render(<ProgressBar label="Tải Java" value={140} />);
    expect(screen.getByRole("progressbar", { name: "Tải Java" }).getAttribute("aria-valuenow")).toBe("100");
  });

  it("không có value thì là thanh vô định, không có aria-valuenow", () => {
    render(<ProgressBar label="Chuẩn bị file" />);
    expect(screen.getByRole("progressbar").hasAttribute("aria-valuenow")).toBe(false);
  });
});

describe("ChoiceGroup", () => {
  it("là nhóm radio có tên, lựa chọn bị khóa thì không chọn được", async () => {
    const onChange = vi.fn();
    render(
      <ChoiceGroup
        label="Loại server"
        value="paper"
        onChange={onChange}
        options={[
          { value: "paper", label: "Paper" },
          { value: "vanilla", label: "Vanilla", disabled: true },
          { value: "fabric", label: "Fabric" },
        ]}
      />,
    );
    expect(screen.getByRole("group", { name: "Loại server" })).toBeTruthy();
    expect(screen.getByRole("radio", { name: "Vanilla" })).toHaveProperty("disabled", true);
    await userEvent.click(screen.getByText("Vanilla"));
    expect(onChange).not.toHaveBeenCalled();
    await userEvent.click(screen.getByText("Fabric"));
    expect(onChange).toHaveBeenLastCalledWith("fabric");
  });
});
