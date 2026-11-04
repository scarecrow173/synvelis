// Vite build + vitest config for the Synvelis Application Shell.
//
// - `@ruviform/*` resolves into `vendor/ruviform/ui/*/src/index.ts`
//   (the tree `tools/sync-ruviform-ui.mjs` materializes at the Cargo.lock
//   rev — the SAME rev `synvelis-assets.json` records as `host`).
// - `vite build` emits `crates/synvelis-app/frontend/dist` — the folder
//   `synvelis-app`'s `UiAssets` rust-embed reads (debug: live FS;
//   release: embedded). `write-manifest.mjs` then stamps the bundle.
// - vitest uses the happy-dom environment so the real SolidJS client
//   renderer runs (`node --test` cannot: the `node` export condition
//   resolves `solid-js/web` to the SSR build).
import { fileURLToPath } from "node:url";
import solid from "vite-plugin-solid";
import { defineConfig } from "vitest/config";

const r = (p: string) => fileURLToPath(new URL(p, import.meta.url));

export default defineConfig({
  plugins: [solid()],
  resolve: {
    alias: [
      {
        find: /^@ruviform\/ui-(.*)$/,
        replacement: `${r("./vendor/ruviform/ui")}/$1/src/index.ts`,
      },
    ],
  },
  build: {
    outDir: r("../crates/synvelis-app/frontend/dist"),
    emptyOutDir: true,
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "happy-dom",
    include: ["test/**/*.test.ts", "test/**/*.test.tsx"],
  },
});
