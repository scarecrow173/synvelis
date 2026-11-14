/**
 * `ProbeViewModel` — DOM-free consumer of the `syn.probe.*` ops
 * (SF-05 ui slice, synvelis#108).
 *
 * The service side owns lease/dedup/capture-owner invariants
 * (`ControlSession` probe surface + `ProbeTraceBridge`); this view
 * model is the typed carrier between the wire contract and a future
 * SolidJS surface. It never fabricates bindings, observations, or
 * availability:
 *
 * - `syn.probe.control` view-scoped actions (`subscribe`,
 *   `unsubscribe`, `capture_for`, `abort_capture`) require the
 *   lease-owning `view_id` (`"view-{n}"`) — it is a required
 *   constructor option so no call site can silently attach a lease
 *   to an implicit default view. Session-scoped actions
 *   (`request`/`set_mode`/`capture`/`stop_capture`/`remove`) never
 *   send `view_id`.
 * - `syn.probe.state` reports `engine_attached: false` with honest
 *   empty collections when no routing engine is attached — the VM
 *   stores that projection verbatim instead of erroring or
 *   inventing rows.
 * - `syn.probe.compare` yields the `AcousticObservation` envelope
 *   verbatim: `quality.kind` stays `aligned`/`conditional`/
 *   `incomparable`/`unknown` — `conditional`/`incomparable` are
 *   never promoted to `aligned`, and `engine_attached: false`
 *   surfaces as `observation: null`, not a synthesized result.
 *
 * Wire-vocabulary rule: probe ops serialize contract enums with
 * serde kebab-case (`"bound-active"`, `"off"`, `"metrics"`) — this
 * is a *different* spelling than the `Debug` vocabulary
 * `syn.signal_flow.view` uses for the same Rust types. One
 * exception inside probe payloads: `SemanticSignalEntityRef`'s
 * serde tag carries the PascalCase variant name (`"Track"`), not
 * kebab — the `*Wire` suffix types below pin the exact spellings;
 * do not merge them with the signal-flow `*Wire` types or
 * "normalize" either direction.
 */

import { call } from "../ipc.ts";

// -- request vocabulary (serde kebab-case) ------------------------------

/** `ProbeMode` — `"off" | "monitor" | "capture"`. */
export type ProbeModeWire = "off" | "monitor" | "capture";

/** `SubscriptionKind` — `"metrics" | "pcm"`. */
export type SubscriptionKindWire = "metrics" | "pcm";

/** `SignalBoundary` — where along the entity's path the point sits. */
export type SignalBoundaryWire =
  | "pre-insert"
  | "post-insert"
  | "pre-fader"
  | "post-fader"
  | "input"
  | "output"
  | "sidechain";

/** `SemanticSignalEntityRef` kinds — probe points have no `port`/
 * `connection` counterpart (sends resolve through their strip).
 * NOTE: this enum has no serde `rename_all`, so the `entity` tag
 * carries the PascalCase variant name (`"Track"`), unlike the
 * kebab-case `boundary`/`kind`/`mode` fields beside it. */
export type SemanticSignalEntityKindWire =
  | "Track"
  | "Bus"
  | "Strip"
  | "Processor"
  | "Send";

/** `SemanticSignalEntityRef` serde shape — `{"entity": kind,
 * "id": "<kind>-<32hex>"}` (`#[serde(tag, content)]`). */
export interface SemanticSignalEntityWire {
  entity: SemanticSignalEntityKindWire;
  id: string;
}

/** `SemanticProbePoint` serde shape — `port` is an optional tagged
 * `SemanticPortId` disambiguator, never an arbitrary override. */
export interface SemanticProbePointWire {
  entity: SemanticSignalEntityWire;
  boundary: SignalBoundaryWire;
  port?: string;
}

/** `ProbeSubscriptionKey` serde shape — session + point + kind. */
export interface ProbeSubscriptionKeyWire {
  session_id: number;
  point: SemanticProbePointWire;
  kind: SubscriptionKindWire;
}

/** `ProbeFrameInterval` — half-open `[start, end)` cursor frames. */
export interface ProbeFrameIntervalWire {
  start: number;
  end: number;
}

// -- syn.probe.control responses ------------------------------------------

export interface ProbeRequestResult {
  key: ProbeSubscriptionKeyWire;
  /** Session-correlated tagged `ParameterTargetId`s. */
  targets: string[];
}

export interface ProbeSetModeResult {
  key: ProbeSubscriptionKeyWire;
  mode: ProbeModeWire;
}

