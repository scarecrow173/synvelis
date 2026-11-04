/**
 * Basic window menu — File (New/Open/Save/Quit). The project gate's
 * path entry lives in `ProjectPanel`; the menu keeps the shell-level
 * actions.
 */

import { call } from "../ipc.ts";
import { appStore } from "../store.ts";

async function save(): Promise<void> {
  try {
    appStore.applyProject(await call("syn.project.save"));
    appStore.clearError();
  } catch (e) {
    appStore.recordError(e);
  }
}

async function quit(): Promise<void> {
  try {
    await call("syn.app.quit");
  } catch (e) {
    appStore.recordError(e);
  }
}

export function MenuBar() {
  return (
    <nav class="menu-bar" aria-label="Application menu">
      <span class="menu-title">Synvelis</span>
      <div class="menu-items" role="menubar">
        <button class="menu-item" onClick={save} disabled={!appStore.state.project?.store}>
          Save
        </button>
        <button class="menu-item" onClick={quit}>
          Quit
        </button>
      </div>
    </nav>
  );
}
