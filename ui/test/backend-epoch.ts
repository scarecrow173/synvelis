/**
 * Backend-side epoch ledger — a stale-guard double that mirrors the
 * Rust `ControlsUi` view/binding generation contract
 * (`crates/synvelis-workspace/src/controls_ui.rs`):
 *
 * - `syn.view.attach` transitions bump the view epoch and every
 *   binding of the view adopts it; the reply reports the live epoch.
 *   Same-window re-attach while attached is an idempotent no-op; a
 *   different window is a dock transfer (detach + attach).
 * - `syn.view.detach` bumps every binding epoch so dead-epoch work
 *   cannot land; `syn.view.close` additionally drops the bindings.
 * - `syn.control.subscribe` registers a binding under the view's live
 *   epoch (detached/unknown views mint detached bindings).
 * - `syn.control.dispatch` applies the same drop order as the real
 *   guard: detached view → detached binding → `ctx.generation` behind
 *   the live epoch → dropped with NO receipt (`staleIntents` /
 *   `detachedIntents` count instead).
 *
 * The ledger owns its own counters — it never echoes values the
 * frontend sent. That is the point: a fake that simply returns what
 * the SDK stamped cannot observe the remount P1 where the wire
 * `ctx.generation` trails the backend-live epoch. Cross-checked
 * against the real backend by `crates/synvelis-app/tests/ui_ops.rs`
 * (`unknown_and_detached_dispatch_are_dropped_not_faked`).
 *
 * Scope: the epoch/guard contract only. Domain routing, capability,
 * gesture, and projection-content semantics stay in the Rust tests —
 * accepted non-preview dispatches mint an `applied`/`accepted` receipt
 * so the receipt→ack path is exercised end to end.
 */

import type { FakeHost, SentIntent } from "./fake-host.ts";

interface ViewState {
  generation: number;
  attached: boolean;
  windowId: string | undefined;
}

interface BindingState {
  generation: number;
  attached: boolean;
  tracked: Set<string>;
}

export interface MintedReceipt {
  viewId: string;
  target: string;
  commandId: string;
  status: string;
}

export interface BackendEpochGuard {
  /** Intents dropped by the generation guard. */
  readonly staleIntents: number;
  /** Intents dropped on detached/unknown views or bindings. */
  readonly detachedIntents: number;
  /** Intents dropped as already-tracked command ids. */
  readonly duplicateCommands: number;
  /** Non-preview dispatches that passed the guard. */
  readonly acceptedIntents: number;
  /** Receipts minted on `syn.control.receipt`, in order. */
  readonly receipts: readonly MintedReceipt[];
  /** Live view epoch (0 when the view never attached). */
  viewGeneration(viewId: string): number;
  /** Live binding epoch; `undefined` when no binding exists. */
  bindingGeneration(viewId: string, target: string): number | undefined;
  /**
   * Process every unprocessed intent on the wire in send order —
   * ledger updates always happen; op replies are delivered unless
   * `answer === false` (they stay queued for out-of-order tests).
   */
  pump(options?: { answer?: boolean }): void;
  /** Deliver one queued reply by wire intent id. */
  answerQueuedId(intentId: string): void;
  /** Deliver every queued reply in send order. */
  drainQueued(): void;
}

