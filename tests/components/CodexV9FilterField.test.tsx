import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CodexV9FilterField } from "@/components/providers/forms/CodexV9FilterField";

describe("Codex V9 filter setting", () => {
  it("requires explicit opt-in and explains the consequences when enabled", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <CodexV9FilterField enabled={false} onChange={onChange} />,
    );
    expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe(
      "false",
    );
    expect(screen.queryByRole("status")).toBeNull();
    fireEvent.click(screen.getByRole("switch"));
    expect(onChange).toHaveBeenCalledWith(true);
    rerender(<CodexV9FilterField enabled onChange={onChange} />);
    expect(screen.getByRole("status").textContent).toMatch(/压缩|compaction/);
  });
});
