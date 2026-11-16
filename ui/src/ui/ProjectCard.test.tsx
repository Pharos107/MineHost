import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import i18n from "../i18n";
import { Badge } from "./Badge";
import { ProjectCard, ProjectChip, ProjectDetailHeader } from "./ProjectCard";

describe("ProjectCard", () => {
  it("hiện tên, mô tả, lượt tải theo ngôn ngữ, nhãn loader và phiên bản", async () => {
    await i18n.changeLanguage("vi");
    const { rerender } = render(
      <ProjectCard
        title="Sodium"
        description="Tối ưu render cho Minecraft."
        downloads={1_500_000}
        loaders={["Fabric", "Quilt"]}
        version="1.21.4"
      />,
    );
    expect(screen.getByText("Sodium")).toBeTruthy();
    expect(screen.getByText("Tối ưu render cho Minecraft.")).toBeTruthy();
    expect(screen.getByText("Fabric")).toBeTruthy();
    expect(screen.getByText("Quilt")).toBeTruthy();
    expect(screen.getByText("1.21.4")).toBeTruthy();
    const expectedVi = new Intl.NumberFormat("vi", { notation: "compact", maximumFractionDigits: 1 }).format(1_500_000);
    const downloadsVi = screen.getByText(expectedVi.replace(/\u00a0/g, " "));
    expect(downloadsVi.getAttribute("aria-label")).toBe(`${expectedVi} lượt tải`);

    await i18n.changeLanguage("en");
    rerender(
      <ProjectCard
        title="Sodium"
        description="Tối ưu render cho Minecraft."
        downloads={1_500_000}
        loaders={["Fabric"]}
        version="1.21.4"
      />,
    );
    const expectedEn = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(1_500_000);
    const downloadsEn = screen.getByText(expectedEn.replace(/\u00a0/g, " "));
    expect(downloadsEn.getAttribute("aria-label")).toBe(`${expectedEn} downloads`);
    await i18n.changeLanguage("vi");
  });

  it("bấm chọn được khi có onSelect", async () => {
    const onSelect = vi.fn();
    render(
      <ProjectCard
        title="Lithium"
        description="Tối ưu hiệu năng server."
        downloads={500_000}
        loaders={["Fabric"]}
        version="1.21.4"
        selected
        onSelect={onSelect}
      />,
    );
    const button = screen.getByRole("button", { name: /Lithium/ });
    expect(button.getAttribute("aria-pressed")).toBe("true");
    await userEvent.click(button);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("hiện nhãn trạng thái bổ sung (badge) trên thẻ", () => {
    render(
      <ProjectCard
        title="Sodium"
        description="Tối ưu render."
        downloads={1000}
        badge={<Badge tone="success">Đã chọn</Badge>}
      />,
    );
    expect(screen.getByText("Đã chọn")).toBeTruthy();
  });
});

describe("ProjectChip", () => {
  it("hiện tên, phiên bản và gọi onRemove khi bấm nút xóa", async () => {
    const onRemove = vi.fn();
    render(<ProjectChip title="Sodium" version="0.6.0" removeLabel="Bỏ Sodium" onRemove={onRemove} />);
    expect(screen.getByText("Sodium")).toBeTruthy();
    expect(screen.getByText("0.6.0")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Bỏ Sodium" }));
    expect(onRemove).toHaveBeenCalledTimes(1);
  });
});

describe("ProjectDetailHeader", () => {
  it("hiện tiêu đề có link, mô tả, badges và danh sách link ngoài", () => {
    render(
      <ProjectDetailHeader
        title="Lithium"
        description="Tối ưu server."
        pageUrl="https://modrinth.com/mod/lithium"
        badges={<Badge>Fabric</Badge>}
        links={[{ key: "source", label: "Mã nguồn", url: "https://github.com/CaffeineMC/lithium" }]}
      />,
    );
    expect(screen.getByRole("link", { name: "Lithium" }).getAttribute("href")).toBe("https://modrinth.com/mod/lithium");
    expect(screen.getByText("Tối ưu server.")).toBeTruthy();
    expect(screen.getByText("Fabric")).toBeTruthy();
    expect(screen.getByRole("link", { name: "Mã nguồn" }).getAttribute("href")).toBe("https://github.com/CaffeineMC/lithium");
  });
});
