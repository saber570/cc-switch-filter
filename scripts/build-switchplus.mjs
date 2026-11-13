import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const json = (path) => JSON.parse(readFileSync(path, "utf8"));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

// Platform configuration may change window appearance and package formats,
// but all builds use the same installed application identity and icon files.
export function mergeConfig(base, override) {
  if (!override || typeof override !== "object" || Array.isArray(override))
    return override;
  const merged = { ...base };
  for (const [key, value] of Object.entries(override)) {
    merged[key] =
      value && typeof value === "object" && !Array.isArray(value)
        ? mergeConfig(merged[key], value)
        : value;
  }
  return merged;
}

export function checkBuildConfig(
  projectRoot = root,
  platform = process.platform,
) {
  const pkg = json(join(projectRoot, "package.json"));
  let config = json(join(projectRoot, "src-tauri/tauri.conf.json"));
  const platformName = { win32: "windows", darwin: "macos", linux: "linux" }[
    platform
  ];
  if (!platformName) throw new Error(`Unsupported platform: ${platform}`);
  const platformFile = join(
    projectRoot,
    `src-tauri/tauri.${platformName}.conf.json`,
  );
  if (existsSync(platformFile))
    config = mergeConfig(config, json(platformFile));
  const cargo = readFileSync(join(projectRoot, "src-tauri/Cargo.toml"), "utf8");
  const cargoVersion = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const failures = [];
  const expect = (condition, label) => {
    if (!condition) failures.push(label);
  };
  expect(config.productName === "SwitchPlus", "productName must be SwitchPlus");
  expect(
    config.identifier === "io.github.saber570.switchplus",
    "installed application identifier changed",
  );
  expect(config.mainBinaryName === "switchplus", "executable name changed");
  expect(
    /\[\[bin\]\]\s*name\s*=\s*"switchplus"/.test(cargo) &&
      cargo.includes('default-run = "switchplus"'),
    "Cargo executable name changed",
  );
  expect(
    config.bundle.publisher === "saber570",
    "installer registry publisher changed",
  );
  expect(
    config.version === pkg.version && config.version === cargoVersion,
    "package/Cargo/Tauri versions differ",
  );
  expect(
    config.bundle.createUpdaterArtifacts === false && !config.plugins?.updater,
    "application updater must stay removed",
  );
  expect(
    config.bundle.windows.nsis.installMode === "currentUser",
    "Windows install scope changed",
  );
  expect(
    config.bundle.windows.allowDowngrades === false,
    "downgrade protection must stay enabled",
  );
  expect(
    config.bundle.windows.nsis.installerIcon === "icons/switchplus/icon.ico",
    "installer icon changed",
  );
  expect(
    config.bundle.windows.nsis.installerHooks === "nsis/hooks.nsh",
    "uninstaller icon hook missing",
  );
  if (platform === "win32")
    expect(
      JSON.stringify(config.bundle.targets) === '["nsis"]',
      "Windows default must be the EXE installer",
    );
  if (platform === "darwin")
    expect(
      JSON.stringify(config.bundle.targets) === '["app","dmg"]',
      "macOS default must be app and dmg",
    );
  const localFile = join(projectRoot, "src-tauri/tauri.local-build.conf.json");
  if (existsSync(localFile)) {
    expect(
      JSON.stringify(mergeConfig(config, json(localFile))) ===
        JSON.stringify(config),
      "legacy local-build config changes the standard build",
    );
  }
  const iconFiles = [
    ...config.bundle.icon,
    "icons/switchplus/icon.png",
    "icons/switchplus/tray/66x66.png",
  ];
  expect(
    config.bundle.icon.every((path) => path.startsWith("icons/switchplus/")),
    "bundle references a different icon set",
  );
  const icons = Object.fromEntries(
    iconFiles.map((path) => [
      path,
      hash(readFileSync(join(projectRoot, "src-tauri", path))),
    ]),
  );
  icons["src/assets/icons/switchplus.svg"] = hash(
    readFileSync(join(projectRoot, "src/assets/icons/switchplus.svg")),
  );
  if (failures.length) throw new Error(failures.join("\n"));
  return { config, icons, platform: platformName };
}

function git(args) {
  try {
    return execFileSync("git", args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return null;
  }
}

function filesUnder(directory) {
  if (!existsSync(directory)) return [];
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(path) : [path];
  });
}

