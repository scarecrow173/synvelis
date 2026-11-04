/**
 * SolidJS ↔ Ruviform Custom Elements adapter.
 *
 * Ruviform elements are plain-TypeScript Custom Elements with typed
 * DOM properties + CustomEvents — they are *not* attributes. The
 * integration contract (SV-GUI-01 / SV-GUI-02):
 *
 * - register once via `registerElements` (idempotent upstream) —
 *   controls + viz element sets together;
 * - get element refs through Solid `ref` callbacks, never selectors;
 * - write object/boolean state through `setProp` (real property
 *   assignment — never JSON-serialized into attributes);
 * - subscribe element CustomEvents through `onRvfEvent`, which ties
 *   listener lifetime to the enclosing owner (mount/unmount cleanup,
 *   safe reconnect/remount);
 * - bind intent/reflection via upstream `bindControlElement`,
 *   `bindTransportElement`, `bindViewIntents` — never by re-implementing
 *   gesture → intent mapping per view.
 */

import { onCleanup } from "solid-js";
import type { JSX } from "solid-js";

import { registerRvfElements } from "@ruviform/ui-components";
import type {
  RvfFader,
  RvfKnob,
  RvfSeekbar,
  RvfTransport,
} from "@ruviform/ui-components";
import { registerRvfVizElements } from "@ruviform/ui-viz";
import type { RvfMeter, RvfWaveform } from "@ruviform/ui-viz";

/** Registers every `rvf-*` element — controls + viz (idempotent). */
export function registerElements(): void {
  registerRvfElements();
  registerRvfVizElements();
}

/** Assigns a typed element property (not an attribute). */
export function setProp<El, K extends keyof El>(el: El, key: K, value: El[K]): void {
  el[key] = value;
}

/** Adds `handler` for `type`, removed automatically on owner cleanup. */
export function onRvfEvent<E extends Event>(
  el: EventTarget,
  type: string,
  handler: (event: E) => void,
): void {
  const listener = (e: Event) => handler(e as E);
  el.addEventListener(type, listener);
  onCleanup(() => el.removeEventListener(type, listener));
}

/** Detail shape of the `rvf-transport` CustomEvent. */
export interface TransportIntent {
  action: "play" | "pause" | "resume" | "stop";
  controlId: string;
  sequence: number;
}

/**
 * Shared attribute-only JSX contract for `rvf-*` elements. Object /
 * typed state (`descriptor`, `ports`, `source`, `overlays`,
 * `capabilities`, `appearance`…) is assigned through `setProp` on the
 * element ref — attributes only carry strings/booleans.
 */
interface RvfAttributes<El> extends JSX.HTMLAttributes<El> {
  label?: string;
  "control-id"?: string;
  disabled?: boolean;
}

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      /**
       * `<rvf-transport>` — display state + label come in as JSX
       * attributes; `capabilities` is a typed property set via `setProp`
       * on the element ref (it cannot ride an attribute).
       */
      "rvf-transport": RvfAttributes<RvfTransport> & {
        state?: string;
      };
      /** `<rvf-knob>` — rotary value control. */
      "rvf-knob": RvfAttributes<RvfKnob>;
      /** `<rvf-fader>` — linear value control. */
      "rvf-fader": RvfAttributes<RvfFader> & {
        orientation?: string;
      };
      /** `<rvf-seekbar>` — position scrub control. */
      "rvf-seekbar": RvfAttributes<RvfSeekbar>;
      /**
       * `<rvf-waveform>` — `source`, `viewport`, `overlays`, `range`,
       * `channelLabels` are typed properties via `setProp`.
       */
      "rvf-waveform": RvfAttributes<RvfWaveform> & {
        backend?: string;
      };
      /** `<rvf-meter>` — `source` is a typed property via `setProp`. */
      "rvf-meter": RvfAttributes<RvfMeter> & {
        backend?: string;
        orientation?: string;
      };
    }
  }
}
