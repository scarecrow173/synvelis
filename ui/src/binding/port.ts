/**
 * `BindingPort` over `window.__rvf` — the frontend half of the
 * `syn.control.*`/`syn.viz.*` op surface.
 *
 * Contract notes (upstream `ui/binding`):
 * - `dispatch` is fire-and-forget: the op result only acks queueing;
 *   receipts arrive on `syn.control.receipt` and are routed to the
 *   issuing binding by the view model (`binding.ack`).
 * - `subscribe` registers a projection listener keyed (viewId, target)
 *   before the op fires so the initial hydration push cannot race the
 *   route registration.
 * - `query`/`cancelQuery` implement the bounded `ViewportQuery`
 *   contract over `syn.viz.query`; aborts are honored client-side —
 *   the op itself is synchronous so there is no wire cancel to send.
 */

import type {
  BindingPort,
  DispatchContext,
  ProjectionListener,
  QueryError,
  QueryId,
  QueryReply,
  StateProjection,
  SubscriptionContext,
  Unsubscribe,
  ViewportQuery,
} from "@ruviform/ui-binding";

import { call } from "../ipc.ts";
import type { NotifyRouter } from "./router.ts";

/** `syn.control.projection` row — mirrors `ui_ops.rs`. */
interface WireRow {
  kind?: string;
  observed?: {
    revision?: number;
    value?: unknown;
    provenance?: string | null;
  };
}

interface WireProjection {
  view_id?: string;
  target?: string;
  row?: WireRow;
}

function queryError(kind: QueryError["kind"], detail: string): QueryError {
  return { kind, detail };
}

export interface IpcPortOptions {
  readonly router: NotifyRouter;
  /**
   * Resolves a `ViewportQuery.sourceId` to the bound surface id —
   * the backend keys surfaces by `(view, surface)`, the query
   * contract keys by source. `undefined` ⇒ `sourceId` is the surface.
   */
  readonly surfaceFor?: (viewId: string, sourceId: string) => string | undefined;
}

export function createIpcBindingPort(options: IpcPortOptions): BindingPort {
  const { router } = options;
  const surfaceFor = options.surfaceFor ?? ((_v, s) => s);

  return {
    dispatch(intent, ctx: DispatchContext) {
      // `expectReceipt` is producer-side metadata, not a wire field —
      // the backend `DispatchContext` denies unknown keys.
      void call("syn.control.dispatch", {
        intent,
        ctx: {
          commandId: ctx.commandId,
          ...(ctx.viewId !== undefined ? { viewId: ctx.viewId } : {}),
          ...(ctx.windowId !== undefined ? { windowId: ctx.windowId } : {}),
          generation: ctx.generation,
          issuedAt: ctx.issuedAt,
        },
      }).catch(() => {
        // Dispatch must not throw per the port contract; the command's
        // ack window simply elapses into `timeout` upstream.
      });
    },

    subscribe<P>(
      target: string,
      listener: ProjectionListener<P>,
      ctx: SubscriptionContext,
    ): Unsubscribe | undefined {
      const off = router.on("syn.control.projection", (data) => {
        const p = data as WireProjection;
        if (p.view_id !== ctx.viewId || p.target !== target) return;
        const observed = p.row?.observed;
        if (observed === undefined || observed === null) return;
        const projection: StateProjection<P> = {
          revision: observed.revision ?? 0,
          value: observed.value as P,
          ...(observed.provenance != null
            ? { provenance: observed.provenance }
            : {}),
        };
        listener(projection);
      });
      void call("syn.control.subscribe", {
        view_id: ctx.viewId,
        target,
      }).catch(() => {
        // Subscribe failure stays honest: no projection arrives and
        // the binding shows `idle` — never faked state.
      });
      return () => {
        off();
        void call("syn.control.unsubscribe", {
          view_id: ctx.viewId,
          target,
        }).catch(() => {});
      };
    },

    async query<T>(
      q: ViewportQuery,
      signal?: AbortSignal,
    ): Promise<QueryReply<T>> {
      if (signal?.aborted) {
        throw queryError("cancelled", "aborted before issue");
      }
      const surface = surfaceFor(q.viewId, q.sourceId);
      if (surface === undefined) {
        throw queryError("unsupported", `no surface for source ${q.sourceId}`);
      }
      const params: Record<string, unknown> = {
        view_id: q.viewId,
        surface_id: surface,
        start: q.start,
        end: q.end,
        buckets: q.buckets ?? 0,
      };
      if (q.maxBytes !== undefined) params.max_bytes = q.maxBytes;
      const abort = signal
        ? (() => {
            let onAbort: (() => void) | undefined;
            const p = new Promise<never>((_, rej) => {
              onAbort = () => rej(queryError("cancelled", "aborted"));
            });
            signal.addEventListener("abort", () => onAbort?.(), {
              once: true,
            });
            return p;
          })()
        : undefined;
      const request = call<{
        view?: unknown;
        status?: unknown;
        stale?: boolean;
        source?: string;
        bytes?: number;
      }>("syn.viz.query", params);
      const res = await (abort !== undefined
        ? Promise.race([request, abort])
        : request);
      if (signal?.aborted) throw queryError("cancelled", "aborted");
      return {
        generation: q.generation,
        sourceRevision: q.sourceRevision,
        payload: {
          view: res.view,
          status: res.status,
          stale: res.stale === true,
          source: res.source,
        } as T,
        ...(res.bytes !== undefined ? { bytes: res.bytes } : {}),
      };
    },

    cancelQuery(_queryId: QueryId): void {
      // `syn.viz.query` resolves synchronously in-process — the
      // AbortSignal guard above is the whole cancel path.
    },
  };
}
