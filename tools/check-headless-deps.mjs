// Guard: headless crates must not depend on GUI/WebView/GPU/windowing
// framework crates (SV-GUI-01 boundary: GUI deps live only in
// `synvelis-app`, gated behind its `webview` feature). Invoked via
// `mise run check-headless-deps`. Portable across
// Windows/macOS/Linux (no POSIX text tools).
import { execFileSync } from "node:child_process";

// Crates that must stay free of GUI framework dependencies — the core
// project/recording/render logic, the CLI-side PoCs, and the headless
// workspace/gateway layers. `synvelis-app` is the GUI crate itself and
// is intentionally absent — the app layer owns GUI deps behind its
// `webview` feature.
const HEADLESS_CRATES = [
  "synvelis-theme",
  "synvelis-project",
  "synvelis-library",
  "synvelis-recording",
  "recording-poc",
  "synvelis-player",
  "synvelis-render",
  "synvelis-workspace",
  "synvelis-gateway",
  "synvelis-automation",
  "mcp-poc",
];

// Curated exact package names of GUI/WebView/GPU/windowing framework
// crates — plus the ruviform driver adapters themselves: the
// dependency-free `ruviform-ui` port contract is allowed, the
// wry/tao and GPUI adapters are not (SV-GPUI-01: `gpui` enters only
// through `synvelis-app`'s `native` feature, like `webview` before it).
// OS API crates (`windows-sys`, `windows-targets`, `winapi`, `libc`, …)
// are deliberately NOT listed — they are not GUI frameworks.
const EXACT = new Set([
  "azul",
  "druid",
  "eframe",
  "egui",
  "fltk",
  "gdk",
  "gdk4",
  "gpui",
  "gtk",
  "gtk4",
  "iced",
  "naga",
  "ruviform-ui-gpui",
  "ruviform-ui-webview",
  "slint",
  "tao",
  "tauri",
  "tauri-runtime",
  "webkit2gtk",
  "webview2-com",
  "wgpu",
  "winit",
  "wry",
]);

// Prefix families for scoped crate ecosystems.
const PREFIXES = [
  "egui-",
  "gdk-",
  "gpui-",
  "gtk-",
  "iced-",
  "iced_",
  "qt-",
  "qt6-",
  "tauri-",
  "webview2-",
  "wgpu-",
  "winit-",
];

function isGuiFramework(name) {
  if (EXACT.has(name)) {
    return true;
  }
  return PREFIXES.some((p) => name.startsWith(p));
}

function tree(crate) {
  return execFileSync(
    "cargo",
    ["tree", "-p", crate, "-e", "normal,build,dev", "--prefix", "none"],
    { encoding: "utf8" },
  );
}

const violations = [];
for (const crate of HEADLESS_CRATES) {
  for (const line of tree(crate).split(/\r?\n/)) {
    const m = line.match(/^([A-Za-z0-9_-]+)\s+v[\d.]+/);
    if (m && isGuiFramework(m[1])) {
      violations.push(`${crate}: ${line.trim()}`);
    }
  }
}

if (violations.length > 0) {
  process.stderr.write(
    "check-headless-deps: GUI framework dependencies found in headless crates:\n",
  );
  for (const v of violations) {
    process.stderr.write(`  ${v}\n`);
  }
  process.exit(1);
}
process.stdout.write(
  `check-headless-deps: no GUI framework deps in ${HEADLESS_CRATES.length} headless crates\n`,
);
