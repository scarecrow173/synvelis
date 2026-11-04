/**
 * SV-GUI-02 conformance lane — shared Ruviform RF-GT fixtures.
 *
 * Three lanes over the one versioned corpus (`ui/guitest/fixtures`):
 *
 * 1. Scenario replay: every `scenarios/binding/*.v1.json` drives the
 *    real vendored `InteractionControl`/`CommandBinding`/`QueryBinding`/
 *    `ViewLink` path; its trace must deep-equal the independent
 *    `ContractModel` oracle — recorded responses are never the PASS
 *    criterion.
 * 2. Wire conformance: op names, emitted intents, and the
 *    receipt/projection notify shapes `syn.*` produces are validated
 *    by the corpus's hand-rolled mirrors of the Rust wire contract —
 *    `validOp`, `parseIntent`, `decodeEvent`, `validateReceipt`,
 *    `validateProjection`.
 * 3. Determinism: a second identical run must produce an identical
 *    trace.
 */

import { readdirSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  FIXTURES_DIR,
  checkExpects,
  decodeEvent,
  loadScenarioFile,
  parseIntent,
  runScenario,
  validateProjection,
  validateReceipt,
  validOp,
  type ScenarioDoc,
} from "@ruviform/ui-guitest";

import type { OpName } from "../src/ipc.ts";

const SCENARIO_DIR = join(FIXTURES_DIR, "scenarios", "binding");
const scenarioFiles = readdirSync(SCENARIO_DIR)
  .filter((f) => f.endsWith(".v1.json"))
  .sort();

describe("RF-GT scenario replay vs ContractModel oracle", () => {
  for (const file of scenarioFiles) {
    const rel = `scenarios/binding/${file}`;
    const loaded = loadScenarioFile(rel);
    const doc = loaded.doc as unknown as ScenarioDoc;

    it(`${loaded.id}: trace deep-equals oracle + expect pins + determinism`, async () => {
      const { actual, expected } = await runScenario(doc);
      expect(actual).toEqual(expected);

      const firstControl = doc.setup.targets[0]?.control ?? "";
      const fails = checkExpects(actual, doc.expect ?? {}, firstControl);
      expect(fails).toEqual([]);

      const again = await runScenario(doc);
      expect(again.actual).toEqual(actual);
    });
  }
});

describe("RF-GT wire conformance of the syn.* surface", () => {
  it("every registered syn.* op name satisfies validOp", () => {
    const ops: OpName[] = [
      "syn.view.attach",
      "syn.view.detach",
      "syn.view.close",
      "syn.view.list",
      "syn.control.dispatch",
      "syn.control.subscribe",
      "syn.control.unsubscribe",
      "syn.control.targets",
      "syn.viz.bind",
      "syn.viz.meta",
      "syn.viz.query",
      "syn.viz.viewport",
      "syn.viz.select",
      "syn.viz.snapshot",
      "syn.viz.register",
      "syn.viz.push_frame",
      "syn.viz.source_changed",
    ];
    for (const op of ops) expect(validOp(op), op).toBe(true);
  });

  it("a syn.control.dispatch intent parses under parseIntent", () => {
    const wire = JSON.stringify({
      v: 1,
      kind: "intent",
      id: "i-1",
      op: "syn.control.dispatch",
      params: {
        intent: {
          kind: "action",
          target: "transport",
          controlId: "transport",
          sequence: 1,
          action: "play",
        },
        ctx: {
          commandId: "cmd-1",
          viewId: "v1",
          windowId: "w0",
          generation: 1,
          issuedAt: 42,
        },
      },
    });
    const parsed = parseIntent(wire);
    expect(parsed.ok).toBe(true);
    if (parsed.ok) {
      expect(parsed.intent.op).toBe("syn.control.dispatch");
    }
  });

  it("syn.control.receipt/projection notifies decode + validate", () => {
    // The exact envelope shape `push_notify` emits in ui_ops.rs.
    const receiptEvent = {
      v: 1,
      kind: "notify",
      seq: 7,
      data: {
        channel: "syn.control.receipt",
        data: {
          view_id: "v1",
          target: "param-gain",
          receipt: { commandId: "cmd-1", status: "applied", revision: 9 },
        },
      },
    };
    const dec = decodeEvent(receiptEvent);
    expect(dec.ok).toBe(true);
    const receipt = validateReceipt(
      (receiptEvent.data.data as { receipt: unknown }).receipt,
    );
    expect(receipt.ok).toBe(true);

    const projEvent = {
      v: 1,
      kind: "notify",
      seq: 8,
      data: {
        channel: "syn.control.projection",
        data: {
          view_id: "v1",
          target: "param-gain",
          row: {
            kind: "param",
            observed: {
              revision: 9,
              value: { plain: 0.5, normalized: 0.5 },
            },
          },
        },
      },
    };
    expect(decodeEvent(projEvent).ok).toBe(true);
    const proj = validateProjection(
      (
        projEvent.data.data.row as {
          observed: unknown;
        }
      ).observed,
    );
    expect(proj.ok).toBe(true);
  });
});
