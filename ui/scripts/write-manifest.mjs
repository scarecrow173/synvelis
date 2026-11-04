// Emit `crates/synvelis-app/frontend/dist/synvelis-assets.json` — the
// manifest `synvelis_app::revision::check_bundle` validates at startup.
//
// Fields are contract-locked with the Rust side:
//   schema    — APP_ASSET_SCHEMA (manifest shape version)
//   frontend  — provenance string (pkg@version+git sha when available)
//   host      — the resolved ruviform rev from Cargo.lock — the SAME
//               source build.rs bakes into SYNVELIS_RUVIFORM_REV, so an
//               honest build always matches
//   ipc       — rvf.ipc.v1 schema version (ruviform_ui::ipc::SCHEMA_VERSION)
//   files     — { path: { bytes, sha256 } } for every bundle file
//               (the manifest itself is excluded)

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

import { readPin } from "../../tools/sync-ruviform-ui.mjs";

const UI_ROOT = resolve(import.meta.dirname, "..");
const REPO_ROOT = resolve(UI_ROOT, "..");
const DIST = resolve(REPO_ROOT, "crates", "synvelis-app", "frontend", "dist");
const MANIFEST = "synvelis-assets.json";
const APP_ASSET_SCHEMA = 1;
const IPC_SCHEMA = 1; // ruviform_ui::ipc::SCHEMA_VERSION — keep in sync

function frontendRev() {
  const pkg = JSON.parse(readFileSync(join(UI_ROOT, "package.json"), "utf8"));
  try {
    const sha = execFileSync(
      "git",
      ["-C", REPO_ROOT, "rev-parse", "--short=12", "HEAD"],
      { encoding: "utf8" },
    ).trim();
    return `${pkg.name}@${pkg.version}+${sha}`;
  } catch {
    return `${pkg.name}@${pkg.version}`;
  }
}

function* walk(dir, prefix = "") {
  for (const name of readdirSync(dir).sort()) {
    if (name === ".gitkeep" || name === MANIFEST) continue;
    const full = join(dir, name);
    const rel = prefix === "" ? name : `${prefix}/${name}`;
    if (statSync(full).isDirectory()) yield* walk(full, rel);
    else yield rel;
  }
}

const files = {};
for (const rel of walk(DIST)) {
  const bytes = readFileSync(join(DIST, rel));
  files[rel] = {
    bytes: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  };
}

const manifest = {
  schema: APP_ASSET_SCHEMA,
  frontend: frontendRev(),
  host: readPin().rev,
  ipc: IPC_SCHEMA,
  files,
};
writeFileSync(join(DIST, MANIFEST), `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote ${MANIFEST} (${Object.keys(files).length} files)`);
