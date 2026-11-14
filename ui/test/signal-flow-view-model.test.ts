/**
 * `SignalFlowViewModel` wire-contract tests — the fake `__rvf` host
 * plays the Rust side; assertions pin the exact intent params and the
 * verbatim wire→VM mapping (Debug vocabulary is never renamed, folded,
 * or promoted).
 */

import { afterEach, describe, expect, it } from "vitest";

import {
  SignalFlowViewModel,
  type SignalFlowViewWire,
} from "../src/viewmodel/signal-flow-view-model.ts";
import { installFakeHost, type FakeHost } from "./fake-host.ts";

let host: FakeHost | undefined;
afterEach(() => {
  host?.uninstall();
  host = undefined;
});

const TRACK = { kind: "track", id: "track-00000000000000000000000000000001" } as const;
const BUS = { kind: "bus", id: "bus-00000000000000000000000000000002" } as const;
const PORT_A = "semantic-port-0000000000000000000000000000000a";
const PORT_B = "semantic-port-0000000000000000000000000000000b";

function viewFixture(overrides: Partial<SignalFlowViewWire> = {}): SignalFlowViewWire {
  return {
    project_id: "project-00000000000000000000000000000001",
    project_revision: 7,
    view_generation: 3,
    coverage: "Complete",
    scope: "Overview",
    nodes: [
      {
        id: "Strip(StripId(1))",
        kind: "TrackStrip { track: TrackId(1), name: \"Kick\" }",
        label: "Kick",
        state: "Ready",
        runtime: "Resolved { generation: 4 }",
        probe: null,
        muted: false,
        soloed: false,
        processor_count: 2,
      },
    ],
    edges: [
      {
        id: "Connection(RoutingConnectionId(9))",
        role: "Main",
        source: { node: "Strip(StripId(1))", port: PORT_A, port_role: "Main" },
        target: { node: "Strip(StripId(2))", port: PORT_B, port_role: "Main" },
        enabled: true,
        tap: null,
      },
      {
        id: "Connection(RoutingConnectionId(10))",
        role: "Send",
        source: { node: "Strip(StripId(1))", port: PORT_A, port_role: "Main" },
        target: { node: "Strip(StripId(3))", port: PORT_B, port_role: "Main" },
        enabled: true,
        tap: "PreFader",
      },
    ],
    status: {
      project_revision: 7,
      engine_attached: true,
      activation: "Published { project_revision: ProjectRevision(7), generation: 4 }",
      planned_generation: 4,
      published_generation: 4,
      observed_active_generation: null,
      last_good_generation: 3,
      fault: null,
      source_map_revision: 7,
      source_map_generation: 4,
      stale: false,
      probes: [
        {
          point: "SemanticProbePoint { entity: Track(TrackId(1)), boundary: PostFader, port: None }",
          state: "BoundActive",
          requested_mode: "Monitor",
          confirmed_mode: null,
          generation: 4,
          project_revision: 7,
        },
      ],
    },
    ...overrides,
  };
}

function lastParams(op: string): Record<string, unknown> {
  const intent = [...host!.sent].reverse().find((s) => s.op === op);
  expect(intent, `intent for ${op}`).toBeDefined();
  return intent!.params;
}

describe("syn.signal_flow.view", () => {
  it("sends exact params and stores the projection verbatim", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "view-3" });
    const pending = vm.loadView({
      scope: "local",
      root: TRACK,
      roles: ["main", "send"],
      collapsed: [BUS],
      max_nodes: 32,
      max_edges: 64,
      expected_revision: 7,
    });
    expect(lastParams("syn.signal_flow.view")).toEqual({
      view_id: "view-3",
      scope: "local",
      root: TRACK,
      roles: ["main", "send"],
      collapsed: [BUS],
      max_nodes: 32,
      max_edges: 64,
      expected_revision: 7,
    });
    const stale = viewFixture({
      coverage: "Stale",
      view_generation: 5,
      status: {
        ...viewFixture().status,
        published_generation: 4,
        observed_active_generation: 4,
      },
    });
    host.answerOp("syn.signal_flow.view", stale);
    const view = await pending;
    expect(vm.projection).toBe(view);
    // Debug vocabulary + coverage kept verbatim — never folded.
    expect(view.coverage).toBe("Stale");
    expect(view.scope).toBe("Overview");
    expect(view.nodes[0]!.state).toBe("Ready");
    expect(view.nodes[0]!.runtime).toBe("Resolved { generation: 4 }");
    expect(view.edges[1]!.tap).toBe("PreFader");
    // Publish ≠ Active: both generation fields survive separately.
    expect(view.status.published_generation).toBe(4);
    expect(view.status.observed_active_generation).toBe(4);
    expect(view.status.activation).toContain("Published");
    // Probe 未取得 is `confirmed_mode: null`, not a zeroed mode.
    expect(view.status.probes[0]!.confirmed_mode).toBeNull();
    expect(view.status.probes[0]!.requested_mode).toBe("Monitor");
    expect(vm.viewGeneration).toBe(5);
  });

  it("omits view_id when unconfigured (service main-view default)", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel();
    const pending = vm.loadView();
    expect(lastParams("syn.signal_flow.view")).toEqual({});
    host.answerOp("syn.signal_flow.view", viewFixture());
    await pending;
    expect(vm.projection?.coverage).toBe("Complete");
  });
});

