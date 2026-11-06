import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { render } from "solid-js/web";

import type { RvfKnob } from "@ruviform/ui-components";
import type { RvfWaveform } from "@ruviform/ui-viz";

import { registerElements } from "../src/adapters/ruviform.ts";
import {
  notifyRouter,
  resetRouterForTests,
} from "../src/binding/router.ts";
import { ConsoleView } from "../src/views/console.tsx";
import {
  installFakeHost,
  type FakeHost,
  type SentIntent,
} from "./fake-host.ts";

const CHANNELS = [
  "syn.control.receipt",
  "syn.control.projection",
  "syn.viz.event",
  "syn.viz.frame",
] as const;

let host: FakeHost;
let cleanups: Array<() => void> = [];

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
}

function mountView(viewId: string, windowId: string) {
  const root = document.createElement("div");
  document.body.appendChild(root);
  const dispose = render(
    () => <ConsoleView viewId={viewId} windowId={windowId} />,
    root,
  );
  let done = false;
  cleanups.push(() => {
    if (done) return;
    done = true;
    dispose();
    root.remove();
  });
  return {
    root,
    dispose() {
      if (done) return;
      done = true;
      dispose();
      root.remove();
    },
  };
}

function firstIntentId(op: string): string {
  const s = host.sent.find((x) => x.op === op);
  if (s === undefined) throw new Error(`no intent sent for ${op}`);
  return s.id;
}

function lastIntentId(op: string): string {
  const s = [...host.sent].reverse().find((x) => x.op === op);
  if (s === undefined) throw new Error(`no intent sent for ${op}`);
  return s.id;
}

function bindIntentId(surfaceId: string): string {
  const s = host.sent.find(
    (x) => x.op === "syn.viz.bind" && x.params.surface_id === surfaceId,
  );
  if (s === undefined) throw new Error(`no syn.viz.bind for ${surfaceId}`);
  return s.id;
}

function opCount(op: string): number {
  return host.sent.filter((s) => s.op === op).length;
}

function opsSince(index: number): string[] {
  return host.sent.slice(index).map((s) => s.op);
}

function answerAttach(viewId: string, generation: number, id?: string): void {
  host.answer(id ?? lastIntentId("syn.view.attach"), {
    view_id: viewId,
    attached: true,
    generation,
  });
}

async function driveToFullyMounted(
  viewId: string,
  generation: number,
): Promise<void> {
  answerAttach(viewId, generation);
  await flush();
  host.answer(bindIntentId("wave"), {});
  await flush();
  host.answer(bindIntentId("meter"), {});
  await flush();
}

function dispatchViewIntents(el: RvfWaveform): void {
  el.dispatchEvent(
    new CustomEvent("rvf-viewport", {
      detail: { viewport: { start: 0, end: 1 } },
    }),
  );
  el.dispatchEvent(
    new CustomEvent("rvf-select", { detail: { start: 0, end: 1 } }),
  );
}

function expectChannelsZero(where: string): void {
  const router = notifyRouter();
  for (const c of CHANNELS) {
    expect.soft(router.listenerCount(c), `${where}: ${c}`).toBe(0);
  }
}

beforeEach(() => {
  registerElements();
  host = installFakeHost();
  cleanups = [];
});

afterEach(() => {
  for (const c of cleanups.splice(0)) c();
  host.uninstall();
  resetRouterForTests();
});

