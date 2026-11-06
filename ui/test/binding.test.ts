/**
 * SV-GUI-02 frontend contract tests — `BindingPort` over the fake
 * `__rvf` host, projection routing, upstream DOM binders, and the
 * `ConsoleViewModel` lifecycle. happy-dom runs the real element
 * classes; the fake host plays the Rust side.
 */

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  bindControlElement,
  bindTransportElement,
  CommandBinding,
  type CommandReceipt,
} from "@ruviform/ui-binding";
import type { RvfKnob, RvfTransport } from "@ruviform/ui-components";

import { registerElements } from "../src/adapters/ruviform.ts";
import { createIpcBindingPort } from "../src/binding/port.ts";
import {
  createRouter,
  notifyRouter,
  resetRouterForTests,
} from "../src/binding/router.ts";
import { ConsoleViewModel } from "../src/viewmodel/console-view-model.ts";
import { installFakeHost, type FakeHost } from "./fake-host.ts";

let host: FakeHost;

beforeEach(() => {
  registerElements();
  host = installFakeHost();
});

afterEach(() => {
  host.uninstall();
  resetRouterForTests();
});

function notify(channel: string, data: unknown): void {
  host.emit({ v: 1, kind: "notify", data: { channel, data } });
}

/** Mirrors `ConsoleViewModel`'s receipt route: wire → `binding.ack`. */
function wireReceipts<P>(
  viewId: string,
  ...bindings: CommandBinding<P>[]
): () => void {
  return notifyRouter().on("syn.control.receipt", (data) => {
    const d = data as {
      view_id?: string;
      target?: string;
      receipt?: CommandReceipt;
    };
    if (d.view_id !== viewId || d.receipt === undefined) return;
    for (const b of bindings) {
      if (b.target === d.target) b.ack(d.receipt);
    }
  });
}

function syncBinding<P>(target: string, viewId = "v1") {
  const port = createIpcBindingPort({ router: notifyRouter() });
  return new CommandBinding<P>({
    target,
    port,
    viewId,
    flushWith: (fn) => fn(),
    ackTimeoutMs: 0,
  });
}

describe("BindingPort dispatch", () => {
  it("maps binding commands to syn.control.dispatch without expectReceipt", () => {
    const b = syncBinding("transport");
    b.action("play", { controlId: "transport", sequence: 1 });
    const sent = host.sent.find((s) => s.op === "syn.control.dispatch");
    expect(sent).toBeDefined();
    const p = sent!.params as {
      intent: { kind: string; action?: string };
      ctx: Record<string, unknown>;
    };
    expect(p.intent.kind).toBe("action");
    expect(p.intent.action).toBe("play");
    expect(p.ctx.commandId).toMatch(/^cmd-/);
    expect(p.ctx.viewId).toBe("v1");
    expect("expectReceipt" in p.ctx).toBe(false);
  });
});

describe("subscription → projection → element", () => {
  it("routes syn.control.projection for (view,target) to observe()", async () => {
    const router = notifyRouter();
    const port = createIpcBindingPort({ router });
    const b = syncBinding<{ plain: number }>("param-gain");
    const el = document.createElement("rvf-knob") as RvfKnob;
    document.body.appendChild(el);
    const unbind = bindControlElement(el, b);

    const unsub = port.subscribe!<{ plain: number }>(
      "param-gain",
      (p) => b.observe(p),
      { viewId: "v1", generation: 1 },
    );
    expect(
      host.sent.some((s) => s.op === "syn.control.subscribe"),
    ).toBe(true);

    notify("syn.control.projection", {
      view_id: "v1",
      target: "param-gain",
      row: {
        kind: "param",
        observed: { revision: 3, value: { plain: 0.75, normalized: 0.75 } },
      },
    });
    await Promise.resolve();
    expect(el.value).toBeCloseTo(0.75);

    // Other view / other target must not reach this binding.
    notify("syn.control.projection", {
      view_id: "v2",
      target: "param-gain",
      row: { kind: "param", observed: { revision: 4, value: { plain: 0.1 } } },
    });
    notify("syn.control.projection", {
      view_id: "v1",
      target: "param-other",
      row: { kind: "param", observed: { revision: 5, value: { plain: 0.1 } } },
    });
    await Promise.resolve();
    expect(el.value).toBeCloseTo(0.75);

    unsub!();
    expect(
      host.sent.some((s) => s.op === "syn.control.unsubscribe"),
    ).toBe(true);
    unbind();
    el.remove();
  });

  it("drops a stale projection (older revision)", async () => {
    const port = createIpcBindingPort({ router: notifyRouter() });
    const b = syncBinding<{ plain: number }>("param-gain");
    const el = document.createElement("rvf-knob") as RvfKnob;
    document.body.appendChild(el);
    const unbind = bindControlElement(el, b);
    port.subscribe!<{ plain: number }>("param-gain", (p) => b.observe(p), {
      viewId: "v1",
      generation: 1,
    });

    notify("syn.control.projection", {
      view_id: "v1",
      target: "param-gain",
      row: { kind: "param", observed: { revision: 5, value: { plain: 0.6 } } },
    });
    await Promise.resolve();
    expect(el.value).toBeCloseTo(0.6);

    notify("syn.control.projection", {
      view_id: "v1",
      target: "param-gain",
      row: { kind: "param", observed: { revision: 2, value: { plain: 0.9 } } },
    });
    await Promise.resolve();
    expect(el.value).toBeCloseTo(0.6);
    expect(b.diagnostics.staleProjections).toBeGreaterThan(0);
    unbind();
    el.remove();
  });
});

