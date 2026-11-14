/**
 * `ProbeViewModel` wire-contract tests — fake `__rvf` host plays the
 * Rust side. Assertions pin (a) exact intent params, (b) verbatim
 * wire→VM mapping (kebab-case serde vocabulary is never renamed or
 * promoted), (c) view-scoped vs session-scoped `view_id` discipline,
 * and (d) honest unavailable/empty states.
 */

import { afterEach, describe, expect, it } from "vitest";

import {
  ProbeViewModel,
  type AcousticObservationWire,
  type ProbeComparisonRequestWire,
  type ProbeStateWire,
  type ProbeSubscriptionKeyWire,
} from "../src/viewmodel/probe-view-model.ts";
import { installFakeHost, type FakeHost } from "./fake-host.ts";

let host: FakeHost | undefined;
afterEach(() => {
  host?.uninstall();
  host = undefined;
});

const POINT = {
  entity: { entity: "Track", id: "track-00000000000000000000000000000001" },
  boundary: "post-fader",
} as const;

const KEY: ProbeSubscriptionKeyWire = {
  session_id: 1,
  point: POINT,
  kind: "pcm",
};

const TARGET = "parameter-target-00000000000000000000000000000001";

function stateFixture(overrides: Partial<ProbeStateWire> = {}): ProbeStateWire {
  return {
    engine_attached: true,
    bindings: [
      {
        key: KEY,
        targets: [TARGET],
        point: POINT,
        project_revision: 7,
        active_plan_generation: 4,
        runtime_probe_id: 42,
        requested_mode: "capture",
        confirmed_mode: "monitor",
        state: "bound-active",
        last_status: {
          kind: { kind: "installed" },
          generation: 4,
          sample_position: 96000,
          clock: "ClockContextRef { domain: ClockDomainId(1), epoch: ClockEpochId(0) }",
        },
        holders: ["session", "view-3"],
        capture_owner: "view-3",
      },
    ],
    subscribers: [{ view_id: "view-3", keys: [KEY] }],
    segments: [
      {
        capture_session_id: 5,
        segment_index: 0,
        point: POINT,
        active_plan_generation: 4,
        clock_context: "ClockContextRef { domain: ClockDomainId(1), epoch: ClockEpochId(0) }",
        frame_range: { start: 0, end: 48000 },
        discontinuities: [
          { range: { start: 1000, end: 1008 }, kind: "lost-blocks" },
        ],
        completeness: "incomplete",
        ended_by: "requested",
      },
    ],
    drain: { updates: 3, uncorrelated: 1, recorded: 2, unrouted: 0 },
    traces: [
      {
        target: TARGET,
        observed: {
          state: "present",
          value: "ProbeObservation { point: SemanticProbePoint { .. }, frames: 480 }",
        },
      },
    ],
    ...overrides,
  };
}

function lastParams(op: string): Record<string, unknown> {
  const intent = [...host!.sent].reverse().find((s) => s.op === op);
  expect(intent, `intent for ${op}`).toBeDefined();
  return intent!.params;
}

function detachedState(): ProbeStateWire {
  return {
    engine_attached: false,
    bindings: [],
    subscribers: [],
    segments: [],
    drain: { updates: 0, uncorrelated: 0, recorded: 0, unrouted: 0 },
    traces: [],
  };
}

describe("syn.probe.control — session-scoped actions", () => {
  it("request/set_mode/capture/stop_capture/remove send no view_id", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });

    let p: Promise<unknown> = vm.request(TARGET, POINT, "pcm", "monitor");
    expect(lastParams("syn.probe.control")).toEqual({
      action: "request",
      target: TARGET,
      point: POINT,
      kind: "pcm",
      mode: "monitor",
    });
    host.answerOp("syn.probe.control", { key: KEY, targets: [TARGET] });
    await expect(p).resolves.toMatchObject({ key: KEY, targets: [TARGET] });

    p = vm.setMode(KEY, "capture");
    expect(lastParams("syn.probe.control")).toEqual({
      action: "set_mode",
      key: KEY,
      mode: "capture",
    });
    host.answerOp("syn.probe.control", { key: KEY, mode: "capture" });
    await p;

    p = vm.capture(KEY, 48000);
    expect(lastParams("syn.probe.control")).toEqual({
      action: "capture",
      key: KEY,
      max_frames: 48000,
    });
    host.answerOp("syn.probe.control", { key: KEY, max_frames: 48000 });
    await p;

    p = vm.stopCapture(KEY);
    expect(lastParams("syn.probe.control")).toEqual({ action: "stop_capture", key: KEY });
    host.answerOp("syn.probe.control", { key: KEY });
    await p;

    p = vm.remove(KEY);
    expect(lastParams("syn.probe.control")).toEqual({ action: "remove", key: KEY });
    host.answerOp("syn.probe.control", { key: KEY, removed: true });
    await expect(p).resolves.toMatchObject({ removed: true });
  });
});