describe("ConsoleView unmount lifecycle", () => {
  it("mounted view → dispose releases every router channel and posts detach, not close", async () => {
    const { root, dispose } = mountView("v-main", "w0");
    await driveToFullyMounted("v-main", 1);

    const waveEl = root.querySelector("rvf-waveform") as RvfWaveform;
    const router = notifyRouter();
    expect(router.listenerCount("syn.control.receipt")).toBe(1);
    expect(
      router.listenerCount("syn.control.projection"),
    ).toBeGreaterThan(0);
    expect(router.listenerCount("syn.viz.event")).toBe(1);
    expect(router.listenerCount("syn.viz.frame")).toBe(1);

    const liveMarker = host.sent.length;
    dispatchViewIntents(waveEl);
    await flush();
    const live = host.sent.slice(liveMarker);
    const liveVp = live.filter((s) => s.op === "syn.viz.viewport");
    const liveSel = live.filter((s) => s.op === "syn.viz.select");
    expect(liveVp).toHaveLength(1);
    expect(liveVp[0]!.params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });
    expect(liveSel).toHaveLength(1);
    expect(liveSel[0]!.params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });

    dispose();

    expectChannelsZero("after dispose");
    const ops = host.sent.map((s) => s.op);
    expect.soft(ops).toContain("syn.view.detach");
    expect.soft(ops).not.toContain("syn.view.close");

    const marker = host.sent.length;
    dispatchViewIntents(waveEl);
    await flush();
    const late = opsSince(marker);
    expect.soft(late).not.toContain("syn.viz.viewport");
    expect.soft(late).not.toContain("syn.viz.select");
  });

  it("dispose while attach is pending releases routes; the late attach reply stays dead", async () => {
    const { dispose } = mountView("v-main", "w0");
    const attachId = firstIntentId("syn.view.attach");

    dispose();
    expectChannelsZero("after dispose during pending attach");

    const marker = host.sent.length;
    answerAttach("v-main", 1, attachId);
    await flush();

    expectChannelsZero("after late attach reply");
    const late = opsSince(marker);
    expect.soft(late).not.toContain("syn.control.subscribe");
    expect.soft(late).not.toContain("syn.viz.bind");
    expect.soft(late).not.toContain("syn.view.detach");
    expect.soft(late).not.toContain("syn.view.close");
  });

  it("dispose during the wave viz.bind gate; the late bind reply issues no meter bind", async () => {
    const { root, dispose } = mountView("v-main", "w0");
    const waveEl = root.querySelector("rvf-waveform") as RvfWaveform;
    answerAttach("v-main", 1);
    await flush();
    const waveBindId = bindIntentId("wave");

    dispose();
    expectChannelsZero("after dispose during wave bind");

    const bindsBefore = opCount("syn.viz.bind");
    host.answer(waveBindId, {});
    await flush();

    expect.soft(opCount("syn.viz.bind")).toBe(bindsBefore);
    expectChannelsZero("after late wave bind reply");

    const marker = host.sent.length;
    dispatchViewIntents(waveEl);
    await flush();
    const late = opsSince(marker);
    expect.soft(late).not.toContain("syn.viz.viewport");
    expect.soft(late).not.toContain("syn.viz.select");
  });

  it("dispose during the meter viz.bind gate; the late reply installs no listeners or refresh", async () => {
    const { root, dispose } = mountView("v-main", "w0");
    const waveEl = root.querySelector("rvf-waveform") as RvfWaveform;
    answerAttach("v-main", 1);
    await flush();
    host.answer(bindIntentId("wave"), {});
    await flush();
    const meterBindId = bindIntentId("meter");

    dispose();
    expectChannelsZero("after dispose during meter bind");

    const snapshot = {
      bind: opCount("syn.viz.bind"),
      meta: opCount("syn.viz.meta"),
      snap: opCount("syn.viz.snapshot"),
      viewport: opCount("syn.viz.viewport"),
      select: opCount("syn.viz.select"),
    };
    host.answer(meterBindId, {});
    await flush();

    expect.soft(opCount("syn.viz.bind")).toBe(snapshot.bind);
    expect.soft(opCount("syn.viz.meta")).toBe(snapshot.meta);
    expect.soft(opCount("syn.viz.snapshot")).toBe(snapshot.snap);
    expectChannelsZero("after late meter bind reply");

    dispatchViewIntents(waveEl);
    await flush();
    expect.soft(opCount("syn.viz.viewport")).toBe(snapshot.viewport);
    expect.soft(opCount("syn.viz.select")).toBe(snapshot.select);
  });

  it("a stale attach reply for a replaced mount cannot resurrect or detach the live view", async () => {
    const oldMount = mountView("v-main", "w0");
    const oldAttachId = firstIntentId("syn.view.attach");
    oldMount.dispose();

    const repl = mountView("v-main", "w1");
    await driveToFullyMounted("v-main", 2);

    const router = notifyRouter();
    const newWaveEl = repl.root.querySelector(
      "rvf-waveform",
    ) as RvfWaveform;

    const marker0 = host.sent.length;
    dispatchViewIntents(newWaveEl);
    await flush();
    const live = host.sent.slice(marker0);
    const vp = live.find((s) => s.op === "syn.viz.viewport");
    const sel = live.find((s) => s.op === "syn.viz.select");
    expect(vp).toBeDefined();
    expect((vp as SentIntent).params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });
    expect(sel).toBeDefined();
    expect((sel as SentIntent).params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });

    expect.soft(router.listenerCount("syn.control.receipt")).toBe(1);
    expect.soft(router.listenerCount("syn.viz.event")).toBe(1);
    expect.soft(router.listenerCount("syn.viz.frame")).toBe(1);
    const projectionRoutes = router.listenerCount(
      "syn.control.projection",
    );
    const viewOpsBefore = host.sent.filter(
      (s) => s.op === "syn.view.detach" || s.op === "syn.view.close",
    ).length;
    const bindsBefore = opCount("syn.viz.bind");
    const subsBefore = opCount("syn.control.subscribe");

    host.answer(oldAttachId, {
      view_id: "v-main",
      attached: true,
      generation: 1,
    });
    await flush();

    expect.soft(router.listenerCount("syn.control.receipt")).toBe(1);
    expect.soft(router.listenerCount("syn.control.projection")).toBe(
      projectionRoutes,
    );
    expect.soft(router.listenerCount("syn.viz.event")).toBe(1);
    expect.soft(router.listenerCount("syn.viz.frame")).toBe(1);
    expect.soft(opCount("syn.viz.bind")).toBe(bindsBefore);
    expect.soft(opCount("syn.control.subscribe")).toBe(subsBefore);
    expect.soft(
      host.sent.filter(
        (s) => s.op === "syn.view.detach" || s.op === "syn.view.close",
      ).length,
    ).toBe(viewOpsBefore);

    const marker1 = host.sent.length;
    dispatchViewIntents(newWaveEl);
    await flush();
    const after = host.sent.slice(marker1);
    const vpAfter = after.find((s) => s.op === "syn.viz.viewport");
    const selAfter = after.find((s) => s.op === "syn.viz.select");
    expect(vpAfter).toBeDefined();
    expect((vpAfter as SentIntent).params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });
    expect(selAfter).toBeDefined();
    expect((selAfter as SentIntent).params).toMatchObject({
      view_id: "v-main",
      surface_id: "wave",
    });

    repl.dispose();
    expectChannelsZero("after replacement dispose");
  });
});