export function createBackendEpochGuard(host: FakeHost): BackendEpochGuard {
  const views = new Map<string, ViewState>();
  // Mirrors the backend's (ViewId, LogicalTarget) tuple key — a nested
  // map cannot collide across prefix-related viewIds or (view,target)
  // pairs the way a concatenated string key does.
  const bindings = new Map<string, Map<string, BindingState>>();
  const receipts: MintedReceipt[] = [];
  const queued: { id: string; body: unknown }[] = [];
  let processed = 0;
  let staleIntents = 0;
  let detachedIntents = 0;
  let duplicateCommands = 0;
  let acceptedIntents = 0;

  function bindingsOf(view: string): BindingState[] {
    const inner = bindings.get(view);
    return inner === undefined ? [] : [...inner.values()];
  }

  function detachView(view: string): void {
    const v = views.get(view);
    if (v === undefined || !v.attached) return;
    v.attached = false;
    v.windowId = undefined;
    for (const b of bindingsOf(view)) {
      b.attached = false;
      b.generation += 1;
      b.tracked.clear();
    }
  }

  function ensureBinding(view: string, target: string): BindingState {
    let inner = bindings.get(view);
    if (inner === undefined) {
      inner = new Map();
      bindings.set(view, inner);
    }
    let b = inner.get(target);
    if (b === undefined) {
      const v = views.get(view);
      b = {
        generation: v?.generation ?? 0,
        attached: v?.attached ?? false,
        tracked: new Set(),
      };
      inner.set(target, b);
    }
    return b;
  }

  function emitReceipt(
    viewId: string,
    target: string,
    commandId: string,
    status: string,
  ): void {
    receipts.push({ viewId, target, commandId, status });
    host.emit({
      v: 1,
      kind: "notify",
      data: {
        channel: "syn.control.receipt",
        data: {
          view_id: viewId,
          target,
          receipt: { commandId, status },
        },
      },
    });
  }

  function handle(s: SentIntent): void {
    const p = s.params;
    switch (s.op) {
      case "syn.view.attach": {
        const viewId = p.view_id as string;
        const windowId = p.window_id as string;
        let v = views.get(viewId);
        if (v === undefined) {
          v = { generation: 0, attached: false, windowId: undefined };
          views.set(viewId, v);
        }
        if (!(v.attached && v.windowId === windowId)) {
          if (v.attached) detachView(viewId);
          v.attached = true;
          v.windowId = windowId;
          v.generation += 1;
          for (const b of bindingsOf(viewId)) {
            b.attached = true;
            b.generation = v.generation;
          }
        }
        queued.push({
          id: s.id,
          body: { view_id: viewId, attached: true, generation: v.generation },
        });
        return;
      }
      case "syn.view.detach": {
        const viewId = p.view_id as string;
        detachView(viewId);
        queued.push({ id: s.id, body: { view_id: viewId, attached: false } });
        return;
      }
      case "syn.view.close": {
        const viewId = p.view_id as string;
        detachView(viewId);
        bindings.delete(viewId);
        queued.push({ id: s.id, body: { view_id: viewId, closed: true } });
        return;
      }
      case "syn.control.subscribe": {
        ensureBinding(p.view_id as string, p.target as string);
        queued.push({ id: s.id, body: {} });
        return;
      }
      case "syn.control.unsubscribe": {
        queued.push({ id: s.id, body: {} });
        return;
      }
      case "syn.control.dispatch": {
        const intent = p.intent as { kind?: string; target?: string };
        const ctx = p.ctx as {
          commandId?: string;
          viewId?: string;
          generation?: number;
        };
        const viewId = ctx.viewId ?? "";
        const target = intent.target ?? "";
        const generation = ctx.generation ?? 0;
        const commandId = ctx.commandId ?? "";
        const v = views.get(viewId);
        if (v === undefined || !v.attached) {
          detachedIntents += 1;
          queued.push({ id: s.id, body: {} });
          return;
        }
        const b = bindings.get(viewId)?.get(target);
        if (b !== undefined) {
          if (!b.attached) {
            detachedIntents += 1;
            queued.push({ id: s.id, body: {} });
            return;
          }
          if (generation < b.generation) {
            staleIntents += 1;
            queued.push({ id: s.id, body: {} });
            return;
          }
          if (b.tracked.has(commandId)) {
            duplicateCommands += 1;
            queued.push({ id: s.id, body: {} });
            return;
          }
        } else if (generation < v.generation) {
          staleIntents += 1;
          queued.push({ id: s.id, body: {} });
          return;
        }
        acceptedIntents += 1;
        if (intent.kind !== "preview") {
          ensureBinding(viewId, target).tracked.add(commandId);
          emitReceipt(
            viewId,
            target,
            commandId,
            intent.kind === "begin" || intent.kind === "cancel"
              ? "accepted"
              : "applied",
          );
        }
        queued.push({ id: s.id, body: {} });
        return;
      }
      default:
        // Fail loud: an unmodeled op otherwise leaves the `call` reply
        // unqueued forever and the test dies on an opaque timeout
        // instead of naming the op the ledger does not know.
        throw new Error(`backend-epoch ledger: unmodeled op ${s.op}`);
    }
  }

  function drainQueued(): void {
    while (queued.length > 0) {
      const q = queued.shift()!;
      host.answer(q.id, q.body);
    }
  }

  return {
    get staleIntents() {
      return staleIntents;
    },
    get detachedIntents() {
      return detachedIntents;
    },
    get duplicateCommands() {
      return duplicateCommands;
    },
    get acceptedIntents() {
      return acceptedIntents;
    },
    receipts,
    viewGeneration(viewId) {
      return views.get(viewId)?.generation ?? 0;
    },
    bindingGeneration(viewId, target) {
      return bindings.get(viewId)?.get(target)?.generation;
    },
    pump(options) {
      for (; processed < host.sent.length; processed += 1) {
        handle(host.sent[processed]!);
      }
      if (options?.answer !== false) drainQueued();
    },
    answerQueuedId(intentId) {
      const i = queued.findIndex((q) => q.id === intentId);
      if (i === -1) throw new Error(`no queued reply for ${intentId}`);
      const [q] = queued.splice(i, 1);
      host.answer(q!.id, q!.body);
    },
    drainQueued,
  };
}
