import { afterEach, describe, expect, it } from "vitest";

import { bridgeReady, call, dataUrl, onEvent } from "../src/ipc.ts";
import { installFakeHost, type FakeHost } from "./fake-host.ts";

describe("ipc adapter", () => {
  let host: FakeHost | undefined;
  afterEach(() => {
    host?.uninstall();
    host = undefined;
  });

  it("call posts an rvf.ipc.v1 intent and resolves on result", async () => {
    host = installFakeHost();
    const pending = call<{ ok: boolean }>("syn.project.info", { verbose: true });
    expect(host.sent).toHaveLength(1);
    const intent = host.sent[0]!;
    expect(intent).toMatchObject({
      v: 1,
      kind: "intent",
      op: "syn.project.info",
      params: { verbose: true },
    });
    expect(intent.id).toMatch(/^i-/);
    host.answer(intent.id, { ok: true });
    await expect(pending).resolves.toEqual({ ok: true });
  });

  it("call rejects with the wire error code", async () => {
    host = installFakeHost();
    const pending = call("syn.project.save");
    const intent = host.sent[0]!;
    host.answerErr(intent.id, "service_error", "no store");
    await expect(pending).rejects.toMatchObject({
      code: "service_error",
      message: "no store",
    });
  });

  it("onEvent receives pushed events but not intent results", async () => {
    host = installFakeHost();
    const events: unknown[] = [];
    const off = onEvent((e) => events.push(e));
    const pending = call("syn.app.info");
    host.answer(host.sent[0]!.id, {});
    await pending;
    host.emit({ v: 1, kind: "notify", seq: 1, data: { project: null } });
    expect(events).toHaveLength(1);
    expect((events[0] as { kind: string }).kind).toBe("notify");
    off();
    host.emit({ v: 1, kind: "notify", seq: 2, data: {} });
    expect(events).toHaveLength(1);
  });

  it("dataUrl builds the rvf://data pull URL", () => {
    host = installFakeHost();
    expect(dataUrl("meters/track-1", 128, 512)).toBe(
      "rvf://data/meters/track-1?offset=128&len=512",
    );
  });

  it("call without a bridge rejects 'unavailable'", async () => {
    expect(bridgeReady()).toBe(false);
    await expect(call("syn.app.info")).rejects.toMatchObject({
      code: "unavailable",
    });
  });
});