export interface ProbeCaptureResult {
  key: ProbeSubscriptionKeyWire;
  max_frames: number;
}

export interface ProbeKeyResult {
  key: ProbeSubscriptionKeyWire;
}

export interface ProbeRemoveResult {
  key: ProbeSubscriptionKeyWire;
  removed: boolean;
}

export interface ProbeSubscribeResult {
  key: ProbeSubscriptionKeyWire;
  view_id: string;
  targets: string[];
}

export interface ProbeUnsubscribeResult {
  key: ProbeSubscriptionKeyWire;
  view_id: string;
  released: boolean;
  removed: boolean;
}

export interface ProbeCaptureForResult {
  key: ProbeSubscriptionKeyWire;
  view_id: string;
  max_frames: number;
  /** Recorded capture owner (`"session"`/`"view-{n}"`), `null` when
   * the key has no live binding. */
  owner: string | null;
}

export interface ProbeAbortCaptureResult {
  key: ProbeSubscriptionKeyWire;
  view_id: string;
}

// -- syn.probe.state projection -------------------------------------------

/** `ProbeResolveState` serde kebab-case — lifecycle/resolution state.
 * Requested/queued states are never proof of RT installation; only
 * `"bound-active"` is RT-observed. */
export type ProbeBindingStateWire =
  | "requested"
  | "unresolved"
  | "unavailable"
  | "unsupported"
  | "queued-for-binding"
  | "pending-rt-install"
  | "bound-active"
  | "stale-generation"
  | "partially-rebound"
  | "lost"
  | "stopped";

/** `ProbeStatusKind` serde-tagged object — payload fields vary per
 * `kind`; kept verbatim, never interpreted. */
export interface ProbeStatusKindWire {
  kind: string;
  [key: string]: unknown;
}

/** `ProbeStatusReceipt` wire shape — `clock` is `ClockContextRef`
 * Debug text (`null` = unwired/unknown). */
export interface ProbeStatusReceiptWire {
  kind: ProbeStatusKindWire;
  generation: number;
  sample_position: number;
  clock: string | null;
}

export interface ProbeBindingWire {
  key: ProbeSubscriptionKeyWire;
  targets: string[];
  point: SemanticProbePointWire;
  project_revision: number;
  active_plan_generation: number;
  /** Broker-issued runtime probe id (`null` until install). */
  runtime_probe_id: number | null;
  /** Last requested mode — intent, not observation. */
  requested_mode: ProbeModeWire;
  /** RT-confirmed mode (`null` until status evidence arrives). */
  confirmed_mode: ProbeModeWire | null;
  state: ProbeBindingStateWire;
  last_status: ProbeStatusReceiptWire | null;
  /** Lease principals (`"session"`/`"view-{n}"`). */
  holders: string[];
  capture_owner: string | null;
}

export interface ProbeSubscriberWire {
  view_id: string;
  keys: ProbeSubscriptionKeyWire[];
}

/** `ObservationGapKind` serde kebab-case. */
export type ObservationGapKindWire =
  | "unobserved"
  | "lost-blocks"
  | "lost-records"
  | "generation-boundary"
  | "rebind-dropped"
  | "off"
  | "unsupported"
  | "unavailable"
  | "unknown";

export interface ObservationGapWire {
  range: ProbeFrameIntervalWire;
  kind: ObservationGapKindWire;
}

/** `CaptureCompleteness` serde kebab-case. */
export type CaptureCompletenessWire = "complete" | "incomplete" | "unknown";

/** `CaptureSegmentEnd` serde kebab-case (`null` while open — open
 * segments are never exposed). */
export type CaptureSegmentEndWire =
  | "requested"
  | "budget-exhausted"
  | "generation-boundary"
  | "rebind-dropped"
  | "removed"
  | "clock-epoch"
  | "unknown";

export interface ProbeCaptureSegmentWire {
  capture_session_id: number;
  segment_index: number;
  point: SemanticProbePointWire;
  active_plan_generation: number;
  /** `ClockContextRef` Debug text (`null` = unknown). */
  clock_context: string | null;
  frame_range: ProbeFrameIntervalWire | null;
  discontinuities: ObservationGapWire[];
  completeness: CaptureCompletenessWire;
  ended_by: CaptureSegmentEndWire | null;
}

export interface ProbeDrainReportWire {
  updates: number;
  uncorrelated: number;
  recorded: number;
  unrouted: number;
}

/** `TraceSlot<ProbeObservation>` wire state — `Missing`/`Disabled`/
 * `Lost`/`Measuring` stay distinguishable; `value` is Debug text. */
