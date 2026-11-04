/**
 * Shell state — a SolidJS store projection of host-reported state.
 *
 * This is a *projection*, not a second source of truth: Project state,
 * capabilities, workspace, and revisions all originate from `syn.*`
 * op results and `notify` pushes; the store only mirrors the latest
 * values for rendering. Mutations always go through ops — the store
 * never writes Project/Graph state directly.
 */

import { createStore } from "solid-js/store";

import type { RvfEvent } from "./ipc.ts";

export interface ProjectState {
  name: string;
  projectId: string;
  revision: number;
  saved: boolean;
  store: boolean;
}

export interface BackendState {
  name: string;
  state: string;
  version?: string;
  notes: string[];
}

export interface CapabilityEntry {
  state: string;
  [key: string]: unknown;
}

export interface AppState {
  /** `syn.app.info` — revisions + backend probe + counters. */
  app: {
    name: string;
    version: string;
    hostRevision: string;
    ipc: number;
  } | null;
  backend: BackendState | null;
  /** `syn.app.capabilities` — unavailable areas are kept, not hidden. */
  capabilities: Record<string, CapabilityEntry> | null;
  project: ProjectState | null;
  workspace: { shellWindow: number; mounted: boolean } | null;
  /** Ops advertised by `sys.capabilities`. */
  ops: string[];
  transportPlaying: boolean;
  /** Bridge presence + hydration status. */
  connected: boolean;
  startupError: string | null;
  lastError: { code: string; message: string } | null;
}

function emptyState(): AppState {
  return {
    app: null,
    backend: null,
    capabilities: null,
    project: null,
    workspace: null,
    ops: [],
    transportPlaying: false,
    connected: false,
    startupError: null,
    lastError: null,
  };
}

function asRecord(v: unknown): Record<string, unknown> {
  return typeof v === "object" && v !== null ? (v as Record<string, unknown>) : {};
}

function asString(v: unknown, fallback = ""): string {
  return typeof v === "string" ? v : fallback;
}

/** Maps a `syn.project.*`/`syn.workspace.mount` payload onto the store. */
export function projectFromPayload(payload: unknown): ProjectState | null {
  const p = asRecord(payload);
  if (typeof p.name !== "string" || typeof p.project_id !== "string") return null;
  return {
    name: p.name,
    projectId: p.project_id,
    revision: typeof p.revision === "number" ? p.revision : 0,
    saved: p.saved === true,
    store: p.store === true,
  };
}

function backendFromPayload(payload: unknown): BackendState | null {
  const b = asRecord(asRecord(payload).backend);
  if (typeof b.name !== "string") return null;
  return {
    name: b.name,
    state: asString(b.state, "Unknown"),
    ...(typeof b.version === "string" ? { version: b.version } : {}),
    notes: Array.isArray(b.notes) ? b.notes.filter((n): n is string => typeof n === "string") : [],
  };
}

/**
 * The store factory — tests create isolated instances; the app uses
 * the module-level `appStore`.
 */
export function createAppStore() {
  const [state, setState] = createStore<AppState>(emptyState());

  return {
    state,

    /** Applies the `syn.app.info` payload. */
    applyAppInfo(info: unknown): void {
      const i = asRecord(info);
      setState("app", {
        name: asString(i.name, "synvelis"),
        version: asString(i.version, "?"),
        hostRevision: asString(i.host_revision, "?"),
        ipc: typeof i.ipc === "number" ? i.ipc : 0,
      });
      const backend = backendFromPayload(i);
      if (backend) setState("backend", backend);
    },

    /** Applies the `syn.app.capabilities` payload verbatim. */
    applyCapabilities(caps: unknown): void {
      const c = asRecord(caps);
      setState("capabilities", c as Record<string, CapabilityEntry>);
    },

    /** Applies a `syn.project.*`/`syn.workspace.mount` payload. */
    applyProject(project: unknown): void {
      const p = projectFromPayload(project);
      if (p) setState("project", p);
    },

    applyWorkspace(mount: unknown): void {
      const m = asRecord(mount);
      setState("workspace", {
        shellWindow: typeof m.shell_window === "number" ? m.shell_window : 0,
        mounted: m.mounted === true,
      });
    },

    /** Applies `sys.capabilities` — op names for the ops list. */
    applySysCapabilities(caps: unknown): void {
      const c = asRecord(caps);
      const ops = Array.isArray(c.ops) ? c.ops : [];
      setState(
        "ops",
        ops
          .map((o) => asRecord(o).name)
          .filter((n): n is string => typeof n === "string"),
      );
    },

    setTransportPlaying(playing: boolean): void {
      setState("transportPlaying", playing);
    },

    setConnected(connected: boolean): void {
      setState("connected", connected);
    },

    setStartupError(message: string): void {
      setState("startupError", message);
    },

    /** Normalizes a rejected `call`/event into `lastError`. */
    recordError(e: unknown): void {
      const err = e as { code?: unknown; message?: unknown };
      setState("lastError", {
        code: typeof err?.code === "string" ? err.code : "service_error",
        message: typeof err?.message === "string" ? err.message : String(e),
      });
    },

    clearError(): void {
      setState("lastError", null);
    },

    /**
     * Routes one pushed `RvfEvent` (`notify`/`lifecycle`/`error`) into
     * the store. `notify` carries the app-state push the bin emits on
     * `Ready` (`{project, capabilities}`).
     */
    applyEvent(ev: RvfEvent): void {
      switch (ev.kind) {
        case "notify": {
          const data = asRecord(ev.data);
          const project = projectFromPayload(data.project);
          if (project) setState("project", project);
          const caps = asRecord(data.capabilities);
          if (Object.keys(caps).length > 0) {
            setState("capabilities", caps as Record<string, CapabilityEntry>);
          }
          break;
        }
        case "lifecycle": {
          const s = asRecord(ev.data ?? ev);
          if (asString(s.state ?? ev.state) === "closing") setState("connected", false);
          break;
        }
        case "error":
          this.recordError(ev.error ?? { code: "service_error", message: "unknown push error" });
          break;
        default:
          break;
      }
    },
  };
}

export type AppStore = ReturnType<typeof createAppStore>;

/** The app-wide store instance. */
export const appStore = createAppStore();