describe("syn.probe.control — view-scoped lease actions", () => {
  it("subscribe/unsubscribe/capture_for/abort_capture always carry view_id", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });

    let p: Promise<unknown> = vm.subscribe(TARGET, POINT, "metrics", "monitor");
    expect(lastParams("syn.probe.control")).toEqual({
      action: "subscribe",
      view_id: "view-3",
      target: TARGET,
      point: POINT,
      kind: "metrics",
      mode: "monitor",
    });
    host.answerOp("syn.probe.control", {
      key: KEY,
      view_id: "view-3",
      targets: [TARGET],
    });
    await expect(p).resolves.toMatchObject({ view_id: "view-3" });

    p = vm.unsubscribe(KEY);
    expect(lastParams("syn.probe.control")).toEqual({
      action: "unsubscribe",
      view_id: "view-3",
      key: KEY,
    });
    host.answerOp("syn.probe.control", {
      key: KEY,
      view_id: "view-3",
      released: true,
      removed: false,
    });
    await expect(p).resolves.toMatchObject({ released: true, removed: false });

    p = vm.captureFor(KEY, 96000);
    expect(lastParams("syn.probe.control")).toEqual({
      action: "capture_for",
      view_id: "view-3",
      key: KEY,
      max_frames: 96000,
    });
    host.answerOp("syn.probe.control", {
      key: KEY,
      view_id: "view-3",
      max_frames: 96000,
      owner: "view-3",
    });
    await expect(p).resolves.toMatchObject({ owner: "view-3" });

    p = vm.abortCapture(KEY);
    expect(lastParams("syn.probe.control")).toEqual({
      action: "abort_capture",
      view_id: "view-3",
      key: KEY,
    });
    host.answerOp("syn.probe.control", { key: KEY, view_id: "view-3" });
    await p;
  });
});

describe("syn.probe.state", () => {
  it("stores the projection verbatim — requested vs confirmed kept distinct", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });
    const p = vm.refreshState();
    expect(host.sent.map((s) => s.op)).toEqual(["syn.probe.state"]);
    host.answerOp("syn.probe.state", stateFixture());
    const state = await p;
    expect(vm.state).toBe(state);

    const binding = state.bindings[0]!;
    // requested ≠ confirmed; `bound-active` is the only RT-confirmed state.
    expect(binding.requested_mode).toBe("capture");
    expect(binding.confirmed_mode).toBe("monitor");
    expect(binding.state).toBe("bound-active");
    expect(binding.holders).toEqual(["session", "view-3"]);
    expect(binding.capture_owner).toBe("view-3");
    expect(binding.last_status?.clock).toContain("ClockContextRef");
    expect(state.subscribers[0]!.keys[0]!.point.boundary).toBe("post-fader");
    expect(state.segments[0]!.completeness).toBe("incomplete");
    expect(state.segments[0]!.ended_by).toBe("requested");
    expect(state.traces[0]!.observed.state).toBe("present");
  });

  it("engine_attached:false yields honest empty collections, not an error", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });
    const p = vm.refreshState();
    host.answerOp("syn.probe.state", detachedState());
    const state = await p;
    expect(state.engine_attached).toBe(false);
    expect(state.bindings).toEqual([]);
    expect(state.segments).toEqual([]);
    expect(state.traces).toEqual([]);
    expect(vm.state).toBe(state);
  });
});

