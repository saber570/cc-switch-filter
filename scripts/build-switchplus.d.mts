export function mergeConfig(
  base: Record<string, unknown>,
  override: Record<string, unknown>,
): Record<string, unknown>;
export function checkBuildConfig(
  projectRoot?: string,
  platform?: string,
): {
  config: {
    identifier: string;
    bundle: { targets: string[] };
  };
  icons: Record<string, string>;
  platform: string;
};
export function main(args?: string[]): void;
export function collectArtifacts(
  bundleDir: string,
  formats: string[],
  startedAt: number,
): { path: string; sha256: string }[];
