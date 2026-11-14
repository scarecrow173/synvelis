/**
 * `SignalFlowViewModel` — DOM-free consumer of the `syn.signal_flow.*`
 * ops (SF-05 ui slice, synvelis#108).
 *
 * The service side (`crates/synvelis-app/src/ui_ops.rs`) owns every
 * invariant — visual state never reaches DSP/project/undo, inspection
 * focus never touches editing selection, routing gestures fix their
 * revision/capability/window epoch at begin and drop `stale` instead
 * of retargeting. This view model is the typed carrier between the
 * wire contract and a future SolidJS surface: it holds the last
 * `syn.signal_flow.view` projection verbatim and forwards each op's
 * params unchanged.
 *
 * Wire-vocabulary rules (do not "fix" these):
 *
 * - `syn.signal_flow.view` renders enums with `Debug` (`"Complete"`,
 *   `"BoundActive"`, `"PreFader"`, …) — the `*Wire` types below keep
 *   that spelling. Request-side enums are different vocabularies
 *   (`scope: "overview"`, `roles: ["main"]`, inspection `mode:
 *   "follow"`) and are typed separately.
 * - `coverage`/`status` are kept verbatim: `Stale` is not folded into
 *   "no routing", `published_generation` and
 *   `observed_active_generation` stay distinct fields (publish ≠
 *   active), and an empty `status.probes` means "no probe was ever
 *   requested" — never a zero observation.
 * - `view_generation` (visual `visual_revision`) is the staleness
 *   token: visual acks and view responses echo the same counter.
 */

import { call } from "../ipc.ts";

// -- wire vocabulary (Debug strings, verbatim) -------------------------

/** `SignalFlowCoverage` — `format!("{:?}")` on the Rust enum. */
export type SignalFlowCoverageWire =
  | "Complete"
  | "Truncated"
  | "Stale"
  | "Unavailable"
  | "Unsupported";

/** `SignalFlowEdgeRole` Debug vocabulary (response side). */
export type SignalFlowEdgeRoleWire = "Main" | "Sidechain" | "Send" | "Chain";

/** `SignalFlowNodeState` Debug vocabulary. */
export type SignalFlowNodeStateWire = "Ready" | "Unresolved";

/** `PortRole` Debug vocabulary on edge endpoints. */
export type SignalFlowPortRoleWire = "Main" | "Sidechain";

/** `SendTap` Debug vocabulary (`Some` only on `role = "Send"`). */
export type SignalFlowSendTapWire = "PreFader" | "PostFader";

/** `ProbeMode` Debug vocabulary inside the view status block. */
export type SignalFlowProbeModeWire = "Off" | "Monitor" | "Capture";

/** `ProbeResolveState` Debug vocabulary inside the view status block. */
export type SignalFlowProbeStateWire =
  | "Requested"
  | "Unresolved"
  | "Unavailable"
  | "Unsupported"
  | "QueuedForBinding"
  | "PendingRtInstall"
  | "BoundActive"
  | "StaleGeneration"
  | "PartiallyRebound"
  | "Lost"
  | "Stopped";

// -- request vocabulary -------------------------------------------------

/** `params.entity`/`root`/`members`/`collapsed` wire object —
 * `{"kind": ..., "id": "<kind>-<32hex>"}` (`sf_entity`). */
export type SignalFlowEntityKind =
  | "track"
  | "bus"
  | "strip"
  | "processor"
  | "port"
  | "connection"
  | "send";

export interface SignalFlowEntity {
  kind: SignalFlowEntityKind;
  /** Tagged id, e.g. `"track-0000…0001"`. */
  id: string;
}

/** `params.scope` request vocabulary (`sf_scope`) — kebab strings,
 * deliberately distinct from the Debug `scope` string in the view
 * response (`"Overview"`, `Local { root: … }`). */
export type SignalFlowScopeParam = "overview" | "expanded" | "local" | "history";

/** `params.roles` entries (`sf_roles`). */
export type SignalFlowRoleFilter = "main" | "sidechain" | "send" | "chain";

// -- syn.signal_flow.view response --------------------------------------

export interface SignalFlowViewNodeWire {
  /** `SignalFlowNodeId` Debug text. */
  id: string;
  /** `SignalFlowNodeKind` Debug text. */
  kind: string;
  label: string;
  state: SignalFlowNodeStateWire;
  /** `SignalFlowNodeRuntime` Debug text. */
  runtime: string;
  /** Latest probe lifecycle state — `null` = never requested. */
  probe: SignalFlowProbeStateWire | null;
  muted: boolean;
  soloed: boolean;
  processor_count: number;
}