describe("ConsoleView mounted control-op", () => {
  it("a strip created post-attach dispatches under the live generation; its receipt applies", async () => {
    const { root } = mountView("v-main", "w0");
    await driveToFullyMounted("v-main", 1);
    const knobEl = root.querySelector("rvf-knob") as RvfKnob;

    // The strip is created inside onMount after attach resolved —
    // the RF #376 path. Its commit must stamp the live epoch (1),
    // or the backend stale guard drops it and no receipt ever lands.
    const marker = host.sent.length;
    knobEl.dispatchEvent(
      new CustomEvent("rvf-gesture", {
        detail: {
          type: "begin",
          meta: { controlId: "param-gain", sequence: 1, gestureId: "g1" },
          value: { plain: 0.6, normalized: 0.6 },
        },
      }),
    );
    knobEl.dispatchEvent(
      new CustomEvent("rvf-gesture", {
        detail: {
          type: "commit",
          meta: { controlId: "param-gain", sequence: 2, gestureId: "g1" },
          value: { plain: 0.6, normalized: 0.6 },
        },
      }),
    );
    await flush();

    const sent = host.sent
      .slice(marker)
      .filter((s) => s.op === "syn.control.dispatch");
    const commit = sent.find(
      (s) => (s.params.intent as { kind?: string }).kind === "commit",
    );
    expect(commit).toBeDefined();
    const ctx = (commit as SentIntent).params.ctx as {
      commandId?: string;
      generation?: number;
    };
    expect(ctx.generation).toBe(1);
    expect(knobEl.pending).toBe(true);

    // The applied receipt correlates under the live epoch — never
    // classified stale. `applied` is still not display truth, so the
    // pending flag only clears when the projection arrives.
    host.emit({
      v: 1,
      kind: "notify",
      data: {
        channel: "syn.control.receipt",
        data: {
          view_id: "v-main",
          target: "param-gain",
          receipt: { commandId: ctx.commandId, status: "applied" },
        },
      },
    });
    await flush();
    expect(knobEl.invalid).toBe(false);
    expect(knobEl.unsupported).toBe(false);

    host.emit({
      v: 1,
      kind: "notify",
      data: {
        channel: "syn.control.projection",
        data: {
          view_id: "v-main",
          target: "param-gain",
          row: {
            kind: "param",
            observed: {
              revision: 1,
              value: { plain: 0.6, normalized: 0.6 },
            },
          },
        },
      },
    });
    await flush();
    expect(knobEl.pending).toBe(false);
    expect(knobEl.value).toBeCloseTo(0.6);
  });
});
