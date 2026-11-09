import { describe, it, expect, vi } from "vitest";
const check = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-updater", () => ({ check }));
import { checkForUpdate } from "@/lib/updater";

describe("SwitchPlus personal updates", () => {
  it("never queries an upstream app update", async () => {
    expect(await checkForUpdate()).toEqual({ status: "up-to-date" });
    expect(check).not.toHaveBeenCalled();
  });
});
