import { execFileSync, spawnSync } from "child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "fs";
import { join, relative, resolve } from "path";

const root = resolve(import.meta.dirname, "..");
const allowDirty = process.argv.includes("--allow-dirty");

const blockedDirs = new Set([
  ".git",
  ".playwright-mcp",
  ".direnv",
  "archive",
  "blob-report",
  "dist",
  "dist-ssr",
  "node_modules",
  "playwright-report",
  "result",
  "target",
  "test-results",
]);

const blockedTrackedPath =
  /^(node_modules|dist|dist-ssr|src-tauri\/target|target|playwright-report|test-results|blob-report|archive|\.playwright-mcp|result)(\/|$)/;
const textDecoder = new TextDecoder("utf-8", { fatal: false });

const secretPatterns: Array<[string, RegExp]> = [
  [
    "private key block",
    /-----BEGIN (?:RSA |DSA |EC |OPENSSH |PGP )?PRIVATE KEY-----/,
  ],
  ["GitHub classic token", /\bghp_[A-Za-z0-9_]{30,}\b/],
  ["GitHub fine-grained token", /\bgithub_pat_[A-Za-z0-9_]{20,}\b/],
  ["OpenAI style token", /\bsk-[A-Za-z0-9]{20,}\b/],
  ["AWS access key", /\bAKIA[0-9A-Z]{16}\b/],
  ["Slack token", /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/],
  [
    "secret env assignment",
    /\b(?:OPENAI_API_KEY|DASHSCOPE_API_KEY|ANTHROPIC_API_KEY|AZURE_CLIENT_SECRET|TAURI_SIGNING_PRIVATE_KEY|TAURI_SIGNING_PRIVATE_KEY_PASSWORD)\s*=\s*["']?(?!\$|\{\{|\$\{)[A-Za-z0-9_./+=-]{12,}/,
  ],
];

function git(args: string[]): string {
  return execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
}

function splitNullList(output: string): string[] {
  return output.split("\0").filter(Boolean);
}

function runStep(
  name: string,
  command: string,
  args: string[],
  cwd = root,
): boolean {
  console.log(`\n==> ${name}`);
  console.log(`+ ${[command, ...args].join(" ")}`);

  const result = spawnSync(command, args, {
    cwd,
    env: {
      ...process.env,
      CMAKE_POLICY_VERSION_MINIMUM: "3.5",
    },
    stdio: "inherit",
  });

  if (result.status === 0) return true;

  console.error(`\n[source-release-preflight] ${name} failed.`);
  return false;
}

function relativePath(path: string): string {
  return relative(root, path).replaceAll("\\", "/");
}

function isExampleEnvFile(fileName: string): boolean {
  return /\.(example|sample|template)$/i.test(fileName);
}

function isSensitiveFileName(fileName: string): boolean {
  if (fileName === ".env") return true;
  if (fileName.startsWith(".env.") && !isExampleEnvFile(fileName)) return true;
  if (/\.(pem|key|p12|pfx|mobileprovision|keystore)$/i.test(fileName)) {
    return true;
  }
  return /^id_(rsa|ed25519)/.test(fileName);
}

function walkRepo(dir: string, files: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory() && blockedDirs.has(entry.name)) continue;

    const fullPath = join(dir, entry.name);
    const rel = relativePath(fullPath);

    if (rel === "src-tauri/target" || rel.startsWith("src-tauri/target/")) {
      continue;
    }

    if (entry.isDirectory()) {
      walkRepo(fullPath, files);
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }
  return files;
}

function isProbablyTextFile(path: string): boolean {
  const stats = statSync(path);
  if (stats.size > 2_000_000) return false;

  const bytes = readFileSync(path);
  if (bytes.includes(0)) return false;

  return true;
}

function checkGitStatus(): boolean {
  console.log("\n==> git working tree");
  const status = git(["status", "--short", "--branch"]);
  console.log(status);

  const lines = status.split("\n").filter(Boolean).slice(1);
  if (lines.length === 0) return true;

  if (allowDirty) {
    console.warn(
      "[source-release-preflight] continuing with dirty working tree because --allow-dirty was provided.",
    );
    return true;
  }

  console.error(
    "[source-release-preflight] working tree must be clean before source release.",
  );
  return false;
}

function checkTrackedReleaseNoise(): boolean {
  console.log("\n==> tracked release noise");
  const tracked = splitNullList(git(["ls-files", "-z"]));
  const blocked = tracked.filter((file) => blockedTrackedPath.test(file));

  if (blocked.length === 0) {
    console.log("No generated or local-only paths are tracked.");
    return true;
  }

  console.error(blocked.join("\n"));
  return false;
}

function checkSensitiveFileNames(): boolean {
  console.log("\n==> sensitive file names");
  const matches = walkRepo(root).filter((file) =>
    isSensitiveFileName(file.split(/[\\/]/).at(-1) ?? ""),
  );

  if (matches.length === 0) {
    console.log(
      "No private key, certificate, or env files found in repo tree.",
    );
    return true;
  }

  console.error(matches.map(relativePath).join("\n"));
  return false;
}

function checkSecretPatterns(): boolean {
  console.log("\n==> secret pattern scan");
  const tracked = splitNullList(git(["ls-files", "-z"]));
  const untracked = splitNullList(
    git(["ls-files", "--others", "--exclude-standard", "-z"]),
  );
  const files = [...new Set([...tracked, ...untracked])];
  const findings: string[] = [];

  for (const file of files) {
    const fullPath = join(root, file);
    if (!existsSync(fullPath) || !isProbablyTextFile(fullPath)) continue;

    const content = textDecoder.decode(readFileSync(fullPath));
    for (const [name, pattern] of secretPatterns) {
      if (pattern.test(content)) findings.push(`${file}: ${name}`);
    }
  }

  if (findings.length === 0) {
    console.log("No common secret patterns found in tracked or visible files.");
    return true;
  }

  console.error(findings.join("\n"));
  return false;
}

function warnAboutCatalogChecksums(): void {
  const catalogPath = join(root, "src-tauri/resources/models/catalog.json");
  if (!existsSync(catalogPath)) return;

  const catalog = JSON.parse(readFileSync(catalogPath, "utf8")) as Array<{
    id?: string;
    sha256?: string | null;
  }>;
  const missing = catalog
    .filter((entry) => !entry.sha256)
    .map((entry) => entry.id ?? "unknown");

  if (missing.length > 0) {
    console.warn(
      `\n[source-release-preflight] non-blocking: model catalog entries without sha256: ${missing.join(", ")}`,
    );
  }
}

const localChecks: Array<[string, string, string[], string?]> = [
  ["frontend vulnerability audit", "bun", ["audit"]],
  ["frontend lint", "bun", ["run", "lint"]],
  ["format check", "bun", ["run", "format:check"]],
  ["translation completeness", "bun", ["run", "check:translations"]],
  ["frontend production build", "bun", ["run", "build"]],
  ["rust cargo check", "cargo", ["check"], join(root, "src-tauri")],
];

let ok = true;
ok = checkGitStatus() && ok;
ok = checkTrackedReleaseNoise() && ok;
ok = checkSensitiveFileNames() && ok;
ok = checkSecretPatterns() && ok;
warnAboutCatalogChecksums();

for (const [name, command, args, cwd] of localChecks) {
  ok = runStep(name, command, args, cwd) && ok;
}

if (!ok) {
  console.error("\n[source-release-preflight] failed.");
  process.exit(1);
}

console.log("\n[source-release-preflight] passed.");
