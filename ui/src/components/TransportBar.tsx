/**
 * Transport bar — the `<rvf-transport>` Custom Element integration.
 * This is the reference for how SolidJS drives Ruviform elements:
 * the ref is a signal, typed properties (`state`, `capabilities`)
 * are written by effects, and the `rvf-transport` CustomEvent is
 * translated into a `syn.transport.set` intent. SV-GUI-02's binding
 * layer plugs in here (the intent mapping is the connection point).
 */

import { createEffect, createSignal, onMount } from "solid-js";

import type { RvfTransport } from "@ruviform/ui-components";

import { onRvfEvent, setProp, type TransportIntent } from "../adapters/ruviform.ts";
import { call } from "../ipc.ts";
import { appStore } from "../store.ts";

export function TransportBar() {
  const [el, setEl] = createSignal<RvfTransport>();

  // Typed-property writes — `state` mirrors the store projection, and
  // `capabilities` disables resume (the service only exposes play/stop
  // in SV-GUI-01) without mutating the element's model.
  createEffect(() => {
    const t = el();
    if (!t) return;
    setProp(t, "state", appStore.state.transportPlaying ? "playing" : "stopped");
    setProp(t, "capabilities", { play: true, pause: true, resume: false, stop: true });
  });

  onMount(() => {
    const t = el();
    if (!t) return;
    onRvfEvent<CustomEvent<TransportIntent>>(t, "rvf-transport", (e) => {
      const playing = e.detail.action === "play" || e.detail.action === "resume";
      void call("syn.transport.set", { playing })
        .then(() => appStore.setTransportPlaying(playing))
        .catch((err: unknown) => appStore.recordError(err));
    });
  });

  return (
    <section class="panel transport-bar" aria-label="Transport">
      <h2>Transport</h2>
      <rvf-transport ref={setEl} label="Transport" state="stopped" />
    </section>
  );
}
