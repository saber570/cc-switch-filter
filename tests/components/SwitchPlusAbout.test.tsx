import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
vi.mock("@tauri-apps/api/app", () => ({ getVersion: async () => "1.0.0" }));
import { AboutSection } from "@/components/settings/AboutSection";

describe("SwitchPlus about", () => {
  it("shows personal identity and attribution without an app update action", async () => {
    render(<AboutSection isPortable={false} />);
    await waitFor(() => expect(screen.getByText("v1.0.0")).toBeInTheDocument());
    expect(screen.getByText("SwitchPlus")).toBeInTheDocument();
    expect(screen.queryByText(/cc[ -]?switch/i)).toBeNull();
    expect(screen.getByText("chow")).toBeInTheDocument();
    expect(screen.getByText(/15731/)).toBeInTheDocument();
    expect(screen.getByText(/MIT/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /检查更新|Check for updates/i }),
    ).toBeNull();
  });
});