describe("receipt reflection", () => {
  function ackCommand<P>(
    b: CommandBinding<P>,
    status: string,
    commandId = "cmd-1",
  ): void {
    notify("syn.control.receipt", {
      view_id: "v1",
      target: b.target,
      receipt: { commandId, status },
    });
  }

  it("applied receipt clears pending; rejected flags invalid and reverts", async () => {
    const router = notifyRouter();
    const port = createIpcBindingPort({ router });
    const b = new CommandBinding<{ plain: number }>({
      target: "param-gain",
      port,
      viewId: "v1",
      flushWith: (fn) => fn(),
      ackTimeoutMs: 0,
    });
    const el = document.createElement("rvf-knob") as RvfKnob;
    document.body.appendChild(el);
    const unbind = bindControlElement(el, b);
    wireReceipts("v1", b);

    port.subscribe!<{ plain: number }>("param-gain", (p) => b.observe(p), {
      viewId: "v1",
      generation: 1,
    });
    notify("syn.control.projection", {
      view_id: "v1",
      target: "param-gain",
      row: { kind: "param", observed: { revision: 1, value: { plain: 0.4 } } },
    });
    await Promise.resolve();
    expect(el.value).toBeCloseTo(0.4);

    // Rejected commit never poses as truth.
    el.value = 0.9;
    b.event({
      type: "begin",
      meta: { controlId: "param-gain", sequence: 1, gestureId: "g1" },
      value: { plain: 0.9, normalized: 0.9 },
    });
    b.event({
      type: "commit",
      meta: { controlId: "param-gain", sequence: 2, gestureId: "g1" },
      value: { plain: 0.9, normalized: 0.9 },
    });
    expect(el.pending).toBe(true);
    ackCommand(b, "rejected", "cmd-2");
    await Promise.resolve();
    expect(el.pending).toBe(false);
    expect(el.invalid).toBe(true);
    expect(el.value).toBeCloseTo(0.4);
    unbind();
    el.remove();
  });

  it("unsupported receipt surfaces as unsupported, not as applied", async () => {
    const router = notifyRouter();
    const port = createIpcBindingPort({ router });
    const b = new CommandBinding({
      target: "param-ro",
      port,
      viewId: "v1",
      flushWith: (fn) => fn(),
      ackTimeoutMs: 0,
    });
    const el = document.createElement("rvf-knob") as RvfKnob;
    document.body.appendChild(el);
    const unbind = bindControlElement(el, b);
    wireReceipts("v1", b);
    b.action("write", { controlId: "param-ro", sequence: 1 });
    ackCommand(b, "unsupported", "cmd-1");
    await Promise.resolve();
    expect(el.unsupported).toBe(true);
    unbind();
    el.remove();
  });

  it("transport intents dispatch through syn.control.dispatch", () => {
    const port = createIpcBindingPort({ router: notifyRouter() });
    const b = new CommandBinding({
      target: "transport",
      port,
      viewId: "v1",
      flushWith: (fn) => fn(),
      ackTimeoutMs: 0,
    });
    const el = document.createElement("rvf-transport") as RvfTransport;
    document.body.appendChild(el);
    const unbind = bindTransportElement(el, b);
    el.dispatchEvent(
      new CustomEvent("rvf-transport", {
        detail: { action: "play", controlId: "transport", sequence: 1 },
      }),
    );
    const sent = host.sent.find((s) => s.op === "syn.control.dispatch");
    expect(sent).toBeDefined();
    expect((sent!.params.intent as { action?: string }).action).toBe("play");
    unbind();
    el.remove();
  });
});

