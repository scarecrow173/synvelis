/**
 * Faithful `window.__rvf`/`window.ipc` harness — mirrors the injected
 * `ruviform_ui::BRIDGE_JS` semantics: `call` posts an `rvf.ipc.v1`
 * intent and resolves/rejects on the correlated `result` event;
 * non-result events fan out to `onEvent` listeners.
 *
 * This is a *test harness*, not the bridge itself — the real bridge is
 * injected by the host at page load.
 */

import type { RvfEvent } from "../src/ipc.ts";

export interface SentIntent {
  v: number;
  kind: string;
  id: string;
  op: string;
  params: Record<string, unknown>;
}

export interface FakeHost {
  sent: SentIntent[];
  /** Pushes an event as the Rust side would (`dispatch_script`). */
  emit(message: unknown): void;
  /** Resolves the intent `id` with `data`. */
  answer(id: string, data: unknown): void;
  /** Rejects the intent `id` with a wire error body. */
  answerErr(id: string, code: string, message: string): void;
  /** Resolves the most recent intent sent for `op`. */
  answerOp(op: string, data: unknown): void;
  /** Rejects the most recent intent sent for `op`. */
  answerOpErr(op: string, code: string, message: string): void;
  /** Removes the bridge/window.ipc globals. */
  uninstall(): void;
}

export function installFakeHost(): FakeHost {
  const sent: SentIntent[] = [];
  const listeners = new Set<(e: RvfEvent) => void>();
  const pending = new Map<
    string,
    { res: (v: unknown) => void; rej: (e: unknown) => void }
  >();
  let seq = 0;

  window.ipc = {
    postMessage(raw: string) {
      sent.push(JSON.parse(raw) as SentIntent);
    },
  };

  window.__rvf = {
    version: 1,
    call(op: string, params: Record<string, unknown> = {}) {
      const id = `i-${++seq}`;
      return new Promise((res, rej) => {
        pending.set(id, { res, rej });
        window.ipc!.postMessage(
          JSON.stringify({ v: 1, kind: "intent", id, op, params }),
        );
      });
    },
    onEvent(cb: (e: RvfEvent) => void) {
      listeners.add(cb);
      return () => {
        listeners.delete(cb);
      };
    },
    dataUrl(name: string, offset = 0, len?: number) {
      const q = new URLSearchParams({ offset: String(offset) });
      if (len !== undefined) q.set("len", String(len));
      const path = name.split("/").map(encodeURIComponent).join("/");
      return `rvf://data/${path}?${q}`;
    },
    dispatch(m: unknown) {
      const msg = m as { kind?: string; id?: string; ok?: boolean; data?: unknown; error?: { code?: string; message?: string } };
      if (msg?.kind === "result" && typeof msg.id === "string" && pending.has(msg.id)) {
        const p = pending.get(msg.id)!;
        pending.delete(msg.id);
        if (msg.ok) {
          p.res(msg.data);
        } else {
          const e = new Error(msg.error?.message ?? "rvf error") as Error & {
            code: string | undefined;
          };
          e.code = msg.error?.code;
          p.rej(e);
        }
        return;
      }
      for (const cb of [...listeners]) cb(msg as RvfEvent);
    },
  };

  const lastIntentFor = (op: string): SentIntent => {
    const intent = [...sent].reverse().find((s) => s.op === op);
    if (!intent) throw new Error(`no intent sent for ${op}`);
    return intent;
  };

  return {
    sent,
    emit: (m) => window.__rvf!.dispatch(m),
    answer: (id, data) =>
      window.__rvf!.dispatch({ v: 1, kind: "result", id, ok: true, data }),
    answerErr: (id, code, message) =>
      window.__rvf!.dispatch({
        v: 1,
        kind: "result",
        id,
        ok: false,
        error: { code, message },
      }),
    answerOp: (op, data) => {
      const i = lastIntentFor(op);
      window.__rvf!.dispatch({ v: 1, kind: "result", id: i.id, ok: true, data });
    },
    answerOpErr: (op, code, message) => {
      const i = lastIntentFor(op);
      window.__rvf!.dispatch({
        v: 1,
        kind: "result",
        id: i.id,
        ok: false,
        error: { code, message },
      });
    },
    uninstall() {
      delete window.__rvf;
      delete window.ipc;
    },
  };
}
