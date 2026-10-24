import { describe, expect, it } from "vitest";

import { emptyHistory, historyDown, historyUp, pushCommand } from "./history";

describe("lịch sử lệnh", () => {
  const withTwo = pushCommand(pushCommand(emptyHistory, "say a"), "list");

  it("bỏ lệnh rỗng và lệnh trùng ngay trước", () => {
    expect(pushCommand(pushCommand(withTwo, "  "), "list ").entries).toEqual(["say a", "list"]);
  });

  it("lên xem lệnh cũ dần, dừng ở lệnh đầu tiên", () => {
    let step = historyUp(withTwo, "");
    expect(step.value).toBe("list");
    step = historyUp(step.history, step.value);
    expect(step.value).toBe("say a");
    step = historyUp(step.history, step.value);
    expect(step.value).toBe("say a");
  });

  it("xuống hết thì trả lại chữ đang gõ dở", () => {
    let step = historyUp(withTwo, "op Steve");
    step = historyUp(step.history, step.value);
    step = historyDown(step.history, step.value);
    expect(step.value).toBe("list");
    step = historyDown(step.history, step.value);
    expect(step.value).toBe("op Steve");
    expect(historyDown(step.history, "op Steve").value).toBe("op Steve");
  });

  it("chưa có lệnh nào thì mũi tên không đổi chữ", () => {
    expect(historyUp(emptyHistory, "abc").value).toBe("abc");
  });

  it("gửi lệnh xong thì quay về dòng gõ mới", () => {
    const step = historyUp(withTwo, "");
    expect(pushCommand(step.history, "say b")).toMatchObject({ index: 3, draft: "" });
  });
});
