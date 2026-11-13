import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DatabaseCompatibility } from "@/components/DatabaseCompatibility";

const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-process", () => ({ exit: vi.fn() }));

describe("SwitchPlus database compatibility", () => {
  it("keeps recovery available without checking or installing app updates", () => {
    invoke.mockClear();
    render(
      <DatabaseCompatibility
        payload={{ path: "test.db", db_version: 99, supported_version: 11 }}
      />,
    );
    expect(invoke).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("button", { name: /upgrade|升级|update|更新/i }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "打开配置目录" }));
    expect(invoke).toHaveBeenCalledWith("open_app_config_folder");
  });
});