export type ProbeTraceStateWire =
  | "present"
  | "missing"
  | "unsupported"
  | "dropped"
  | "expired"
  | "unknown";

export interface ProbeTraceRowWire {
  /** Tagged `ParameterTargetId`. */
  target: string;
  observed: {
    state: ProbeTraceStateWire;
    value: string | null;
  };
}

/** `syn.probe.state` body — `engine_attached: false` carries honest
 * empty collections (never fabricated rows). */
export interface ProbeStateWire {
  engine_attached: boolean;
  bindings: ProbeBindingWire[];
  subscribers: ProbeSubscriberWire[];
  segments: ProbeCaptureSegmentWire[];
  drain: ProbeDrainReportWire;
  traces: ProbeTraceRowWire[];
}

// -- syn.probe.compare -----------------------------------------------------

/** `ProbeMetricKind` serde kebab-case (request intent). */
export type ProbeMetricKindWire = "peak-rms" | "pcm-difference";

/** `ProbeAlignPolicy` — explicit consumer alignment choices. */
export interface ProbeAlignPolicyWire {
  require_same_generation: boolean;
  require_known_latency: boolean;
  allow_rate_conversion: boolean;
  unknown_latency_bound_frames?: number;
  max_error_frames: number;
  allow_conditional: boolean;
  prefer_engine_frame: boolean;
}

/** `ProbeComparisonRequest` serde shape — sent verbatim as
 * `params.request`. */
export interface ProbeComparisonRequestWire {
  before: SemanticProbePointWire;
  after: SemanticProbePointWire;
  observed_range?: ProbeFrameIntervalWire;
  alignment_policy?: ProbeAlignPolicyWire;
  metric_set?: ProbeMetricKindWire[];
  max_bytes: number;
}

/** `ClockContextRef` wire shape — raw domain/epoch ids. */
export interface ProbeClockContextWire {
  domain: number;
  epoch: number;
}

/** `ProbeComparisonBasis` wire shape — the coordinate basis an
 * alignment was computed in (`"unknown"` covers future variants). */
export type ProbeComparisonBasisWire =
  | { kind: "shared-cursor"; clock: ProbeClockContextWire }
  | { kind: "engine-frame"; epoch: number }
  | { kind: "unknown" };

/** `ProbeAlignCondition` wire shape — serde-tagged `kind` plus
 * variant payload; kept verbatim. */
export interface ProbeAlignConditionWire {
  kind: string;
  [key: string]: unknown;
}

/** `ProbeIncomparableReason` wire shape — serde-tagged `kind` plus
 * variant payload (`side`, `channels`, `a`/`b`, …); kept verbatim. */
export interface ProbeIncomparableReasonWire {
  kind: string;
  [key: string]: unknown;
}

/** `ProbeComparisonQuality` wire shape — the three-valued verdict
 * plus the `"unknown"` catch-all the service emits for
 * `#[non_exhaustive]` future variants. Never promoted client-side. */
export type ProbeComparisonQualityWire =
  | {
      kind: "aligned";
      basis: ProbeComparisonBasisWire;
      offset_frames: number;
      error_budget_frames: number;
    }
  | {
      kind: "conditional";
      basis: ProbeComparisonBasisWire;
      offset_frames: number;
      error_budget_frames: number;
      conditions: ProbeAlignConditionWire[];
      excluded_intervals: ProbeFrameIntervalWire[];
    }
  | { kind: "incomparable"; reason: ProbeIncomparableReasonWire }
  | { kind: "unknown" };

/** `ProbeComparisonSide` serde kebab-case. */
export type ProbeComparisonSideWire = "before" | "after";

export interface ProbeComparisonGapWire {
  side: ProbeComparisonSideWire;
  range: ProbeFrameIntervalWire;
  kind: ObservationGapKindWire;
}

export interface ProbeGenerationRangeWire {
  first: number;
  last: number;
}

/** `ProbeComparisonProvenance` wire shape — clock identities as raw
 * domain/epoch ids (`null` = side unresolved or mark unknown). */
export interface ProbeComparisonProvenanceWire {
  before_clock: ProbeClockContextWire | null;
  after_clock: ProbeClockContextWire | null;
  before_generations: ProbeGenerationRangeWire | null;
  after_generations: ProbeGenerationRangeWire | null;
  runtime_instance: number;
  sample_rate: number;
}

/** `AcousticObservation` wire shape (`probe_observation_json`) —
 * `before`/`after` echo the request's semantic points. */