export interface SignalFlowViewEdgeEndpointWire {
  /** `SignalFlowNodeId` Debug text. */
  node: string;
  /** Tagged `SemanticPortId`, `null` for synthesized chain edges. */
  port: string | null;
  port_role: SignalFlowPortRoleWire;
}

export interface SignalFlowViewEdgeWire {
  /** `SignalFlowEdgeId` Debug text. */
  id: string;
  role: SignalFlowEdgeRoleWire;
  source: SignalFlowViewEdgeEndpointWire;
  target: SignalFlowViewEdgeEndpointWire;
  enabled: boolean;
  tap: SignalFlowSendTapWire | null;
}

/** Per-binding probe status — requested vs RT-confirmed kept distinct. */
export interface SignalFlowProbeStatusWire {
  /** `SemanticProbePoint` Debug text. */
  point: string;
  state: SignalFlowProbeStateWire;
  requested_mode: SignalFlowProbeModeWire;
  /** `null` until RT status evidence arrives — "probe 未取得". */
  confirmed_mode: SignalFlowProbeModeWire | null;
  generation: number;
  project_revision: number;
}

export interface SignalFlowViewStatusWire {
  project_revision: number;
  engine_attached: boolean;
  /** `RoutingActivationState` Debug text (`null` before first prepare). */
  activation: string | null;
  planned_generation: number | null;
  /** Non-RT publish evidence — never proof of RT activation. */
  published_generation: number | null;
  /** RT-observed active generation — the only `Active` evidence. */
  observed_active_generation: number | null;
  last_good_generation: number | null;
  /** `RoutingFault` Debug text when activation is Blocked/Failed. */
  fault: string | null;
  source_map_revision: number | null;
  source_map_generation: number | null;
  stale: boolean;
  probes: SignalFlowProbeStatusWire[];
}

/** `signal_flow_view_json` — the full `syn.signal_flow.view` body. */
export interface SignalFlowViewWire {
  project_id: string;
  project_revision: number;
  /** Echo of the visual state's `visual_revision` — staleness token. */
  view_generation: number;
  coverage: SignalFlowCoverageWire;
  /** `SignalFlowScope` Debug text (`Local` carries its root payload). */
  scope: string;
  nodes: SignalFlowViewNodeWire[];
  edges: SignalFlowViewEdgeWire[];
  status: SignalFlowViewStatusWire;
}

/** `syn.signal_flow.view` params (all optional; unset fields fall back
 * to the per-view persisted visual state, then defaults). */
export interface SignalFlowViewQuery {
  scope?: SignalFlowScopeParam;
  /** Required by the service when `scope === "local"`. */
  root?: SignalFlowEntity;
  roles?: SignalFlowRoleFilter[];
  collapsed?: SignalFlowEntity[];
  max_nodes?: number;
  max_edges?: number;
  /** Live-revision CAS — mismatch yields `coverage: "Stale"`. */
  expected_revision?: number;
}

// -- syn.signal_flow.visual ----------------------------------------------

export interface SignalFlowVisualGroupParam {
  group_id: number;
  label: string;
  members: SignalFlowEntity[];
  collapsed?: boolean;
}

/** `{ok, view, view_generation}` ack — `view_generation` is the
 * visual `visual_revision` staleness token. */
export interface SignalFlowVisualAck {
  ok: boolean;
  view: string;
  view_generation: number;
}

// -- syn.signal_flow.inspection ------------------------------------------

export type InspectionModeWire = "follow" | "explore" | "pinned";

/** `follow_source` wire object — `{"view": "view-{n}"}` or
 * `{"group": <u64>}`; `null` clears the narrowing. */
export type InspectionFollowSourceWire = { view: string } | { group: number };

/** `signal_flow_inspection_json` — the view-local inspection focus.
 * `explore_back`/`explore_fwd` are stack depths, not contents. */
export interface SignalFlowInspectionWire {
  target: SignalFlowEntity | null;
  mode: InspectionModeWire;
  follow_source: InspectionFollowSourceWire | null;
  explore_back: number;
  explore_fwd: number;
}

export interface SignalFlowInspectionResult {
  ok: boolean;
  view: string;
  inspection: SignalFlowInspectionWire;
}

// -- syn.signal_flow.routing ----------------------------------------------

/** `RoutingGestureFixation` wire shape — begin-fixed state. */
export interface RoutingFixationWire {
  view: string;
  /** Tagged `SemanticPortId` the drag started from. */
  source: string;
  /** Project revision at begin — the commit CAS. */
  revision: number;
  capability_generation: number;
  /** Owning window at begin (`null` = detached pool). */
  window: string | null;
}

