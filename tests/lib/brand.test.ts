import { describe, it, expect } from "vitest";
import { brandTranslations } from "@/lib/brand";

describe("SwitchPlus displayed identity", () => {
  it("brands nested display text without changing keys or upstream links", () => {
    const source = {
      ccSwitchDir: "CC Switch: ~/.cc-switch/skills",
      urls: ["https://github.com/farion1231/cc-switch"],
    };
    expect(brandTranslations(source)).toEqual({
      ccSwitchDir: "SwitchPlus: ~/.switchplus/skills",
      urls: source.urls,
    });
    expect(source.ccSwitchDir).toBe("CC Switch: ~/.cc-switch/skills");
  });
});
