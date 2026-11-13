import { describe, expect, it } from "vitest";
import { createRequire } from "node:module";
import {
  mkdtempSync,
  mkdirSync,
  writeFileSync,
  utimesSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  checkBuildConfig,
  mergeConfig,
  collectArtifacts,
} from "../../scripts/build-switchplus.mjs";

const projectRoot = process.cwd();

describe("SwitchPlus reproducible application identity", () => {
  it("uses the same identity and icon set on Windows and macOS", () => {
    const windows = checkBuildConfig(projectRoot, "win32");
    const mac = checkBuildConfig(projectRoot, "darwin");
    expect(windows.icons).toEqual(mac.icons);
    expect(windows.config.identifier).toBe(mac.config.identifier);
    expect(windows.config.bundle.targets).toEqual(["nsis"]);
    expect(mac.config.bundle.targets).toEqual(["app", "dmg"]);
  });

  it("merges a platform window array as a replacement", () => {
    expect(
      mergeConfig(
        { app: { windows: [{ label: "old" }], other: true } },
        { app: { windows: [{ label: "main" }] } },
      ),
    ).toEqual({ app: { windows: [{ label: "main" }], other: true } });
  });

  it("resolves the CLI without a shell-specific pnpm executable", () => {
    expect(
      createRequire(join(projectRoot, "package.json")).resolve(
        "@tauri-apps/cli/tauri.js",
      ),
    ).toMatch(/tauri\.js$/);
  });

  it("does not report cached installers or other package formats as this build", () => {
    const sandbox = mkdtempSync(join(tmpdir(), "switchplus-build-test-"));
    try {
      const startedAt = Date.now();
      mkdirSync(join(sandbox, "nsis"));
      mkdirSync(join(sandbox, "msi"));
      const fresh = join(sandbox, "nsis/current.exe");
      const cached = join(sandbox, "nsis/old.exe");
      writeFileSync(fresh, "new package");
      writeFileSync(cached, "old package");
      writeFileSync(join(sandbox, "msi/current.msi"), "other format");
      utimesSync(cached, new Date(0), new Date(0));
      expect(collectArtifacts(sandbox, ["nsis"], startedAt)).toEqual([
        { path: fresh, sha256: expect.stringMatching(/^[a-f0-9]{64}$/) },
      ]);
    } finally {
      rmSync(sandbox, { recursive: true });
    }
  });
});
