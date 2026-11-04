/**
 * Viz element sources over `syn.viz.*` — `WaveformDataSource` and
 * `MeterDataSource`/`SpectrumDataSource` implementations the
 * `<rvf-waveform>`/`<rvf-meter>` elements consume through their typed
 * `source` property.
 *
 * Bounded by contract: `query` forwards the element's `buckets` LOD
 * budget and the backend enforces its own byte bound; meter/spectrum
 * are latest-complete-frame pulls fed by `syn.viz.frame` pushes.
 */

import type {
  MeterDataSource,
  MeterFrame,
  MeterInfo,
  SourceMeta,
  SpectrumDataSource,
  SpectrumFrame,
  SpectrumInfo,
  WaveformDataSource,
  WaveformInfo,
  WaveformQuery,
  WaveformView,
} from "@ruviform/ui-viz";

import { call } from "../ipc.ts";
import type { NotifyRouter } from "../binding/router.ts";

interface WireVizEventData {
  event?: {
    Viewport?: { view_id?: string; surface_id?: string };
    Select?: { view_id?: string };
  };
}

interface WireVizFrame {
  view_id?: string;
  surface_id?: string;
  kind?: string;
  frame?: unknown;
}

interface VizSourceOptions {
  readonly viewId: string;
  readonly surfaceId: string;
  readonly sourceId: string;
  readonly router: NotifyRouter;
}

export class IpcWaveformSource implements WaveformDataSource {
  private metaValue: (WaveformInfo & SourceMeta) | undefined;
  private readonly changeListeners = new Set<() => void>();
  private readonly eventOff: () => void;
  private readonly o: VizSourceOptions;

  constructor(options: VizSourceOptions) {
    this.o = options;
    this.eventOff = options.router.on("syn.viz.event", (data) => {
      const d = data as WireVizEventData;
      const vp = d.event?.Viewport;
      if (vp?.view_id === options.viewId && vp.surface_id === options.surfaceId) {
        for (const cb of [...this.changeListeners]) cb();
      }
    });
  }

  /** `syn.viz.meta` hydrate — call once on bind. */
  async refresh(): Promise<void> {
    const meta = await call<{
      kind?: string;
      meta?: WaveformInfo & SourceMeta;
    }>("syn.viz.meta", {
      view_id: this.o.viewId,
      surface_id: this.o.surfaceId,
    }).catch(() => undefined);
    if (meta?.meta !== undefined && meta.meta !== null) {
      this.metaValue = meta.meta;
    }
  }

  meta(): WaveformInfo & SourceMeta {
    return (
      this.metaValue ?? {
        id: this.o.sourceId,
        revision: "unknown",
        axis: "seconds",
        extent: 0,
        channels: 0,
      }
    );
  }

  async query(q: WaveformQuery, signal?: AbortSignal): Promise<WaveformView> {
    if (signal?.aborted) throw new Error("aborted");
    const res = await call<{
      view?: WaveformView;
      stale?: boolean;
    }>("syn.viz.query", {
      view_id: this.o.viewId,
      surface_id: this.o.surfaceId,
      start: q.start,
      end: q.end,
      buckets: q.buckets,
      ...(q.channels !== undefined ? { channels: q.channels } : {}),
    });
    if (signal?.aborted) throw new Error("aborted");
    if (res.stale === true) throw new Error("stale");
    if (res.view === undefined) throw new Error("no view");
    return res.view;
  }

  subscribe(onChange: () => void): () => void {
    this.changeListeners.add(onChange);
    return () => this.changeListeners.delete(onChange);
  }

  dispose(): void {
    this.eventOff();
    this.changeListeners.clear();
  }
}

export class IpcMeterSource implements MeterDataSource {
  private latest: MeterFrame | null = null;
  private metaValue: (SourceMeta & MeterInfo) | undefined;
  private readonly updateListeners = new Set<() => void>();
  private readonly frameOff: () => void;
  private readonly o: VizSourceOptions;

  constructor(options: VizSourceOptions) {
    this.o = options;
    this.frameOff = options.router.on("syn.viz.frame", (data) => {
      const d = data as WireVizFrame;
      if (
        d.view_id === options.viewId &&
        d.surface_id === options.surfaceId &&
        d.kind === "meter"
      ) {
        this.latest = d.frame as MeterFrame;
        for (const cb of [...this.updateListeners]) cb();
      }
    });
  }

  async refresh(): Promise<void> {
    const meta = await call<{
      kind?: string;
      meta?: SourceMeta & MeterInfo;
    }>("syn.viz.meta", {
      view_id: this.o.viewId,
      surface_id: this.o.surfaceId,
    }).catch(() => undefined);
    if (meta?.meta !== undefined && meta.meta !== null) {
      this.metaValue = meta.meta;
    }
    const snap = await call<{ frame?: MeterFrame | null }>(
      "syn.viz.snapshot",
      { view_id: this.o.viewId, surface_id: this.o.surfaceId },
    ).catch(() => undefined);
    if (snap?.frame != null) this.latest = snap.frame;
  }

  meta(): SourceMeta & MeterInfo {
    return (
      this.metaValue ?? {
        id: this.o.sourceId,
        revision: "unknown",
        channels: 0,
      }
    );
  }

  snapshot(): MeterFrame | null {
    return this.latest;
  }

  subscribe(onUpdate: () => void): () => void {
    this.updateListeners.add(onUpdate);
    return () => this.updateListeners.delete(onUpdate);
  }

  dispose(): void {
    this.frameOff();
    this.updateListeners.clear();
  }
}

export class IpcSpectrumSource implements SpectrumDataSource {
  private latest: SpectrumFrame | null = null;
  private metaValue: (SourceMeta & SpectrumInfo) | undefined;
  private readonly updateListeners = new Set<() => void>();
  private readonly frameOff: () => void;
  private readonly o: VizSourceOptions;

  constructor(options: VizSourceOptions) {
    this.o = options;
    this.frameOff = options.router.on("syn.viz.frame", (data) => {
      const d = data as WireVizFrame;
      if (
        d.view_id === options.viewId &&
        d.surface_id === options.surfaceId &&
        d.kind === "spectrum"
      ) {
        this.latest = d.frame as SpectrumFrame;
        for (const cb of [...this.updateListeners]) cb();
      }
    });
  }

  async refresh(): Promise<void> {
    const meta = await call<{
      kind?: string;
      meta?: SourceMeta & SpectrumInfo;
    }>("syn.viz.meta", {
      view_id: this.o.viewId,
      surface_id: this.o.surfaceId,
    }).catch(() => undefined);
    if (meta?.meta !== undefined && meta.meta !== null) {
      this.metaValue = meta.meta;
    }
  }

  meta(): SourceMeta & SpectrumInfo {
    return (
      this.metaValue ?? {
        id: this.o.sourceId,
        revision: "unknown",
        bins: 0,
        axis: { kind: "uniform", firstHz: 0, stepHz: 0 },
      }
    );
  }

  snapshot(): SpectrumFrame | null {
    return this.latest;
  }

  subscribe(onUpdate: () => void): () => void {
    this.updateListeners.add(onUpdate);
    return () => this.updateListeners.delete(onUpdate);
  }

  dispose(): void {
    this.frameOff();
    this.updateListeners.clear();
  }
}