/** `RoutingPortAssessment` wire shape — hover dry-run verdict. */
export type RoutingAssessmentWire =
  | { verdict: "compatible" }
  | {
      verdict: "needs_mapping";
      source_channels: number;
      destination_channels: number;
    }
  | { verdict: "prohibited"; reason: string };

/** `RoutingGesturePreview` wire shape. */
export interface RoutingPreviewWire {
  candidate: string | null;
  assessment: RoutingAssessmentWire | null;
  dropped_target: string | null;
}

/** `CancelReason` wire vocabulary (kebab serde). */
export type RoutingCancelReasonWire =
  | "escape"
  | "pointer-cancel"
  | "window-closed"
  | "target-deleted"
  | "input-scope-lost";

/** `RoutingGestureOutcome` wire shape — `stale` is terminal and
 * never retargets. */
export type RoutingOutcomeWire =
  | { outcome: "applied"; revision: number }
  | { outcome: "cancelled"; reason: RoutingCancelReasonWire }
  | { outcome: "stale" };

/** `state` action's `gesture` payload — fixation + optional preview. */
export interface RoutingGestureStateWire {
  fixation: RoutingFixationWire;
  preview: RoutingPreviewWire | null;
}

export interface RoutingBeginResult {
  ok: boolean;
  view: string;
  fixation: RoutingFixationWire;
}

export interface RoutingHoverResult {
  ok: boolean;
  view: string;
  assessment: RoutingAssessmentWire | null;
}

export interface RoutingDropResult {
  ok: boolean;
  view: string;
  /** Fixed target port (`null` = dropped on empty canvas → cancel). */
  dropped: string | null;
}

export interface RoutingConfirmResult {
  ok: boolean;
  view: string;
  outcome: RoutingOutcomeWire;
}

export interface RoutingCancelResult {
  ok: boolean;
  view: string;
  outcome: RoutingOutcomeWire;
}

export interface RoutingStateResult {
  ok: boolean;
  view: string;
  gesture: RoutingGestureStateWire | null;
}

/** `params.channel_map` — explicit channel-conversion consent. */
export interface RoutingChannelMapParam {
  entries: number[];
}

export class SignalFlowViewModel {
  private readonly viewIdValue: string | undefined;
  private projectionValue: SignalFlowViewWire | undefined;
  private viewGenerationValue = 0;
  private inspectionValue: SignalFlowInspectionWire | undefined;
  /** Last `routingState()` poll — `undefined` until polled, `null`
   * when the service reports no running gesture. Other gesture ops
   * return their wire response without refreshing this cache. */
  private gestureValue: RoutingGestureStateWire | null | undefined;

  constructor(options: { viewId?: string } = {}) {
    this.viewIdValue = options.viewId;
  }

  /** The configured wire `view_id` (`undefined` = service main-view
   * default is used — every `syn.signal_flow.*` op accepts the
   * omission). */
  get viewId(): string | undefined {
    return this.viewIdValue;
  }

  /** Last `syn.signal_flow.view` projection, verbatim. */
  get projection(): SignalFlowViewWire | undefined {
    return this.projectionValue;
  }

  /** Visual `visual_revision` — bumped by every `visual` ack and
   * echoed by `view` responses; `0` before the first response. */
  get viewGeneration(): number {
    return this.viewGenerationValue;
  }

  /** Last inspection focus returned by `syn.signal_flow.inspection`. */
  get inspection(): SignalFlowInspectionWire | undefined {
    return this.inspectionValue;
  }

  /** Last polled routing gesture (`undefined` = never polled). */
  get gesture(): RoutingGestureStateWire | null | undefined {
    return this.gestureValue;
  }

  private params(extra: Record<string, unknown>): Record<string, unknown> {
    return this.viewIdValue === undefined
      ? extra
      : { view_id: this.viewIdValue, ...extra };
  }

  /**
   * `syn.signal_flow.view` — fetches the headless projection and
   * stores it verbatim. `coverage`, `status`, Debug-vocabulary
   * fields, and the publish/active generation split are preserved
   * exactly as the wire sent them.
   */
  async loadView(query: SignalFlowViewQuery = {}): Promise<SignalFlowViewWire> {
    const view = await call<SignalFlowViewWire>(
      "syn.signal_flow.view",
      this.params({
        ...(query.scope !== undefined ? { scope: query.scope } : {}),
        ...(query.root !== undefined ? { root: query.root } : {}),
        ...(query.roles !== undefined ? { roles: query.roles } : {}),
        ...(query.collapsed !== undefined ? { collapsed: query.collapsed } : {}),
        ...(query.max_nodes !== undefined ? { max_nodes: query.max_nodes } : {}),
        ...(query.max_edges !== undefined ? { max_edges: query.max_edges } : {}),
        ...(query.expected_revision !== undefined
          ? { expected_revision: query.expected_revision }
          : {}),
      }),
    );
    if (!Number.isSafeInteger(view.view_generation)) {
      throw new RangeError(
        `syn.signal_flow.view returned invalid view_generation: ${String(view.view_generation)}`,
      );
    }
    this.projectionValue = view;
    this.viewGenerationValue = view.view_generation;
    return view;
  }

