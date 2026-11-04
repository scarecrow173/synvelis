// Verify that every ruviform git dependency resolves to a single
// commit SHA in Cargo.lock, that the vendored ui tree
// (ui/vendor/ruviform/REV) matches it, and that a built frontend
// manifest (crates/synvelis-app/frontend/dist/synvelis-assets.json)
// was produced against the same rev. Synvelis runs lockfile-primary
// mode: ruviform deps track the default branch and the resolved SHA
// lives only in Cargo.lock `git+...#<sha>` source fragments — the same
// source `crates/synvelis-app/build.rs` bakes into the binary.
//
// Fail-closed: zero ruviform lockfile sources, divergent SHAs, a REV
// or manifest that disagrees are all errors. `--require-vendor` makes
// a missing REV an error — the ui verification lane uses it after
// `mise run sync-ui-deps` materializes the vendor tree.
//
// Usage:
//   node tools/check-ruviform-rev.mjs                   lockfile (+ REV/manifest if present)
//   node tools/check-ruviform-rev.mjs --require-vendor  + REV must exist and match

import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const REPO_ROOT = resolve(import.meta.dirname, "..");
const CARGO_LOCK = join(REPO_ROOT, "Cargo.lock");
const REV_FILE = join(REPO_ROOT, "ui", "vendor", "ruviform", "REV");
const MANIFEST = join(
  REPO_ROOT,
  "crates",
  "synvelis-app",
  "frontend",
  "dist",
  "synvelis-assets.json",
);
const RUVIFORM_GIT = /scarecrow173\/ruviform\.git/;
const SHA = /[0-9a-f]{40}/;

const failures = [];
const fail = (msg) => failures.push(msg);

/** Cargo.lock: resolved SHA of every ruviform git source. */
function lockfileSha() {
  const lines = readFileSync(CARGO_LOCK, "utf8").split("\n");
  const shas = new Set();
  let count = 0;
  for (const line of lines) {
    if (!line.trimStart().startsWith("source = ") || !RUVIFORM_GIT.test(line)) {
      continue;
    }
    count += 1;
    const sha = line.match(new RegExp(`#(${SHA.source})"?\\s*$`))?.[1];
    if (sha === undefined) {
      fail(`ruviform lockfile source without #<sha> fragment: ${line.trim()}`);
      continue;
    }
    shas.add(sha);
  }
  if (count === 0) {
    fail("no ruviform git sources in Cargo.lock");
    return { count, sha: null };
  }
  if (shas.size !== 1) {
    fail(`ruviform sources resolved to divergent SHAs: ${[...shas].join(", ")}`);
    return { count, sha: null };
  }
  return { count, sha: [...shas][0] };
}

const requireVendor = process.argv.includes("--require-vendor");
const lock = lockfileSha();

const revExists = existsSync(REV_FILE);
if (revExists && lock.sha !== null) {
  const vendored = readFileSync(REV_FILE, "utf8").trim();
  if (vendored !== lock.sha) {
    fail(`ui vendor REV ${vendored} != lockfile resolved ${lock.sha}`);
  }
} else if (!revExists && requireVendor) {
  fail("ui/vendor/ruviform/REV is missing — run `mise run sync-ui-deps` first");
}

if (existsSync(MANIFEST) && lock.sha !== null) {
  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8"));
  if (manifest.host !== lock.sha) {
    fail(
      `frontend manifest host ${manifest.host} != lockfile resolved ${lock.sha} — ` +
        "rebuild the bundle (`mise run build-ui`)",
    );
  }
}

if (failures.length > 0) {
  for (const f of failures) {
    process.stderr.write(`check-ruviform-rev: ${f}\n`);
  }
  process.exit(1);
}

const vendorNote = revExists ? "REV match" : "no vendor tree";
process.stdout.write(
  `check-ruviform-rev: resolved ${lock.sha} (${lock.count} lockfile sources, ` +
    `lockfile-primary, ${vendorNote})\n`,
);