export interface AcousticObservationWire {
  before: SemanticProbePointWire;
  after: SemanticProbePointWire;
  quality: ProbeComparisonQualityWire;
  interval: ProbeFrameIntervalWire | null;
  gap_mask: ProbeComparisonGapWire[];
  provenance: ProbeComparisonProvenanceWire;
}

/** `syn.probe.compare` body — discriminated on `engine_attached`:
 * detached means `observation: null` (honest Unavailable), never a
 * synthesized observation. */
export type ProbeCompareResult =
  | { engine_attached: true; observation: AcousticObservationWire }
  | { engine_attached: false; observation: null };

export class ProbeViewModel {
  private readonly viewIdValue: string;
  private stateValue: ProbeStateWire | undefined;
  private comparisonValue: ProbeCompareResult | undefined;

  constructor(options: { viewId: string }) {
    this.viewIdValue = options.viewId;
  }

  /** The lease-owning wire `view_id` (`"view-{n}"`) sent with every
   * view-scoped control action. */
  get viewId(): string {
    return this.viewIdValue;
  }

  /** Last `syn.probe.state` projection, verbatim (`undefined` until
   * the first successful poll). */
  get state(): ProbeStateWire | undefined {
    return this.stateValue;
  }

  /** Last `syn.probe.compare` result, verbatim. */
  get comparison(): ProbeCompareResult | undefined {
    return this.comparisonValue;
  }

  private control<T>(extra: Record<string, unknown>): Promise<T> {
    return call<T>("syn.probe.control", extra);
  }

  // -- session-scoped actions (no view_id on the wire) ------------------

  /** `request` — session authority probe request → `{key, targets}`. */
  request(
    target: string,
    point: SemanticProbePointWire,
    kind: SubscriptionKindWire,
    mode: ProbeModeWire,
  ): Promise<ProbeRequestResult> {
    return this.control({
      action: "request",
      target,
      point,
      kind,
      mode,
    });
  }

  setMode(key: ProbeSubscriptionKeyWire, mode: ProbeModeWire) {
    return this.control<ProbeSetModeResult>({ action: "set_mode", key, mode });
  }

  capture(key: ProbeSubscriptionKeyWire, maxFrames: number) {
    return this.control<ProbeCaptureResult>({
      action: "capture",
      key,
      max_frames: maxFrames,
    });
  }

  stopCapture(key: ProbeSubscriptionKeyWire) {
    return this.control<ProbeKeyResult>({ action: "stop_capture", key });
  }

  remove(key: ProbeSubscriptionKeyWire) {
    return this.control<ProbeRemoveResult>({ action: "remove", key });
  }

  // -- view-scoped lease actions (view_id required — per-view ownership)

  subscribe(
    target: string,
    point: SemanticProbePointWire,
    kind: SubscriptionKindWire,
    mode: ProbeModeWire,
  ): Promise<ProbeSubscribeResult> {
    return this.control({
      action: "subscribe",
      view_id: this.viewId,
      target,
      point,
      kind,
      mode,
    });
  }

  unsubscribe(key: ProbeSubscriptionKeyWire): Promise<ProbeUnsubscribeResult> {
    return this.control({
      action: "unsubscribe",
      view_id: this.viewId,
      key,
    });
  }

  captureFor(
    key: ProbeSubscriptionKeyWire,
    maxFrames: number,
  ): Promise<ProbeCaptureForResult> {
    return this.control({
      action: "capture_for",
      view_id: this.viewId,
      key,
      max_frames: maxFrames,
    });
  }

  abortCapture(key: ProbeSubscriptionKeyWire): Promise<ProbeAbortCaptureResult> {
    return this.control({
      action: "abort_capture",
      view_id: this.viewId,
      key,
    });
  }

  // -- read projections ----------------------------------------------------

  /** `syn.probe.state` — read-only projection; never drains. Stores
   * the result verbatim (`engine_attached: false` → empty
   * collections, the honest unavailable state). */
  async refreshState(): Promise<ProbeStateWire> {
    const state = await call<ProbeStateWire>("syn.probe.state");
    this.stateValue = state;
    return state;
  }

  /** `syn.probe.compare` — alignment envelope over retained evidence.
   * `quality.kind` is preserved verbatim; a detached engine yields
   * `{engine_attached: false, observation: null}`. */
  async compare(
    request: ProbeComparisonRequestWire,
  ): Promise<ProbeCompareResult> {
    const res = await call<ProbeCompareResult>("syn.probe.compare", { request });
    this.comparisonValue = res;
    return res;
  }
}
