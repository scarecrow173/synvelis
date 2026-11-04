/**
 * `<App/>` — the Synvelis Application Shell root.
 *
 * Mount sequence: register the Ruviform elements → check the bridge →
 * hydrate through `sys.capabilities` + `syn.app.info` +
 * `syn.workspace.mount` → subscribe to pushed events. When the bridge
 * is absent (plain browser, tests) the shell still renders and says
 * so — it never pretends to be connected.
 */

import { onCleanup, onMount } from "solid-js";

import { registerElements } from "./adapters/ruviform.ts";
import { ErrorBanner } from "./components/ErrorBanner.tsx";
import { MenuBar } from "./components/MenuBar.tsx";
import { ProjectPanel } from "./components/ProjectPanel.tsx";
import { StatusBar } from "./components/StatusBar.tsx";
import { TransportBar } from "./components/TransportBar.tsx";
import { bridgeReady, call, onEvent } from "./ipc.ts";
import { appStore } from "./store.ts";

/** Initial hydration — the same ops a `Ready` notify mirrors. */
export async function hydrateApp(): Promise<void> {
  const [caps, info, mount] = await Promise.all([
    call("sys.capabilities"),
    call("syn.app.info"),
    call("syn.workspace.mount"),
  ]);
  appStore.applySysCapabilities(caps);
  appStore.applyAppInfo(info);
  appStore.applyWorkspace(mount);
}

export function App() {
  onMount(() => {
    registerElements();
    if (!bridgeReady()) {
      appStore.setStartupError(
        "rvf bridge unavailable — this shell must be launched by the Synvelis host",
      );
      return;
    }
    const off = onEvent((e) => appStore.applyEvent(e));
    onCleanup(off);
    hydrateApp()
      .then(() => appStore.setConnected(true))
      .catch((e: unknown) => appStore.recordError(e));
  });

  return (
    <div class="shell">
      <MenuBar />
      <ErrorBanner />
      <main class="shell-main">
        <ProjectPanel />
        <TransportBar />
      </main>
      <StatusBar />
    </div>
  );
}
