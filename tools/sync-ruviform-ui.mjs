// Sync the ruviform `ui/*` TypeScript packages (ui/core, ui/components,
// ui/binding, ui/viz — ruviform Issues #258–#261) into `ui/vendor/ruviform/`
// at the same commit SHA the Cargo workspace resolves.
//
// Ruviform is consumed via default-branch-tracking git deps: the resolved
// revision lives in `Cargo.lock` (`source = "git+<url>#<rev>"` on the
// `ruviform-*` packages) — it is the single source of truth shared with
// `crates/synvelis-app/build.rs`. The `ui/*` packages are `private` npm
// packages that ship as TS source, so there is no registry mechanism;
// materializing them here mirrors cargo's git checkout. The vendored tree
// is a gitignored generated artifact restored by `mise run setup`/
// `mise run sync-ui-deps`.
//
// The source is the *local* cargo checkout (`~/.cargo/git/checkouts/`),
// so sync works offline — no network fetch beyond the one `cargo fetch`
// that populated the checkout.
//
// Usage:
//   node tools/sync-ruviform-ui.mjs           ensure the vendor tree matches the lock
//   node tools/sync-ruviform-ui.mjs --check   fail unless it already matches

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, cpSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const REPO_ROOT = resolve(import.meta.dirname, "..");
const VENDOR_DIR = join(REPO_ROOT, "ui", "vendor", "ruviform");
const REV_FILE = join(VENDOR_DIR, "REV");
// `ui/guitest` is vendored for the shared RF-GT fixture corpus +
// ContractModel replay lane (test-only — never imported by the bundle).
const PACKAGES = ["ui/core", "ui/components", "ui/binding", "ui/viz", "ui/guitest"];

/** Read the resolved ruviform git revision + url from Cargo.lock. */
export function readPin(lockPath = join(REPO_ROOT, "Cargo.lock")) {
  const lock = readFileSync(lockPath, "utf8");
  const revs = new Set();
  let url;
  for (const block of lock.split("[[package]]")) {
    if (!/name = "ruviform-[^"]+"/.test(block)) continue;
    const src = block.match(/source = "git\+([^"#]+)#([0-9a-f]{40})"/);
    if (src === null) continue;
    url ??= src[1];
    revs.add(src[2]);
  }
  if (revs.size !== 1 || url === undefined) {
    throw new Error(
      `ruviform revs diverged or missing (${[...revs].join(", ") || "<none>"}); ` +
        "Cargo.lock must resolve every ruviform-* dep to one rev",
    );
  }
  return { rev: [...revs][0], url };
}

/** Locate the cargo git checkout for `rev` under ~/.cargo/git/checkouts. */
function findCheckout(rev) {
  const checkouts = join(homedir(), ".cargo", "git", "checkouts");
  for (const entry of readdirSync(checkouts)) {
    if (!entry.startsWith("ruviform-")) continue;
    const base = join(checkouts, entry);
    for (const sub of readdirSync(base)) {
      if (!rev.startsWith(sub)) continue;
      const candidate = join(base, sub);
      if (PACKAGES.every((p) => existsSync(join(candidate, p, "src")))) {
        return candidate;
      }
    }
  }
  throw new Error(
    `no local cargo checkout for ruviform@${rev.slice(0, 12)} under ${checkouts} — ` +
      "run `cargo fetch` (or `mise run check`) first",
  );
}

function syncedRev() {
  return existsSync(REV_FILE) ? readFileSync(REV_FILE, "utf8").trim() : undefined;
}

function vendorComplete() {
  return PACKAGES.every((p) => existsSync(join(VENDOR_DIR, p, "src", "index.ts")));
}

const SKIP = new Set(["node_modules", "target", "dist", ".git"]);

function sync(pin) {
  const checkout = findCheckout(pin.rev);
  rmSync(VENDOR_DIR, { recursive: true, force: true });
  mkdirSync(VENDOR_DIR, { recursive: true });
  for (const pkg of PACKAGES) {
    cpSync(join(checkout, pkg), join(VENDOR_DIR, pkg), {
      recursive: true,
      filter: (src) => !SKIP.has(src.split(/[\\/]/).pop()),
    });
  }
  writeFileSync(REV_FILE, `${pin.rev}\n`);
  if (!vendorComplete()) {
    throw new Error(`vendor extraction incomplete in ${VENDOR_DIR}`);
  }
  console.log(`synced ruviform ui/* @ ${pin.rev.slice(0, 10)} -> ui/vendor/ruviform`);
}

const invokedDirectly =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href;

if (invokedDirectly) {
  const checkOnly = process.argv.includes("--check");
  const pin = readPin();
  const current = syncedRev();
  if (current === pin.rev && vendorComplete()) {
    if (!checkOnly) console.log(`ruviform ui/* already synced @ ${pin.rev.slice(0, 10)}`);
    process.exit(0);
  }
  if (checkOnly) {
    console.error(`ruviform ui/* vendor out of sync: vendored=${current ?? "<none>"} pin=${pin.rev}`);
    console.error("run `node tools/sync-ruviform-ui.mjs` (or `mise run sync-ui-deps`)");
    process.exit(1);
  }
  sync(pin);
}