describe("syn.signal_flow.visual", () => {
  it("move_node/set_group/fold/unfold/zoom/pan/link_focus/set_scope send exact params and track view_generation", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "view-2" });

    let p = vm.moveNode(TRACK, 10.5, -3.25);
    expect(lastParams("syn.signal_flow.visual")).toEqual({
      view_id: "view-2",
      action: "move_node",
      entity: TRACK,
      x: 10.5,
      y: -3.25,
    });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 4 });
    await expect(p).resolves.toMatchObject({ view_generation: 4 });
    expect(vm.viewGeneration).toBe(4);

    p = vm.setGroup({ group_id: 1, label: "Drums", members: [TRACK, BUS], collapsed: true });
    expect(lastParams("syn.signal_flow.visual")).toEqual({
      view_id: "view-2",
      action: "set_group",
      group_id: 1,
      label: "Drums",
      members: [TRACK, BUS],
      collapsed: true,
    });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 5 });
    await p;

    p = vm.fold(TRACK);
    expect(lastParams("syn.signal_flow.visual")).toMatchObject({ action: "fold", entity: TRACK });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 6 });
    await p;

    p = vm.unfold(TRACK);
    expect(lastParams("syn.signal_flow.visual")).toMatchObject({ action: "unfold", entity: TRACK });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 7 });
    await p;

    p = vm.zoom(1.5);
    expect(lastParams("syn.signal_flow.visual")).toMatchObject({ action: "zoom", zoom: 1.5 });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 8 });
    await p;

    p = vm.pan(12, -4);
    expect(lastParams("syn.signal_flow.visual")).toMatchObject({ action: "pan", x: 12, y: -4 });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 9 });
    await p;

    p = vm.linkFocus(null);
    expect(lastParams("syn.signal_flow.visual")).toMatchObject({ action: "link_focus", entity: null });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 10 });
    await p;

    p = vm.setScope("local", TRACK);
    expect(lastParams("syn.signal_flow.visual")).toEqual({
      view_id: "view-2",
      action: "set_scope",
      scope: "local",
      root: TRACK,
    });
    host.answerOp("syn.signal_flow.visual", { ok: true, view: "view-2", view_generation: 11 });
    await p;
    expect(vm.viewGeneration).toBe(11);
  });
});

describe("syn.signal_flow.inspection", () => {
  it("sends exact params per action and stores the returned focus", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "view-4" });
    const inspection = {
      target: TRACK,
      mode: "explore",
      follow_source: { group: 2 },
      explore_back: 1,
      explore_fwd: 0,
    };

    let p = vm.inspect(TRACK);
    expect(lastParams("syn.signal_flow.inspection")).toEqual({
      view_id: "view-4",
      action: "inspect",
      entity: TRACK,
    });
    host.answerOp("syn.signal_flow.inspection", { ok: true, view: "view-4", inspection });
    await expect(p).resolves.toMatchObject({ inspection });
    expect(vm.inspection).toEqual(inspection);

    p = vm.setInspectionMode("pinned");
    expect(lastParams("syn.signal_flow.inspection")).toMatchObject({ action: "set_mode", mode: "pinned" });
    host.answerOp("syn.signal_flow.inspection", {
      ok: true,
      view: "view-4",
      inspection: { ...inspection, mode: "pinned" },
    });
    await p;
    expect(vm.inspection?.mode).toBe("pinned");

    p = vm.setFollowSource({ view: "view-7" });
    expect(lastParams("syn.signal_flow.inspection")).toMatchObject({
      action: "set_follow_source",
      follow_source: { view: "view-7" },
    });
    host.answerOp("syn.signal_flow.inspection", { ok: true, view: "view-4", inspection });
    await p;

    p = vm.inspectionBack();
    expect(lastParams("syn.signal_flow.inspection")).toMatchObject({ action: "back" });
    host.answerOp("syn.signal_flow.inspection", { ok: true, view: "view-4", inspection });
    await p;

    p = vm.inspectionForward();
    expect(lastParams("syn.signal_flow.inspection")).toMatchObject({ action: "forward" });
    host.answerOp("syn.signal_flow.inspection", { ok: true, view: "view-4", inspection });
    await p;
  });
});