describe("lifecycle", () => {
  it("remount rebinds idempotently and unbind cleans listeners", async () => {
    const port = createIpcBindingPort({ router: notifyRouter() });
    const b = new CommandBinding({
      target: "param-gain",
      port,
      viewId: "v1",
      flushWith: (fn) => fn(),
      ackTimeoutMs: 0,
    });
    const el = document.createElement("rvf-knob") as RvfKnob;
    document.body.appendChild(el);

    const u1 = bindControlElement(el, b);
    const u2 = bindControlElement(el, b);
    expect(u2).toBe(u1); // idempotent re-bind returns the same unbind
    u1();
    const u3 = bindControlElement(el, b);
    u3();
    el.remove();
  });

  it("view model attach/detach/close call the syn.view ops", async () => {
    const router = notifyRouter();
    const port = createIpcBindingPort({ router });
    const vm = new ConsoleViewModel({
      viewId: "v-main",
      windowId: "w0",
      port,
      router,
    });
    const attachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-main",
      attached: true,
      generation: 1,
    });
    await attachP;
    expect(vm.link.attached).toBe(true);
    expect(vm.generation).toBe(1);

    const detachP = vm.detach("dock-transfer");
    host.answerOp("syn.view.detach", { view_id: "v-main", attached: false });
    await detachP;
    expect(vm.link.attached).toBe(false);

    const closeP = vm.close();
    host.answerOp("syn.view.close", { view_id: "v-main", closed: true });
    await closeP;
    const ops = host.sent.map((s) => s.op);
    expect(ops).toContain("syn.view.attach");
    expect(ops).toContain("syn.view.detach");
    expect(ops).toContain("syn.view.close");
  });

  it("link subscriptions release on detach — no leaked routes", async () => {
    const router = createRouter();
    const port = createIpcBindingPort({ router });
    const vm = new ConsoleViewModel({
      viewId: "v-leak",
      windowId: "w0",
      port,
      router,
    });
    void vm.strip("param-gain");
    const attachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-leak",
      attached: true,
      generation: 1,
    });
    await attachP;
    const subs = vm.link.linkStats.activeSubscriptions;
    expect(subs).toBeGreaterThan(0);

    vm.link.detach("unmount");
    expect(vm.link.linkStats.activeSubscriptions).toBe(0);
    expect(router.listenerCount("syn.control.projection")).toBe(0);
  });

  it("attach → detach → re-attach ties the receipt route to attach lifetime", async () => {
    const router = createRouter();
    const port = createIpcBindingPort({ router });
    const vm = new ConsoleViewModel({
      viewId: "v-life",
      windowId: "w0",
      port,
      router,
    });
    void vm.strip("param-gain");

    const attachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-life",
      attached: true,
      generation: 1,
    });
    await attachP;
    expect(vm.link.attached).toBe(true);
    expect(vm.generation).toBe(1);
    expect.soft(router.listenerCount("syn.control.receipt")).toBe(1);
    const subs = vm.link.linkStats.activeSubscriptions;
    expect(subs).toBeGreaterThan(0);
    const projRoutes = router.listenerCount("syn.control.projection");
    expect(projRoutes).toBeGreaterThan(0);

    const detachP = vm.detach("dock-transfer");
    expect.soft(router.listenerCount("syn.control.receipt")).toBe(0);
    expect.soft(router.listenerCount("syn.control.projection")).toBe(0);
    expect.soft(vm.link.linkStats.activeSubscriptions).toBe(0);
    host.answerOp("syn.view.detach", {
      view_id: "v-life",
      attached: false,
    });
    await detachP;
    expect(vm.link.attached).toBe(false);
    expect.soft(router.listenerCount("syn.control.receipt")).toBe(0);

    const reattachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-life",
      attached: true,
      generation: 2,
    });
    await reattachP;
    expect(vm.link.attached).toBe(true);
    expect(vm.generation).toBe(2);
    expect.soft(router.listenerCount("syn.control.receipt")).toBe(1);
    expect.soft(vm.link.linkStats.activeSubscriptions).toBe(subs);
    expect.soft(router.listenerCount("syn.control.projection")).toBe(
      projRoutes,
    );

    const finalP = vm.detach("unmount");
    expect.soft(router.listenerCount("syn.control.receipt")).toBe(0);
    expect.soft(router.listenerCount("syn.control.projection")).toBe(0);
    expect.soft(vm.link.linkStats.activeSubscriptions).toBe(0);
    host.answerOp("syn.view.detach", {
      view_id: "v-life",
      attached: false,
    });
    await finalP;
    expect(vm.link.attached).toBe(false);
  });

  for (const mode of ["detach", "close"] as const) {
    it(`pending attach → ${mode}: the late attach reply cannot resurrect the link`, async () => {
      const router = createRouter();
      const port = createIpcBindingPort({ router });
      const vm = new ConsoleViewModel({
        viewId: "v-pend",
        windowId: "w0",
        port,
        router,
      });
      void vm.strip("param-gain");

      const attachP = vm.attach();
      const attachId = host.sent.find(
        (s) => s.op === "syn.view.attach",
      )!.id;

      let doneP: Promise<unknown>;
      if (mode === "detach") {
        doneP = vm.detach("unmount");
        host.answerOp("syn.view.detach", {
          view_id: "v-pend",
          attached: false,
        });
      } else {
        doneP = vm.close();
        host.answerOp("syn.view.close", {
          view_id: "v-pend",
          closed: true,
        });
      }
      await doneP;

      const viewOpsBefore = host.sent.filter((s) =>
        s.op.startsWith("syn.view."),
      ).length;
      host.answer(attachId, {
        view_id: "v-pend",
        attached: true,
        generation: 1,
      });
      await attachP;
      await Promise.resolve();

      expect.soft(vm.link.attached).toBe(false);
      expect.soft(vm.link.linkStats.activeSubscriptions).toBe(0);
      expect.soft(router.listenerCount("syn.control.projection")).toBe(
        0,
      );
      expect.soft(router.listenerCount("syn.control.receipt")).toBe(0);
      expect.soft(
        host.sent.filter((s) => s.op.startsWith("syn.view.")).length,
      ).toBe(viewOpsBefore);
    });
  }
});

