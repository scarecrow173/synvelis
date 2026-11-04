import { afterEach, describe, expect, it } from "vitest";
import { render } from "solid-js/web";

import type { RvfTransport } from "@ruviform/ui-components";

import { App } from "../src/app.tsx";
import { appStore } from "../src/store.ts";
import { installFakeHost, type FakeHost } from "./fake-host.ts";

const APP_INFO = {
  name: "synvelis",
  version: "0.1.0",
  host_revision: "073a3ff698c5bf9002fe21c7547b04917601e058",
  ipc: 1,
  backend: { name: "webview2", version: "131.0", state: "Available", notes: [] },
  counters: { intents: 0, intent_errors: 0 },
};

const MOUNT = { shell_window: 1, windows: [], views: 0, mounted: true };

const SYS_CAPS = {
  schema: 1,
  os: "windows",
  ops: [{ name: "syn.app.info" }, { name: "syn.workspace.mount" }],
  data: {},
  jobs: { running: 0 },
};

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
}

function answerHydration(host: FakeHost): void {
  host.answerOp("sys.capabilities", SYS_CAPS);
  host.answerOp("syn.app.info", APP_INFO);
  host.answerOp("syn.workspace.mount", MOUNT);
}

describe("App shell", () => {
  let host: FakeHost | undefined;
  let dispose: (() => void) | undefined;
  let root: HTMLDivElement | undefined;

  afterEach(() => {
    dispose?.();
    dispose = undefined;
    root?.remove();
    root = undefined;
    host?.uninstall();
    host = undefined;
    appStore.clearError();
    appStore.setConnected(false);
    appStore.setStartupError("");
    appStore.setTransportPlaying(false);
  });

  function mount(): HTMLDivElement {
    root = document.createElement("div");
    document.body.appendChild(root);
    dispose = render(() => <App />, root!);
    return root;
  }

  it("mounts, registers rvf elements, and hydrates through syn.* ops", async () => {
    host = installFakeHost();
    const el = mount();
    await flush();
    const ops = host.sent.map((s) => s.op);
    expect(ops).toContain("sys.capabilities");
    expect(ops).toContain("syn.app.info");
    expect(ops).toContain("syn.workspace.mount");

    answerHydration(host);
    await flush();

    expect(customElements.get("rvf-transport")).toBeDefined();
    expect(el.querySelector("rvf-transport")).not.toBeNull();
    expect(el.textContent).toContain("connected");
    expect(el.textContent).toContain("workspace mounted");
    expect(appStore.state.app?.hostRevision).toBe(APP_INFO.host_revision);
  });

  it("shows the startup error when the bridge is absent", async () => {
    const el = mount();
    await flush();
    expect(el.querySelector(".banner-startup")).not.toBeNull();
    expect(el.textContent).toContain("rvf bridge unavailable");
  });

  it("drives rvf-transport: CustomEvent → syn.transport.set intent", async () => {
    host = installFakeHost();
    mount();
    await flush();
    answerHydration(host);
    await flush();

    const transport = document.querySelector("rvf-transport") as RvfTransport;
    expect(transport).toBeTruthy();
    // Typed property written by the effect — attribute-backed `state`.
    expect(transport.state).toBe("stopped");
    transport.dispatchEvent(
      new CustomEvent("rvf-transport", {
        detail: { action: "play", controlId: "t", sequence: 1 },
      }),
    );
    await flush();
    const intent = host.sent.find((s) => s.op === "syn.transport.set");
    expect(intent).toBeDefined();
    expect(intent!.params).toEqual({ playing: true });
    host.answerOp("syn.transport.set", { playing: true });
    await flush();
    expect(transport.state).toBe("playing");
  });

  it("presents an op failure in the error banner with its code", async () => {
    host = installFakeHost();
    const el = mount();
    await flush();
    host.answerOpErr("syn.app.info", "service_error", "backend exploded");
    host.answerOp("sys.capabilities", SYS_CAPS);
    host.answerOp("syn.workspace.mount", MOUNT);
    await flush();
    const banner = el.querySelector(".banner-error");
    expect(banner).not.toBeNull();
    expect(banner!.textContent).toContain("service_error");
    expect(banner!.textContent).toContain("backend exploded");
  });
});
