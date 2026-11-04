/**
 * SolidJS ↔ Ruviform Custom Elements adapter.
 *
 * Ruviform elements are plain-TypeScript Custom Elements with typed
 * DOM properties + CustomEvents — they are *not* attributes. The
 * integration contract (SV-GUI-01):
 *
 * - register once via `registerElements` (idempotent upstream);
 * - get element refs through Solid `ref` callbacks, never selectors;
 * - write object/boolean state through `setProp` (real property
 *   assignment — never JSON-serialized into attributes);
 * - subscribe element CustomEvents through `onRvfEvent`, which ties
 *   listener lifetime to the enclosing owner (mount/unmount cleanup,
 *   safe reconnect/remount).
 */

import { onCleanup } from "solid-js";
import type { JSX } from "solid-js";

import { registerRvfElements } from "@ruviform/ui-components";
import type { RvfTransport } from "@ruviform/ui-components";

/** Registers every `rvf-*` element (idempotent). */
export function registerElements(): void {
  registerRvfElements();
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

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      /**
       * `<rvf-transport>` — display state + label come in as JSX
       * attributes; `capabilities` is a typed property set via `setProp`
       * on the element ref (it cannot ride an attribute).
       */
      "rvf-transport": JSX.HTMLAttributes<RvfTransport> & {
        state?: string;
        label?: string;
      };
    }
  }
}
