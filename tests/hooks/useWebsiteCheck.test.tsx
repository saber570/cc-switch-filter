import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { useWebsiteCheck, validWebsiteUrl } from "@/hooks/useWebsiteCheck";
import { websiteCheckApi } from "@/lib/api/websiteCheck";
import type { Provider } from "@/types";
import type { ProvidersQueryData } from "@/lib/query/queries";
import { createTestQueryClient } from "../utils/testQueryClient";

vi.mock("@/lib/api/websiteCheck", () => ({
  websiteCheckApi: { status: vi.fn(), setEnabled: vi.fn(), refresh: vi.fn() },
}));
vi.mock("@/lib/toast", () => ({ toast: { error: vi.fn() } }));

const provider: Provider = {
  id: "p",
  name: "Test",
  websiteUrl: "https://example.com/",
  settingsConfig: {},
  meta: { usage_script: { enabled: true, language: "javascript", code: "" } },
};

function setup(initial: Provider = provider) {
  const client = createTestQueryClient();
  client.setQueryData(["providers", "claude"], {
    providers: { p: initial },
    currentProviderId: "p",
  });
  const view = renderHook(({ item }) => useWebsiteCheck(item, "claude"), {
    initialProps: { item: initial },
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
  return { client, ...view };
}

describe("website checks opt-in", () => {
  it("makes no request when off; persists the toggle without writing the provider configuration", async () => {
    vi.mocked(websiteCheckApi.setEnabled).mockResolvedValue(undefined);
    vi.mocked(websiteCheckApi.status).mockResolvedValue(null);
    const { result, client, rerender } = setup();
    expect(result.current.enabled).toBe(false);
    expect(websiteCheckApi.status).not.toHaveBeenCalled();
    await act(async () => result.current.toggle.mutateAsync(true));
    expect(websiteCheckApi.setEnabled).toHaveBeenCalledWith(
      "claude",
      "p",
      true,
    );
    const saved = client.getQueryData<ProvidersQueryData>([
      "providers",
      "claude",
    ])!.providers.p;
    expect(saved.meta).toEqual({ ...provider.meta, websiteCheckEnabled: true });
    expect(saved.settingsConfig).toEqual(provider.settingsConfig);
    rerender({ item: saved });
    await waitFor(() =>
      expect(websiteCheckApi.status).toHaveBeenCalledWith("claude", "p"),
    );
    await act(async () => result.current.toggle.mutateAsync(false));
    expect(
      client.getQueryData<ProvidersQueryData>(["providers", "claude"])!
        .providers.p.meta!.websiteCheckEnabled,
    ).toBe(false);
  });

  it("keeps the opt-in off if saving fails", async () => {
    vi.mocked(websiteCheckApi.setEnabled).mockRejectedValueOnce(
      new Error("save failed"),
    );
    const { result, client } = setup();
    await act(async () => {
      await expect(result.current.toggle.mutateAsync(true)).rejects.toThrow(
        "save failed",
      );
    });
    expect(
      client.getQueryData<ProvidersQueryData>(["providers", "claude"])!
        .providers.p.meta!.websiteCheckEnabled,
    ).toBeUndefined();
    expect(websiteCheckApi.status).not.toHaveBeenCalled();
  });

  it("discards a previous site's result after editing the URL", async () => {
    vi.mocked(websiteCheckApi.status)
      .mockResolvedValueOnce({
        url: provider.websiteUrl!,
        checking: false,
        available: true,
        checkedAt: Date.now(),
        httpStatus: 200,
        error: null,
      })
      .mockResolvedValue(null);
    const enabled = { ...provider, meta: { websiteCheckEnabled: true } };
    const { result, rerender } = setup(enabled);
    await waitFor(() =>
      expect(result.current.query.data?.available).toBe(true),
    );
    rerender({
      item: { ...enabled, websiteUrl: "https://different.example.com" },
    });
    expect(result.current.query.data?.available).not.toBe(true);
    await waitFor(() => expect(result.current.query.data).toBeNull());
  });

  it.each([
    undefined,
    "",
    "file:///tmp/test",
    "ftp://example.com",
    "https://user:password@example.com",
  ])("does not enable probing for %s", (url) => {
    expect(validWebsiteUrl(url)).toBe(false);
  });
  it("accepts a normal website URL", () =>
    expect(validWebsiteUrl(" https://example.com/path ")).toBe(true));
});