export function collectArtifacts(bundleDir, formats, startedAt) {
  return formats
    .filter((format) => format !== "app")
    .flatMap((format) =>
      filesUnder(join(bundleDir, format))
        .filter(
          (path) =>
            /\.(exe|msi|dmg|deb|rpm|AppImage)$/.test(path) &&
            statSync(path).mtimeMs >= startedAt - 2000,
        )
        .map((path) => ({ path, sha256: hash(readFileSync(path)) })),
    );
}

export function main(args = process.argv.slice(2)) {
  let checkOnly = false;
  let target;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--check") checkOnly = true;
    else if (args[i] === "--target" && args[i + 1]) target = args[++i];
    else
      throw new Error(
        `Unknown build argument: ${args[i]}. Use --check or --target <triple>.`,
      );
  }
  if (process.env.TAURI_CONFIG)
    throw new Error(
      "Remove TAURI_CONFIG before a standard build; it can override application identity.",
    );
  const { config, icons, platform } = checkBuildConfig();
  // Always pass the resolved target so global Cargo config cannot silently
  // choose a different architecture or move the output on another machine.
  target ??= process.env.CARGO_BUILD_TARGET;
  if (!target && !checkOnly) {
    target = execFileSync("rustc", ["--version", "--verbose"], {
      cwd: root,
      encoding: "utf8",
    })
      .match(/^host: (.+)$/m)?.[1]
      ?.trim();
    if (!target) throw new Error("Unable to determine the native Rust target.");
  }
  if (
    target &&
    !target.includes(
      { win32: "windows", darwin: "apple-darwin", linux: "linux" }[
        process.platform
      ],
    )
  ) {
    throw new Error(
      "The target must match this build machine's operating system.",
    );
  }
  const trackedStatus = git([
    "status",
    "--porcelain",
    "--untracked-files=normal",
  ]);
  const sourcePatch = git(["diff", "HEAD", "--binary"]);
  const info = {
    product: config.productName,
    version: config.version,
    commit: git(["rev-parse", "HEAD"]),
    branch: git(["branch", "--show-current"]),
    sourceDirty: trackedStatus === null ? null : trackedStatus.length > 0,
    trackedPatchSha256: sourcePatch ? hash(sourcePatch) : null,
    platform,
    target: target ?? "native",
    identifier: config.identifier,
    mainBinaryName: config.mainBinaryName,
    publisher: config.bundle.publisher,
    bundles: config.bundle.targets,
    icons,
    defaultInstallPath:
      platform === "windows"
        ? "%LOCALAPPDATA%\\SwitchPlus"
        : platform === "macos"
          ? "/Applications/SwitchPlus.app"
          : null,
    existingInstall:
      "Reuse the saved installation path and existing SwitchPlus database/port.",
    node: process.version,
    packageManager: json(join(root, "package.json")).packageManager,
    rust: readFileSync(join(root, "rust-toolchain.toml"), "utf8").match(
      /channel\s*=\s*"([^"]+)"/,
    )?.[1],
  };
  console.log(JSON.stringify(info, null, 2));
  if (checkOnly) return;
  if (info.sourceDirty)
    console.warn(
      "Building local uncommitted changes. Use the same clean Git commit on both machines to compare builds.",
    );
  const cli = require.resolve("@tauri-apps/cli/tauri.js");
  const buildArgs = [cli, "build", "--ci", "--no-sign"];
  if (target) buildArgs.push("--target", target);
  buildArgs.push("--", "--locked");
  const startedAt = Date.now();
  const result = spawnSync(process.execPath, buildArgs, {
    cwd: root,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`Tauri build failed (${result.status ?? result.signal})`);
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--locked",
        "--offline",
      ],
      { cwd: join(root, "src-tauri"), encoding: "utf8" },
    ),
  );
  const targetRoot = metadata.target_directory;
  const bundleDir = join(
    targetRoot,
    ...(target ? [target] : []),
    "release/bundle",
  );
  const formats = Array.isArray(config.bundle.targets)
    ? config.bundle.targets
    : ["appimage", "deb", "rpm"];
  info.artifacts = collectArtifacts(bundleDir, formats, startedAt);
  if (!info.artifacts.length)
    throw new Error(`No freshly built installer found in ${bundleDir}`);
  info.builtAt = new Date().toISOString();
  mkdirSync(bundleDir, { recursive: true });
  const infoPath = join(bundleDir, "switchplus-build-info.json");
  writeFileSync(infoPath, JSON.stringify(info, null, 2) + "\n");
  console.log(`Build metadata: ${infoPath}`);
}

if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
