/**
 * Typed adapter over the injected `window.__rvf` bridge
 * (`ruviform_ui::BRIDGE_JS`). This is the ONLY module that touches the
 * raw bridge — components and the store go through `call`/`onEvent`
 * so the wire contract stays in one place.
 *
 * The bridge itself is framework-agnostic plain JS owned by the host;
 * this file only adds types.
 */

export type OpName =
  | "sys.capabilities"
  | "syn.app.info"
  | "syn.app.capabilities"
  | "syn.app.quit"
  | "syn.project.new"
  | "syn.project.open"
  | "syn.project.save"
  | "syn.project.info"
  | "syn.transport.set"
  | "syn.workspace.mount";

/** One `rvf.ipc.v1` envelope pushed from Rust (non-result kinds). */
export interface RvfEvent {
  v?: number;
  kind?: string;
  seq?: number;
  data?: unknown;
  error?: { code?: string; message?: string };
  [key: string]: unknown;
}

/** The bridge surface the host injects (rvf.ipc.v1). */
export interface RvfBridge {
  version: number;
  call(op: string, params?: Record<string, unknown>): Promise<unknown>;
  onEvent(cb: (event: RvfEvent) => void): () => void;
  dataUrl(name: string, offset?: number, len?: number): string;
  dispatch(m: unknown): void;
}

declare global {
  interface Window {
    __rvf?: RvfBridge;
    ipc?: { postMessage(msg: string): void };
  }
}

/** Bridge-rejected calls carry the wire `code` on the `Error`. */
export interface IpcError extends Error {
  code?: string;
}

/** Whether the host bridge is live (false inside a plain browser/test). */
export function bridgeReady(): boolean {
  return typeof window.__rvf?.call === "function";
}

/**
 * Sends an intent for `op`. Resolves with the result `data` on
 * `ok: true`; rejects with an `IpcError` carrying `.code`.
 */
export async function call<T = unknown>(
  op: OpName,
  params: Record<string, unknown> = {},
): Promise<T> {
  const bridge = window.__rvf;
  if (!bridge) {
    const err: IpcError = new Error(
      "rvf bridge unavailable — the shell only runs inside the Synvelis host",
    );
    err.code = "unavailable";
    throw err;
  }
  return (await bridge.call(op, params)) as T;
}

/**
 * Subscribes to pushed events (notify/job/lifecycle/error). Returns an
 * unsubscribe function; no-op-safe when the bridge is absent.
 */
export function onEvent(cb: (event: RvfEvent) => void): () => void {
  return window.__rvf?.onEvent(cb) ?? (() => {});
}

/** `rvf://data` pull URL for a registered DataSource. */
export function dataUrl(name: string, offset = 0, len?: number): string {
  return window.__rvf?.dataUrl(name, offset, len) ?? "";
}
