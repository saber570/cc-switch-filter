import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { WebsiteStatusDot } from "@/components/providers/WebsiteStatusDot";
import type { WebsiteCheckStatus } from "@/lib/api/websiteCheck";

const status: WebsiteCheckStatus = {
  url: "https://example.com/",
  checking: false,
  available: true,
  checkedAt: Date.now(),
  httpStatus: 200,
  error: null,
};

describe("website status dot", () => {
  it("turns green to red and back on fresh results, and allows manual refresh", () => {
    const refresh = vi.fn();
    const view = render(
      <WebsiteStatusDot
        status={status}
        error={false}
        refreshing={false}
        onRefresh={refresh}
      />,
    );
    const green = screen.getByRole("button", { name: "网站可访问" });
    expect(green.firstChild).toHaveClass("bg-success");
    fireEvent.click(green);
    expect(refresh).toHaveBeenCalledOnce();
    view.rerender(
      <WebsiteStatusDot
        status={{
          ...status,
          available: false,
          httpStatus: 503,
          error: "HTTP 503",
        }}
        error={false}
        refreshing={false}
        onRefresh={refresh}
      />,
    );
    expect(
      screen.getByRole("button", { name: "网站不可用" }).firstChild,
    ).toHaveClass("bg-danger");
    view.rerender(
      <WebsiteStatusDot
        status={status}
        error={false}
        refreshing={false}
        onRefresh={refresh}
      />,
    );
    expect(
      screen.getByRole("button", { name: "网站可访问" }).firstChild,
    ).toHaveClass("bg-success");
  });

  it("does not present an old green result as current while checking or after IPC failure", () => {
    const view = render(
      <WebsiteStatusDot
        status={status}
        error={false}
        refreshing={true}
        onRefresh={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "网站检测中" })).toBeDisabled();
    expect(screen.getByRole("button").firstChild).toHaveClass("bg-fg-3");
    view.rerender(
      <WebsiteStatusDot
        status={status}
        error={true}
        refreshing={false}
        onRefresh={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("button", { name: "网站状态待更新" }).firstChild,
    ).toHaveClass("bg-fg-3");
  });

  it("marks an overdue result unknown", () => {
    render(
      <WebsiteStatusDot
        status={{ ...status, checkedAt: Date.now() - 12 * 60_000 }}
        error={false}
        refreshing={false}
        onRefresh={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("button", { name: "网站状态待更新" }).firstChild,
    ).toHaveClass("bg-fg-3");
  });
});
