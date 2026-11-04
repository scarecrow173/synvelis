/**
 * `ConsoleView` — the SV-GUI-02 reference product view: transport,
 * seek, one knob + fader parameter strip, waveform, meter.
 *
 * Everything user-visible flows through the shared binding surface:
 * `bindControlElement`/`bindTransportElement` carry `rvf-gesture` /
 * `rvf-transport` intents into `CommandBinding`s and reflect state
 * back (`pending`, `invalid`, `unsupported`, controlled `value`).
 * Viz elements receive their `source` as a typed property and report
 * viewport/select through `bindViewIntents`. On cleanup every unbind
 * runs — no stacked listeners or leaked subscriptions on remount.
 */

import { onCleanup, onMount } from "solid-js";

import {
  bindControlElement,
  bindTransportElement,
  bindViewIntents,
} from "@ruviform/ui-binding";
import type { RvfFader, RvfKnob, RvfSeekbar, RvfTransport } from "@ruviform/ui-components";
import type { RvfMeter, RvfWaveform } from "@ruviform/ui-viz";

import { notifyRouter } from "../binding/router.ts";
import { createIpcBindingPort } from "../binding/port.ts";
import { setProp } from "../adapters/ruviform.ts";
import { call } from "../ipc.ts";
import { ConsoleViewModel } from "../viewmodel/console-view-model.ts";
import { IpcMeterSource, IpcWaveformSource } from "../viewmodel/viz-source.ts";

/** One logical console view — `viewId`/`windowId` come from the host. */
export function ConsoleView(props: { viewId: string; windowId: string }) {
  const router = notifyRouter();
  const port = createIpcBindingPort({ router });
  const vm = new ConsoleViewModel({
    viewId: props.viewId,
    windowId: props.windowId,
    port,
    router,
  });

  let transportEl!: RvfTransport;
  let seekEl!: RvfSeekbar;
  let knobEl!: RvfKnob;
  let faderEl!: RvfFader;
  let waveEl!: RvfWaveform;
  let meterEl!: RvfMeter;

  const waveSource = new IpcWaveformSource({
    viewId: props.viewId,
    surfaceId: "wave",
    sourceId: "wave",
    router,
  });
  const meterSource = new IpcMeterSource({
    viewId: props.viewId,
    surfaceId: "meter",
    sourceId: "meter",
    router,
  });

  onMount(async () => {
    await vm.attach();

    // Controls — gesture → intent → receipt → projection → element.
    const unbinds = [
      bindTransportElement(transportEl, vm.transport),
      bindControlElement(seekEl, vm.seek),
      bindControlElement(knobEl, vm.strip("param-gain")),
      bindControlElement(faderEl, vm.strip("param-fader")),
    ];
    onCleanup(() => {
      for (const u of unbinds) u();
    });

    // Viz — `source` is a typed property, never an attribute.
    setProp(waveEl, "source", waveSource);
    setProp(meterEl, "source", meterSource);
    await call("syn.viz.bind", {
      view_id: props.viewId,
      surface_id: "wave",
      kind: "waveform",
      source: "wave",
    }).catch(() => {});
    await call("syn.viz.bind", {
      view_id: props.viewId,
      surface_id: "meter",
      kind: "meter",
      source: "meter",
    }).catch(() => {});
    void waveSource.refresh();
    void meterSource.refresh();

    // View-state intents (rvf-viewport / rvf-select) forward to the
    // bounded syn.viz.* surface ops — viewport truth stays Rust-side.
    const unbindViewIntents = bindViewIntents(waveEl, {
      onViewport: (detail) => {
        const d = detail as
          | { viewport?: { start?: number; end?: number } }
          | undefined;
        const vp = d?.viewport;
        if (typeof vp?.start !== "number" || typeof vp.end !== "number") return;
        void call("syn.viz.viewport", {
          view_id: props.viewId,
          surface_id: "wave",
          start: vp.start,
          end: vp.end,
        }).catch(() => {});
      },
      onSelect: (detail) => {
        const d = detail as { start?: number; end?: number } | undefined;
        if (typeof d?.start !== "number" || typeof d.end !== "number") return;
        void call("syn.viz.select", {
          view_id: props.viewId,
          surface_id: "wave",
          start: d.start,
          end: d.end,
        }).catch(() => {});
      },
    });
    onCleanup(unbindViewIntents);

    onCleanup(() => {
      waveSource.dispose();
      meterSource.dispose();
      void vm.detach("unmount");
    });
  });

  return (
    <section class="console-view" data-view={props.viewId}>
      <rvf-transport ref={transportEl} label="Transport" />
      <rvf-seekbar ref={seekEl} label="Position" />
      <rvf-knob ref={knobEl} label="Gain" />
      <rvf-fader ref={faderEl} label="Fader" />
      <rvf-waveform ref={waveEl} label="Waveform" />
      <rvf-meter ref={meterEl} label="Meter" />
    </section>
  );
}