  private async visual(
    extra: Record<string, unknown>,
  ): Promise<SignalFlowVisualAck> {
    const ack = await call<SignalFlowVisualAck>(
      "syn.signal_flow.visual",
      this.params(extra),
    );
    if (Number.isSafeInteger(ack.view_generation)) {
      this.viewGenerationValue = ack.view_generation;
    }
    return ack;
  }

  // -- visual ops (SF-UX-02: visual-only, never reaches DSP/undo) -----

  moveNode(entity: SignalFlowEntity, x: number, y: number) {
    return this.visual({ action: "move_node", entity, x, y });
  }

  setGroup(group: SignalFlowVisualGroupParam) {
    return this.visual({
      action: "set_group",
      group_id: group.group_id,
      label: group.label,
      members: group.members,
      ...(group.collapsed !== undefined ? { collapsed: group.collapsed } : {}),
    });
  }

  fold(entity: SignalFlowEntity) {
    return this.visual({ action: "fold", entity });
  }

  unfold(entity: SignalFlowEntity) {
    return this.visual({ action: "unfold", entity });
  }

  zoom(zoom: number) {
    return this.visual({ action: "zoom", zoom });
  }

  pan(x: number, y: number) {
    return this.visual({ action: "pan", x, y });
  }

  /** `entity: null` clears the linked inspection focus. */
  linkFocus(entity: SignalFlowEntity | null) {
    return this.visual({ action: "link_focus", entity });
  }

  setScope(scope: SignalFlowScopeParam, root?: SignalFlowEntity) {
    return this.visual({
      action: "set_scope",
      scope,
      ...(root !== undefined ? { root } : {}),
    });
  }

  private async sendInspection(
    extra: Record<string, unknown>,
  ): Promise<SignalFlowInspectionResult> {
    const res = await call<SignalFlowInspectionResult>(
      "syn.signal_flow.inspection",
      this.params(extra),
    );
    this.inspectionValue = res.inspection;
    return res;
  }

  // -- inspection ops (SF-UX-04: view-local focus, selection untouched)

  inspect(entity: SignalFlowEntity) {
    return this.sendInspection({ action: "inspect", entity });
  }

  setInspectionMode(mode: InspectionModeWire) {
    return this.sendInspection({ action: "set_mode", mode });
  }

  /** `source: null` clears the Follow narrowing. */
  setFollowSource(source: InspectionFollowSourceWire | null) {
    return this.sendInspection({
      action: "set_follow_source",
      follow_source: source,
    });
  }

  inspectionBack() {
    return this.sendInspection({ action: "back" });
  }

  inspectionForward() {
    return this.sendInspection({ action: "forward" });
  }

  private routing<T>(extra: Record<string, unknown>): Promise<T> {
    return call<T>("syn.signal_flow.routing", this.params(extra));
  }

  // -- routing gesture ops (SF-UX-03) ------------------------------------
  // Every response is returned typed; `stale` outcomes are surfaced
  // verbatim and never retargeted client-side.

  /** `begin` — fixes view/source port/revision/capability/window. */
  routingBegin(port: string): Promise<RoutingBeginResult> {
    return this.routing({ action: "begin", port });
  }

  /** `hover` — `port: null` = empty canvas. */
  routingHover(port: string | null): Promise<RoutingHoverResult> {
    return this.routing({ action: "hover", port });
  }

  /** `drop` — fixes the hovered candidate; `null` = empty-canvas cancel. */
  routingDrop(): Promise<RoutingDropResult> {
    return this.routing({ action: "drop" });
  }

  /** `confirm` — `channelMap` is the explicit layout-consent. */
  routingConfirm(
    channelMap?: RoutingChannelMapParam,
  ): Promise<RoutingConfirmResult> {
    return this.routing({
      action: "confirm",
      ...(channelMap !== undefined ? { channel_map: channelMap } : {}),
    });
  }

  routingCancel(
    reason: RoutingCancelReasonWire = "escape",
  ): Promise<RoutingCancelResult> {
    return this.routing({ action: "cancel", reason });
  }

  /** `state` — fixation + preview of the running gesture; refreshes
   * the `gesture` cache. */
  async routingState(): Promise<RoutingStateResult> {
    const res = await this.routing<RoutingStateResult>({ action: "state" });
    this.gestureValue = res.gesture;
    return res;
  }
}