/**
 * Late-created binding epoch — the shared-SDK contract behind
 * Ruviform #376. `ConsoleView` builds strips via `vm.strip()` inside
 * `onMount` *after* `vm.attach()` resolves, so `createBinding` runs on
 * an already-attached `ViewLink`. Such a binding must adopt the live
 * view generation: the backend `ControlsUi` stale guard drops any
 * intent whose `ctx.generation` is behind the binding's live epoch and
 * mints no receipt, so a generation-0 dispatch would leave every
 * mounted strip permanently pending.
 */
describe("late-created bindings adopt the live view epoch", () => {
  async function attachVm(
    viewId: string,
    generation: number,
  ): Promise<ConsoleViewModel> {
    const router = notifyRouter();
    const port = createIpcBindingPort({ router });
    const vm = new ConsoleViewModel({
      viewId,
      windowId: "w0",
      port,
      router,
    });
    const p = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: viewId,
      attached: true,
      generation,
    });
    await p;
    return vm;
  }

  function lastDispatch(target: string) {
    const s = [...host.sent].reverse().find(
      (x) =>
        x.op === "syn.control.dispatch" &&
        (x.params.intent as { target?: string }).target === target,
    );
    if (s === undefined) {
      throw new Error(`no syn.control.dispatch for ${target}`);
    }
    return s.params.ctx as {
      commandId: string;
      generation: number;
      viewId?: string;
      windowId?: string;
    };
  }

  function sendReceipt(viewId: string, target: string, commandId: string) {
    notify("syn.control.receipt", {
      view_id: viewId,
      target,
      receipt: { commandId, status: "applied" },
    });
  }

  it("a strip created after attach dispatches under the live generation", async () => {
    const vm = await attachVm("v-epoch", 1);
    const strip = vm.strip("param-gain");
    expect(strip.generation).toBe(1);

    vm.transport.action("play", { controlId: "transport", sequence: 1 });
    strip.action("write", { controlId: "param-gain", sequence: 1 });
    await Promise.resolve();

    // Positive control — the pre-attach transport binding already
    // stamped the live epoch; the late strip must match it.
    expect(lastDispatch("transport").generation).toBe(1);
    const ctx = lastDispatch("param-gain");
    expect(ctx.generation).toBe(1);
    expect(ctx.viewId).toBe("v-epoch");
    expect(ctx.windowId).toBe("w0");

    sendReceipt("v-epoch", "param-gain", ctx.commandId);
    await Promise.resolve();
    expect(strip.state.lastReceipt?.commandId).toBe(ctx.commandId);
    expect(strip.state.phase).toBe("applied");
    expect(strip.diagnostics.staleReceipts).toBe(0);
  });

  it("bindings created while attached adopt the re-attach epoch", async () => {
    const vm = await attachVm("v-epoch", 1);
    const early = vm.strip("param-gain");

    const detachP = vm.detach("dock-transfer");
    host.answerOp("syn.view.detach", {
      view_id: "v-epoch",
      attached: false,
    });
    await detachP;
    const reattachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-epoch",
      attached: true,
      generation: 2,
    });
    await reattachP;

    // Created under the second epoch — must not stay at generation 0.
    const late = vm.strip("param-fader");
    early.action("write", { controlId: "param-gain", sequence: 1 });
    late.action("write", { controlId: "param-fader", sequence: 1 });
    await Promise.resolve();

    expect(lastDispatch("param-gain").generation).toBe(2);
    expect(lastDispatch("param-fader").generation).toBe(2);
    expect(early.generation).toBe(2);
    expect(late.generation).toBe(2);
  });

  it("a receipt minted under a dead generation is classified stale, not applied", async () => {
    const vm = await attachVm("v-epoch", 1);
    const strip = vm.strip("param-gain");
    strip.action("write", { controlId: "param-gain", sequence: 1 });
    await Promise.resolve();
    const deadId = lastDispatch("param-gain").commandId;

    const detachP = vm.detach("unmount");
    host.answerOp("syn.view.detach", {
      view_id: "v-epoch",
      attached: false,
    });
    await detachP;
    const reattachP = vm.attach();
    host.answerOp("syn.view.attach", {
      view_id: "v-epoch",
      attached: true,
      generation: 2,
    });
    await reattachP;

    strip.action("write", { controlId: "param-gain", sequence: 2 });
    await Promise.resolve();
    const liveId = lastDispatch("param-gain").commandId;

    // The dead-epoch receipt must never move the display.
    sendReceipt("v-epoch", "param-gain", deadId);
    await Promise.resolve();
    expect(strip.diagnostics.staleReceipts).toBeGreaterThan(0);
    expect(strip.state.lastReceipt?.commandId).not.toBe(deadId);

    // The live-epoch receipt still correlates and applies.
    sendReceipt("v-epoch", "param-gain", liveId);
    await Promise.resolve();
    expect(strip.state.lastReceipt?.commandId).toBe(liveId);
    expect(strip.state.phase).toBe("applied");
    expect(strip.diagnostics.staleReceipts).toBe(1);
  });

  for (const mode of ["detach", "close"] as const) {
    it(`after ${mode} a late-created strip drops intents without a wire dispatch`, async () => {
      const vm = await attachVm("v-epoch", 1);
      const strip = vm.strip("param-gain");
      const doneP =
        mode === "detach" ? vm.detach("unmount") : vm.close();
      host.answerOp(
        mode === "detach" ? "syn.view.detach" : "syn.view.close",
        mode === "detach"
          ? { view_id: "v-epoch", attached: false }
          : { view_id: "v-epoch", closed: true },
      );
      await doneP;

      const marker = host.sent.length;
      strip.action("write", { controlId: "param-gain", sequence: 1 });
      await Promise.resolve();
      expect(
        host.sent.slice(marker).filter(
          (s) => s.op === "syn.control.dispatch",
        ),
      ).toHaveLength(0);
      expect(strip.diagnostics.detachedIntents).toBeGreaterThan(0);
    });
  }
});
