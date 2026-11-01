// Tests for scripts/line-limit.mjs. Run with `node --test scripts/`.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import { countLines, LIMIT, violations } from "./line-limit.mjs";

const script = join(dirname(fileURLToPath(import.meta.url)), "line-limit.mjs");
const repos = [];
after(() => {
  for (const dir of repos) rmSync(dir, { recursive: true, force: true });
});

/** A fresh git repo with `files` (path → text) written; `commit` paths are committed. */
function repo(files, commit = []) {
  const dir = mkdtempSync(join(tmpdir(), "line-limit-"));
  repos.push(dir);
  const git = (...args) => execFileSync("git", args, { cwd: dir, stdio: "pipe" });
  git("init", "-q");
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  if (commit.length > 0) {
    git("add", "--", ...commit);
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init");
  }
  return dir;
}

const lines = (n) => "x\n".repeat(n);

test("counts physical lines; a trailing newline is not an extra line", () => {
  assert.equal(countLines(""), 0);
  assert.equal(countLines("a"), 1);
  assert.equal(countLines("a\n"), 1);
  assert.equal(countLines("a\nb"), 2);
  assert.equal(countLines("\n\n"), 2);
  assert.equal(countLines("a\r\nb\r\n"), 2);
});

test("allows 500 lines and reports 501, including an unterminated last line", () => {
  assert.equal(LIMIT, 500);
  const dir = repo({
    "ok.rs": lines(500),
    "over.rs": lines(501),
    "unterminated.rs": `${lines(500)}// last`,
  });
  assert.deepEqual(violations(dir), [
    { path: "over.rs", lines: 501 },
    { path: "unterminated.rs", lines: 501 },
  ]);
});

test("finds tracked and untracked nested sources of every covered kind", () => {
  const big = lines(501);
  const dir = repo(
    {
      "crates/a/src/deep/tracked.rs": big,
      "web/src/new/untracked.ts": big,
      "web/src/style.css": big,
      "web/index.html": big,
      "scripts/tool.mjs": big,
      "scripts/run.sh": big,
      ".githooks/pre-commit": `#!/usr/bin/env bash\n${big}`,
    },
    ["crates/a/src/deep/tracked.rs"],
  );
  assert.deepEqual(
    violations(dir).map((v) => v.path),
    [
      ".githooks/pre-commit",
      "crates/a/src/deep/tracked.rs",
      "scripts/run.sh",
      "scripts/tool.mjs",
      "web/index.html",
      "web/src/new/untracked.ts",
      "web/src/style.css",
    ],
  );
});

test("skips generated, dependency, local and non-code files", () => {
  const big = lines(501);
  const dir = repo({
    ".gitignore": "ignored.rs\n",
    "ignored.rs": big,
    "web/src/pkg/sim_wasm.js": big,
    "web/node_modules/dep/index.js": big,
    "target/debug/build.rs": big,
    "vendor/lib.rs": big,
    ".scratch/probe.rs": big,
    ".claude/skills/x/run.sh": big,
    ".agents/skills/x/run.sh": big,
    "README.md": big,
    "web/package-lock.json": big,
    "Cargo.toml": big,
    "notes": big,
  });
  assert.deepEqual(violations(dir), []);
});

test("CLI reports each violation and fails, or passes quietly", () => {
  const bad = repo({ "a.rs": lines(501), "b/c.ts": lines(600), "d.rs": lines(2) });
  const r = spawnSync(process.execPath, [script, bad], { encoding: "utf8" });
  assert.equal(r.status, 1);
  assert.match(r.stderr, /a\.rs: 501 lines \(limit 500\)/);
  assert.match(r.stderr, /b\/c\.ts: 600 lines \(limit 500\)/);
  assert.doesNotMatch(r.stderr, /d\.rs/);

  const good = repo({ "a.rs": lines(500) });
  const ok = spawnSync(process.execPath, [script, good], { encoding: "utf8" });
  assert.equal(ok.status, 0, ok.stderr);
});
