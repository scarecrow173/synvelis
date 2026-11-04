/**
 * Notify router — fan-out for the `data.channel` discriminated push
 * channels the `syn.*` UI ops emit (`syn.control.receipt`,
 * `syn.control.projection`, `syn.viz.frame`, `syn.viz.event`,
 * `syn.view.state`).
 *
 * One `onEvent` subscription sits behind the router; listeners are
 * keyed by channel so each binding/source hears only its own stream.
 * Listener removal is the returned function — view models tie it to
 * their owner so remounts cannot double-register.
 */

import { onEvent, type RvfEvent } from "../ipc.ts";

export type ChannelListener = (data: unknown) => void;

export interface NotifyRouter {
  /** Registers `cb` for `channel`; returns the unsubscribe function. */
  on(channel: string, cb: ChannelListener): () => void;
  /** Routes one raw event — exposed for tests that bypass `onEvent`. */
  route(event: RvfEvent): void;
  /** Listener count per channel — evidence for leak checks. */
  listenerCount(channel: string): number;
}

export function createRouter(): NotifyRouter {
  const listeners = new Map<string, Set<ChannelListener>>();
  return {
    on(channel, cb) {
      let set = listeners.get(channel);
      if (set === undefined) {
        set = new Set();
        listeners.set(channel, set);
      }
      set.add(cb);
      return () => {
        set.delete(cb);
        if (set.size === 0) listeners.delete(channel);
      };
    },
    route(event) {
      if (event.kind !== "notify") return;
      const body = event.data as { channel?: unknown; data?: unknown };
      if (typeof body?.channel !== "string") return;
      const set = listeners.get(body.channel);
      if (set === undefined) return;
      for (const cb of [...set]) cb(body.data);
    },
    listenerCount(channel) {
      return listeners.get(channel)?.size ?? 0;
    },
  };
}

let singleton: { router: NotifyRouter; off: () => void } | undefined;

/** The app-wide router — wired to `__rvf.onEvent` once, lazily. */
export function notifyRouter(): NotifyRouter {
  if (singleton === undefined) {
    const router = createRouter();
    const off = onEvent((e) => router.route(e));
    singleton = { router, off };
  }
  return singleton.router;
}

/** Test-only: drops the singleton so a fresh bridge re-wires. */
export function resetRouterForTests(): void {
  singleton?.off();
  singleton = undefined;
}
