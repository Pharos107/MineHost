import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import { ConfirmDialog, CopyField, Dialog, Menu, Tabs } from ".";

describe("ConfirmDialog", () => {
  it("chỉ cho xác nhận khi gõ đúng chuỗi yêu cầu", async () => {
    const onConfirm = vi.fn();
    render(
      <ConfirmDialog open onCancel={vi.fn()} onConfirm={onConfirm} title="Xóa server?" confirmLabel="Xóa" requireText="Survival">
        <p>Server sẽ được chuyển vào thùng rác.</p>
      </ConfirmDialog>,
    );
    const confirm = screen.getByRole("button", { name: "Xóa" });
    expect(confirm).toHaveProperty("disabled", true);

    const input = screen.getByLabelText("Gõ “Survival” để xác nhận");
    expect(document.activeElement).toBe(input);
    await userEvent.type(input, "survival");
    expect(confirm).toHaveProperty("disabled", true);

    await userEvent.clear(input);
    await userEvent.type(input, "Survival{Enter}");
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it("xóa chữ đã gõ sau khi đóng rồi mở lại", async () => {
    function Harness() {
      const [open, setOpen] = useState(true);
      return (
        <>
          <button onClick={() => setOpen(true)}>mở</button>
          <ConfirmDialog open={open} onCancel={() => setOpen(false)} onConfirm={vi.fn()} title="Xóa?" confirmLabel="Xóa" requireText="A">
            <p>?</p>
          </ConfirmDialog>
        </>
      );
    }
    render(<Harness />);
    await userEvent.type(screen.getByRole("textbox"), "A");
    await userEvent.click(screen.getByRole("button", { name: "Hủy" }));
    await userEvent.click(screen.getByRole("button", { name: "mở", hidden: true }));
    expect(screen.getByRole("textbox")).toHaveProperty("value", "");
  });
});

describe("Dialog", () => {
  it("Esc gọi onClose khi dismissible, bỏ qua khi không", () => {
    const onClose = vi.fn();
    const { rerender } = render(
      <Dialog open onClose={onClose} title="EULA">
        <p>?</p>
      </Dialog>,
    );
    const pressEsc = () => fireEvent(screen.getByRole("dialog", { hidden: true }), new Event("cancel", { cancelable: true }));
    pressEsc();
    expect(onClose).toHaveBeenCalledTimes(1);

    rerender(
      <Dialog open onClose={onClose} title="EULA" dismissible={false}>
        <p>?</p>
      </Dialog>,
    );
    pressEsc();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Đóng" })).toBeNull();
  });
});

describe("Tabs", () => {
  function Harness() {
    const [value, setValue] = useState("a");
    const items = ["a", "b", "c"].map((id) => ({ id, label: `Tab ${id}`, panel: <p>Nội dung {id}</p> }));
    return <Tabs label="Trang server" items={items} value={value} onChange={setValue} />;
  }

  it("phím mũi tên chuyển tab và vòng lại đầu", async () => {
    render(<Harness />);
    await userEvent.click(screen.getByRole("tab", { name: "Tab a" }));
    await userEvent.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Tab b" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tabpanel").textContent).toBe("Nội dung b");

    await userEvent.keyboard("{End}{ArrowRight}");
    expect(document.activeElement?.textContent).toBe("Tab a");
  });
});

describe("Menu", () => {
  it("mở, chọn mục và đóng; Esc đóng và trả focus về nút", async () => {
    const onSelect = vi.fn();
    render(<Menu label="Thao tác khác" items={[{ label: "Buộc dừng", tone: "danger", onSelect }]} />);
    const trigger = screen.getByRole("button", { name: "Thao tác khác" });

    await userEvent.click(trigger);
    expect(document.activeElement).toBe(screen.getByRole("menuitem", { name: "Buộc dừng" }));
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);

    await userEvent.click(trigger);
    await userEvent.click(screen.getByRole("menuitem", { name: "Buộc dừng" }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("bấm ra ngoài thì đóng", async () => {
    render(
      <>
        <Menu label="Thêm" items={[{ label: "X", onSelect: vi.fn() }]} />
        <p>ngoài</p>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Thêm" }));
    await userEvent.click(screen.getByText("ngoài"));
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

describe("CopyField", () => {
  it("chép giá trị vào clipboard và báo đã chép", async () => {
    const user = userEvent.setup();
    render(<CopyField value="192.168.1.10:25565" />);
    await user.click(screen.getByRole("button", { name: "Chép" }));
    expect(await navigator.clipboard.readText()).toBe("192.168.1.10:25565");
    expect(screen.getByText("Đã chép")).toBeTruthy();
  });
});
