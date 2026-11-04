/**
 * Status bar — connection state, app/host/IPC revisions, backend
 * probe, workspace mount, and the unavailable-area capability strip.
 */

import { For, Show } from "solid-js";

import { appStore } from "../store.ts";

export function StatusBar() {
  const app = () => appStore.state.app;
  const backend = () => appStore.state.backend;
  const caps = () => appStore.state.capabilities ?? {};
  const unavailable = () =>
    Object.entries(caps()).filter(([, v]) => v.state === "unavailable");

  return (
    <footer class="status-bar">
      <div class="status-row">
        <span class={appStore.state.connected ? "dot ok" : "dot off"} />
        <span>{appStore.state.connected ? "connected" : "no bridge"}</span>
        <Show when={app()}>
          {(a) => (
            <span class="status-item mono" data-testid="status-revisions">
              {a().name} {a().version} · rvf {a().hostRevision.slice(0, 10)} · ipc {a().ipc}
            </span>
          )}
        </Show>
        <Show when={backend()}>
          {(b) => (
            <span class="status-item">
              {b().name} {b().version ?? ""} ({b().state})
            </span>
          )}
        </Show>
        <Show when={appStore.state.workspace?.mounted}>
          <span class="status-item">
            workspace mounted · shell {appStore.state.workspace?.shellWindow}
          </span>
        </Show>
      </div>
      <Show when={unavailable().length > 0}>
        <div class="status-row caps-row">
          <span class="status-label">unavailable:</span>
          <For each={unavailable()}>
            {([area]) => <span class="cap-chip">{area}</span>}
          </For>
        </div>
      </Show>
    </footer>
  );
}
