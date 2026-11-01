#!/usr/bin/env node
// Fails if any hand-written source file is over LIMIT physical lines.
//   node scripts/line-limit.mjs [repo-root]
// Scans tracked and untracked (not gitignored) files, so new files can't slip past. Generated
// WASM bindings, dependencies, build output, local scratch and skills, docs and config data
// are skipped. Extensionless files count when they start with a shebang.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export const LIMIT = 500;

const SOURCE = /\.(rs|ts|tsx|mts|cts|js|jsx|mjs|cjs|sh|css|html)$/;
const SKIP_PREFIX = ["web/src/pkg/", ".scratch/", ".claude/", ".agents/"];
const SKIP_DIR = /(^|\/)(node_modules|target|vendor)\//;

/** Physical lines, counting blanks and comments; a trailing newline is not an extra line. */
export function countLines(text) {
  if (text === "") return 0;
  const breaks = text.split("\n").length - 1;
  return text.endsWith("\n") ? breaks : breaks + 1;
}

function isSource(root, path) {
  if (SKIP_PREFIX.some((p) => path.startsWith(p)) || SKIP_DIR.test(path)) return false;
  if (SOURCE.test(path)) return true;
  const name = path.slice(path.lastIndexOf("/") + 1);
  return !name.includes(".") && readFileSync(join(root, path), "utf8").startsWith("#!");
}

/** Source files over LIMIT under `root`, sorted by path. */
export function violations(root) {
  const listed = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 },
  );
  const paths = [...new Set(listed.split("\0").filter(Boolean))].sort();
  return paths
    .filter((path) => existsSync(join(root, path)) && isSource(root, path))
    .map((path) => ({ path, lines: countLines(readFileSync(join(root, path), "utf8")) }))
    .filter((f) => f.lines > LIMIT);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = process.argv[2] ?? join(dirname(fileURLToPath(import.meta.url)), "..");
  const found = violations(root);
  for (const { path, lines } of found) {
    console.error(`${path}: ${lines} lines (limit ${LIMIT})`);
  }
  if (found.length > 0) {
    console.error(`${found.length} file(s) over ${LIMIT} lines; split them by responsibility.`);
    process.exit(1);
  }
}