describe("syn.probe.compare", () => {
  const REQUEST: ProbeComparisonRequestWire = {
    before: POINT,
    after: {
      entity: { entity: "Bus", id: "bus-00000000000000000000000000000002" },
      boundary: "output",
    },
    observed_range: { start: 0, end: 48000 },
    alignment_policy: {
      require_same_generation: true,
      require_known_latency: true,
      allow_rate_conversion: false,
      max_error_frames: 64,
      allow_conditional: true,
      prefer_engine_frame: true,
    },
    metric_set: ["peak-rms", "pcm-difference"],
    max_bytes: 1 << 20,
  };

  function observation(
    quality: AcousticObservationWire["quality"],
  ): AcousticObservationWire {
    return {
      before: POINT,
      after: REQUEST.after,
      quality,
      interval: { start: 0, end: 48000 },
      gap_mask: [{ side: "before", range: { start: 0, end: 480 }, kind: "unobserved" }],
      provenance: {
        before_clock: { domain: 1, epoch: 0 },
        after_clock: { domain: 1, epoch: 0 },
        before_generations: { first: 4, last: 4 },
        after_generations: { first: 4, last: 4 },
        runtime_instance: 1,
        sample_rate: 48000,
      },
    };
  }

  it("forwards params.request verbatim and keeps quality.kind unpromoted", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });

    let p = vm.compare(REQUEST);
    expect(lastParams("syn.probe.compare")).toEqual({ request: REQUEST });
    const incomparable = observation({
      kind: "incomparable",
      reason: { kind: "domain-mismatch" },
    });
    host.answerOp("syn.probe.compare", { engine_attached: true, observation: incomparable });
    await expect(p).resolves.toEqual({ engine_attached: true, observation: incomparable });
    // `incomparable` is never promoted to `aligned`.
    expect(vm.comparison).toEqual({ engine_attached: true, observation: incomparable });
    expect(vm.comparison?.engine_attached && vm.comparison.observation?.quality.kind).toBe(
      "incomparable",
    );

    p = vm.compare(REQUEST);
    const conditional = observation({
      kind: "conditional",
      basis: { kind: "shared-cursor", clock: { domain: 1, epoch: 0 } },
      offset_frames: 480,
      error_budget_frames: 1024,
      conditions: [{ kind: "unknown-latency", bound_frames: 2048 }],
      excluded_intervals: [{ start: 1000, end: 1008 }],
    });
    host.answerOp("syn.probe.compare", { engine_attached: true, observation: conditional });
    await expect(p).resolves.toMatchObject({
      observation: { quality: { kind: "conditional", offset_frames: 480 } },
    });

    p = vm.compare(REQUEST);
    const aligned = observation({
      kind: "aligned",
      basis: { kind: "engine-frame", epoch: 0 },
      offset_frames: 0,
      error_budget_frames: 64,
    });
    host.answerOp("syn.probe.compare", { engine_attached: true, observation: aligned });
    await expect(p).resolves.toMatchObject({
      observation: { quality: { kind: "aligned" } },
    });
  });

  it("detached engine → engine_attached:false + observation:null (honest unavailable)", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });
    const p = vm.compare(REQUEST);
    expect(lastParams("syn.probe.compare")).toEqual({ request: REQUEST });
    host.answerOp("syn.probe.compare", { engine_attached: false, observation: null });
    const res = await p;
    expect(res).toEqual({ engine_attached: false, observation: null });
    expect(vm.comparison).toEqual({ engine_attached: false, observation: null });
  });
});

describe("error paths", () => {
  it("propagates wire errors verbatim (no-engine control request)", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "view-3" });
    const p = vm.request(TARGET, POINT, "pcm", "monitor");
    host.answerOpErr("syn.probe.control", "internal", "no routing engine attached");
    await expect(p).rejects.toMatchObject({
      code: "internal",
      message: "no routing engine attached",
    });
  });

  it("propagates malformed-view_id rejections from view-scoped actions", async () => {
    host = installFakeHost();
    const vm = new ProbeViewModel({ viewId: "bogus" });
    const p = vm.subscribe(TARGET, POINT, "pcm", "monitor");
    expect(lastParams("syn.probe.control")).toMatchObject({ view_id: "bogus" });
    host.answerOpErr(
      "syn.probe.control",
      "invalid_params",
      'params.view_id must be "view-{n}", got bogus',
    );
    await expect(p).rejects.toMatchObject({ code: "invalid_params" });
  });
});

describe("no fabricated state", () => {
  it("starts empty — no bindings/subscribers claimed before the first poll", () => {
    const vm = new ProbeViewModel({ viewId: "view-3" });
    expect(vm.state).toBeUndefined();
    expect(vm.comparison).toBeUndefined();
  });
});