describe("syn.signal_flow.routing", () => {
  const fixation = {
    view: "view-4",
    source: PORT_A,
    revision: 7,
    capability_generation: 2,
    window: "w1",
  };

  it("begin/hover/drop/confirm/cancel/state send exact params; outcomes stay typed", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "view-4" });

    let p: Promise<unknown> = vm.routingBegin(PORT_A);
    expect(lastParams("syn.signal_flow.routing")).toEqual({
      view_id: "view-4",
      action: "begin",
      port: PORT_A,
    });
    host.answerOp("syn.signal_flow.routing", { ok: true, view: "view-4", fixation });
    await expect(p).resolves.toMatchObject({ fixation });

    p = vm.routingHover(PORT_B);
    expect(lastParams("syn.signal_flow.routing")).toMatchObject({ action: "hover", port: PORT_B });
    host.answerOp("syn.signal_flow.routing", {
      ok: true,
      view: "view-4",
      assessment: { verdict: "needs_mapping", source_channels: 2, destination_channels: 1 },
    });
    await expect(p).resolves.toMatchObject({
      assessment: { verdict: "needs_mapping", source_channels: 2, destination_channels: 1 },
    });

    p = vm.routingHover(null);
    expect(lastParams("syn.signal_flow.routing")).toMatchObject({ action: "hover", port: null });
    host.answerOp("syn.signal_flow.routing", { ok: true, view: "view-4", assessment: null });
    await p;

    p = vm.routingDrop();
    expect(lastParams("syn.signal_flow.routing")).toMatchObject({ action: "drop" });
    host.answerOp("syn.signal_flow.routing", { ok: true, view: "view-4", dropped: PORT_B });
    await expect(p).resolves.toMatchObject({ dropped: PORT_B });

    p = vm.routingConfirm({ entries: [0, 1] });
    expect(lastParams("syn.signal_flow.routing")).toEqual({
      view_id: "view-4",
      action: "confirm",
      channel_map: { entries: [0, 1] },
    });
    host.answerOp("syn.signal_flow.routing", {
      ok: true,
      view: "view-4",
      outcome: { outcome: "applied", revision: 8 },
    });
    await expect(p).resolves.toMatchObject({ outcome: { outcome: "applied", revision: 8 } });

    p = vm.routingCancel("pointer-cancel");
    expect(lastParams("syn.signal_flow.routing")).toMatchObject({
      action: "cancel",
      reason: "pointer-cancel",
    });
    host.answerOp("syn.signal_flow.routing", {
      ok: true,
      view: "view-4",
      outcome: { outcome: "cancelled", reason: "pointer-cancel" },
    });
    await expect(p).resolves.toMatchObject({
      outcome: { outcome: "cancelled", reason: "pointer-cancel" },
    });

    // `stale` is surfaced verbatim — the VM never retargets.
    p = vm.routingConfirm();
    expect(lastParams("syn.signal_flow.routing")).toEqual({
      view_id: "view-4",
      action: "confirm",
    });
    host.answerOp("syn.signal_flow.routing", {
      ok: true,
      view: "view-4",
      outcome: { outcome: "stale" },
    });
    await expect(p).resolves.toMatchObject({ outcome: { outcome: "stale" } });

    p = vm.routingState();
    expect(lastParams("syn.signal_flow.routing")).toMatchObject({ action: "state" });
    host.answerOp("syn.signal_flow.routing", {
      ok: true,
      view: "view-4",
      gesture: {
        fixation,
        preview: { candidate: PORT_B, assessment: null, dropped_target: PORT_B },
      },
    });
    await expect(p).resolves.toMatchObject({
      gesture: { fixation: { source: PORT_A, revision: 7 } },
    });
    expect(vm.gesture?.fixation.source).toBe(PORT_A);

    p = vm.routingState();
    host.answerOp("syn.signal_flow.routing", { ok: true, view: "view-4", gesture: null });
    await expect(p).resolves.toMatchObject({ gesture: null });
    expect(vm.gesture).toBeNull();
  });
});

describe("error paths", () => {
  it("propagates wire errors with their code", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "view-1" });
    const p = vm.moveNode(TRACK, 0, 0);
    host.answerOpErr("syn.signal_flow.visual", "invalid_params", "params.entity: malformed tagged id");
    await expect(p).rejects.toMatchObject({
      code: "invalid_params",
      message: "params.entity: malformed tagged id",
    });
    // A failed ack must not advance the staleness token.
    expect(vm.viewGeneration).toBe(0);
  });

  it("passes a malformed view_id through verbatim so the wire rejects it", async () => {
    host = installFakeHost();
    const vm = new SignalFlowViewModel({ viewId: "not-a-view" });
    const p = vm.setInspectionMode("follow");
    expect(lastParams("syn.signal_flow.inspection")).toMatchObject({
      view_id: "not-a-view",
      action: "set_mode",
    });
    host.answerOpErr(
      "syn.signal_flow.inspection",
      "invalid_params",
      'params.view_id must be "view-{n}", got not-a-view',
    );
    await expect(p).rejects.toMatchObject({ code: "invalid_params" });
    expect(vm.inspection).toBeUndefined();
  });
});

describe("no fabricated state", () => {
  it("starts empty — nothing is claimed before the first response", () => {
    const vm = new SignalFlowViewModel({ viewId: "view-1" });
    expect(vm.projection).toBeUndefined();
    expect(vm.inspection).toBeUndefined();
    expect(vm.gesture).toBeUndefined();
    expect(vm.viewGeneration).toBe(0);
  });
});
