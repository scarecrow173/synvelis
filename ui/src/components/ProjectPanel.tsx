/**
 * Project panel — the New/Open/Save entry point and live project
 * summary. Paths go to the host as opaque strings; the Rust side owns
 * all filesystem semantics.
 */

import { createSignal } from "solid-js";

import { call } from "../ipc.ts";
import { appStore } from "../store.ts";

async function runOp(
  op: "syn.project.new" | "syn.project.open",
  params: Record<string, unknown>,
): Promise<void> {
  try {
    appStore.applyProject(await call(op, params));
    appStore.clearError();
  } catch (e) {
    appStore.recordError(e);
  }
}

export function ProjectPanel() {
  const [path, setPath] = createSignal("");
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const submit = (op: "syn.project.new" | "syn.project.open") => async () => {
    if (path().trim() === "" || busy()) return;
    setBusy(true);
    try {
      const params: Record<string, unknown> = { path: path().trim() };
      if (op === "syn.project.new" && name().trim() !== "") params.name = name().trim();
      await runOp(op, params);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section class="panel project-panel" aria-label="Project">
      <h2>Project</h2>
      <dl class="project-info">
        <dt>Name</dt>
        <dd data-testid="project-name">{appStore.state.project?.name ?? "—"}</dd>
        <dt>Id</dt>
        <dd class="mono">{appStore.state.project?.projectId ?? "—"}</dd>
        <dt>Revision</dt>
        <dd>{appStore.state.project?.revision ?? "—"}</dd>
        <dt>Saved</dt>
        <dd>{appStore.state.project === null ? "—" : appStore.state.project.saved ? "yes" : "no"}</dd>
      </dl>
      <form class="project-form" onSubmit={(e) => e.preventDefault()}>
        <label>
          Bundle path
          <input
            type="text"
            placeholder="C:\\…\\my-project"
            value={path()}
            onInput={(e) => setPath(e.currentTarget.value)}
          />
        </label>
        <label>
          Name (new only)
          <input
            type="text"
            placeholder="untitled"
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
          />
        </label>
        <div class="project-actions">
          <button type="button" disabled={busy()} onClick={submit("syn.project.new")}>
            New
          </button>
          <button type="button" disabled={busy()} onClick={submit("syn.project.open")}>
            Open
          </button>
        </div>
      </form>
    </section>
  );
}
