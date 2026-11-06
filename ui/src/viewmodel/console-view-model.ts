/**
 * `ConsoleViewModel` — the DOM-free owner of one logical view's
 * binding graph: a `ViewLink` plus the `CommandBinding`s the console
 * strip uses (transport action + seek + one parameter strip).
 *
 * The view model is the only place that wires backend pushes to
 * bindings: receipts land on `binding.ack`, projections arrive via
 * the port's `subscribe` — SolidJS never holds a second truth; it
 * only renders `binding.state`.
 */

import type {
  BindingPort,
  CommandBinding,
  CommandReceipt,
  LinkDetachReason,
} from "@ruviform/ui-binding";
import { ViewLink } from "@ruviform/ui-binding";

import { call } from "../ipc.ts";
import type { NotifyRouter } from "../binding/router.ts";

/** `syn.control.receipt` wire payload. */
interface WireReceiptData {
  view_id?: string;
  target?: string;
  receipt?: CommandReceipt;
}

export interface ConsoleViewModelOptions {
  readonly viewId: string;
  readonly windowId: string;
  readonly port: BindingPort;
  readonly router: NotifyRouter;
}

export class ConsoleViewModel {
  readonly viewId: string;
  readonly windowId: string;
  readonly link: ViewLink;
  /** `transport` target binding — play/pause/resume/stop actions. */
  readonly transport: CommandBinding<TransportView>;
  /** `transport/position` binding — scrub/seek gestures. */
  readonly seek: CommandBinding<TransportView>;
  /** Parameter strip bindings keyed by logical target. */
  private readonly strips = new Map<string, CommandBinding<ParamView>>();
  private readonly router: NotifyRouter;
  private receiptOff: (() => void) | undefined;
  private attachedGeneration = 0;
  private attachToken: object | undefined;

  constructor(options: ConsoleViewModelOptions) {
    this.viewId = options.viewId;
    this.windowId = options.windowId;
    this.router = options.router;
    this.link = new ViewLink({ viewId: options.viewId, port: options.port });
    this.transport = this.link.createBinding<TransportView>("transport");
    this.seek = this.link.createBinding<TransportView>("transport/position");
  }

  private *bindingsFor(target: string | undefined) {
    if (target === "transport") yield this.transport;
    else if (target === "transport/position") yield this.seek;
    const s = target !== undefined ? this.strips.get(target) : undefined;
    if (s !== undefined) yield s;
  }

  /** A parameter strip binding for `target` (created once). */
  strip(target: string): CommandBinding<ParamView> {
    let b = this.strips.get(target);
    if (b === undefined) {
      b = this.link.createBinding<ParamView>(target);
      this.strips.set(target, b);
    }
    return b;
  }

  /** Live backend generation after `attach` (0 before first attach). */
  get generation(): number {
    return this.attachedGeneration;
  }

  /**
   * `syn.view.attach` + link attach — remounts and dock transfers go
   * through the same path: the backend bumps the view epoch and
   * re-pushes subscription truth.
   */
  async attach(): Promise<number> {
    const token = {};
    this.attachToken = token;
    const res = await call<{ generation?: number }>("syn.view.attach", {
      view_id: this.viewId,
      window_id: this.windowId,
    });
    if (this.attachToken !== token) return this.attachedGeneration;
    this.attachedGeneration = res.generation ?? 0;
    this.link.attach(this.windowId);
    this.receiptOff?.();
    this.receiptOff = this.router.on("syn.control.receipt", (data) => {
      const d = data as WireReceiptData;
      if (d.view_id !== this.viewId || d.receipt === undefined) return;
      for (const b of this.bindingsFor(d.target)) b.ack(d.receipt);
    });
    return this.attachedGeneration;
  }

  private releaseAttach(reason: LinkDetachReason): void {
    this.attachToken = undefined;
    this.receiptOff?.();
    this.receiptOff = undefined;
    this.link.detach(reason);
  }

  /** `syn.view.detach` + link detach — dock transfer or unmount. */
  async detach(reason?: "dock-transfer" | "unmount"): Promise<void> {
    this.releaseAttach(reason ?? "unmount");
    await call("syn.view.detach", {
      view_id: this.viewId,
      ...(reason !== undefined ? { reason } : {}),
    });
  }

  /** `syn.view.close` — terminal; drops UI state on both gateways. */
  async close(): Promise<void> {
    this.releaseAttach("explicit");
    await call("syn.view.close", { view_id: this.viewId });
  }
}

/** `transport`/`transport/position` projection payload. */
export interface TransportView {
  status?: string;
  position?: string;
  loop_range?: string | null;
  record_enabled?: boolean;
  recording_active?: boolean;
}

/** `param-*` projection payload. */
export interface ParamView {
  plain?: number;
  normalized?: number;
  read_only?: boolean;
  mode?: string;
  arm?: string;
  observation?: string;
  capability?: boolean;
}
