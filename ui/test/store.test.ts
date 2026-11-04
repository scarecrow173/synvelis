import { describe, expect, it } from "vitest";

import { createAppStore, projectFromPayload } from "../src/store.ts";

const APP_INFO = {
  name: "synvelis",
  version: "0.1.0",
  host_revision: "073a3ff698c5bf9002fe21c7547b04917601e058",
  ipc: 1,
  backend: { name: "webview2", version: "131.0", state: "Available", notes: [] },
  counters: { intents: 3, intent_errors: 0 },
};

const CAPS = {
  project: { state: "available" },
  audio_runtime: { state: "unavailable", attached: false },
  signal_flow: { state: "unavailable", reason: "issue #108" },
};

describe("app store projections", () => {
  it("maps syn.app.info into app + backend", () => {
    const s = createAppStore();
    s.applyAppInfo(APP_INFO);
    expect(s.state.app?.hostRevision).toBe(APP_INFO.host_revision);
    expect(s.state.app?.ipc).toBe(1);
    expect(s.state.backend?.name).toBe("webview2");
    expect(s.state.backend?.state).toBe("Available");
  });

  it("keeps unavailable capability areas instead of dropping them", () => {
    const s = createAppStore();
    s.applyCapabilities(CAPS);
    expect(s.state.capabilities?.audio_runtime?.state).toBe("unavailable");
    expect(s.state.capabilities?.signal_flow?.reason).toBe("issue #108");
  });

  it("maps syn.workspace.mount", () => {
    const s = createAppStore();
    s.applyWorkspace({ shell_window: 1, windows: [], views: 0, mounted: true });
    expect(s.state.workspace?.shellWindow).toBe(1);
    expect(s.state.workspace?.mounted).toBe(true);
  });

  it("maps sys.capabilities into the op list", () => {
    const s = createAppStore();
    s.applySysCapabilities({
      schema: 1,
      ops: [
        { name: "sys.capabilities", summary: "" },
        { name: "syn.app.info", summary: "" },
      ],
    });
    expect(s.state.ops).toContain("syn.app.info");
  });

  it("maps syn.project.* payloads and rejects malformed ones", () => {
    expect(
      projectFromPayload({
        name: "demo",
        project_id: "project-abc",
        revision: 4,
        saved: true,
        store: true,
      }),
    ).toEqual({
      name: "demo",
      projectId: "project-abc",
      revision: 4,
      saved: true,
      store: true,
    });
    expect(projectFromPayload({ name: "demo" })).toBeNull();
    expect(projectFromPayload(null)).toBeNull();
  });

  it("routes notify pushes into project + capabilities", () => {
    const s = createAppStore();
    s.applyEvent({
      kind: "notify",
      seq: 1,
      data: {
        project: {
          name: "demo",
          project_id: "project-abc",
          revision: 1,
          saved: true,
          store: true,
        },
        capabilities: CAPS,
      },
    });
    expect(s.state.project?.name).toBe("demo");
    expect(s.state.capabilities?.project?.state).toBe("available");
  });

  it("normalizes pushed errors and rejected calls", () => {
    const s = createAppStore();
    s.applyEvent({ kind: "error", error: { code: "service_error", message: "boom" } });
    expect(s.state.lastError).toEqual({ code: "service_error", message: "boom" });
    s.recordError({ code: "unavailable", message: "no bridge" });
    expect(s.state.lastError?.code).toBe("unavailable");
    s.clearError();
    expect(s.state.lastError).toBeNull();
  });
});
