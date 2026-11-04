/**
 * Error boundary surface — the startup asset/backend error and the
 * latest op failure are shown here, with the stable error code.
 */

import { Show } from "solid-js";

import { appStore } from "../store.ts";

export function ErrorBanner() {
  return (
    <>
      <Show when={appStore.state.startupError}>
        {(msg) => (
          <div class="banner banner-startup" role="alert">
            <span class="banner-tag">startup</span>
            <span>{msg()}</span>
          </div>
        )}
      </Show>
      <Show when={appStore.state.lastError}>
        {(err) => (
          <div class="banner banner-error" role="alert">
            <span class="banner-tag">{err().code}</span>
            <span>{err().message}</span>
            <button class="banner-close" onClick={() => appStore.clearError()}>
              ×
            </button>
          </div>
        )}
      </Show>
    </>
  );
}
