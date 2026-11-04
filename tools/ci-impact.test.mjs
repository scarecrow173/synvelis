// node --test tools/ci-impact.test.mjs
// Issue #148: unit tests for the impact classifier and the fail-closed
// evidence gate. These pin the contract invariants: no under-verification,
// skipped is never Pass, and unknown inputs fall back to full CI.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { buildImpactReport, evaluateGate } from "./ci-impact.mjs";

const ALL_LANES = [
  "verify", "verify", "verify",
  "rt-verify",
  "gw10-verify", "gw10-verify", "gw10-verify",
  "msrv-verify",
  "ui-verify", "ui-verify", "ui-verify",
  "dep-guard", "classify",
];

function requiredLanes(report) {
  return report.lanes.filter((l) => l.expectation === "required").map((l) => l.lane).sort();
}

function notApplicable(report, lane) {
  return report.lanes.filter((l) => l.lane === lane && l.expectation === "not-applicable");
}

const job = (name, conclusion = "success", attempt = 1) => ({
  name,
  status: "completed",
  conclusion,
  run_attempt: attempt,
  started_at: "2026-11-04T00:00:00Z",
  completed_at: "2026-11-04T00:05:00Z",
});

const allPassJobs = (skip = []) =>
  ["verify (ubuntu-latest)", "verify (windows-latest)", "verify (macos-latest)",
    "rt-verify",
    "gw10-verify (ubuntu-latest)", "gw10-verify (windows-latest)", "gw10-verify (macos-latest)",
    "msrv-verify",
    "ui-verify (ubuntu-latest)", "ui-verify (windows-latest)", "ui-verify (macos-latest)",
    "dep-guard", "classify"]
    .map((n) => job(n, skip.includes(n) ? "skipped" : "success"))
    .concat([{ name: "evidence", status: "in_progress", conclusion: null, run_attempt: 1 }]);

function reportFor(files, mode = "shadow") {
  return buildImpactReport({ files, mode, base: { sha: "b" }, head: { sha: "h" }, event: "pull_request" });
}

describe("classify: path rules and classes", () => {
  it("docs-only change requires only classify", () => {
    const r = reportFor(["docs/designs/foo.md", "README.md", "assets/branding/logo.svg"]);
    assert.deepEqual(requiredLanes(r), ["classify"]);
    assert.equal(r.fallback_full_ci, false);
    assert.equal(r.lanes.find((l) => l.lane === "verify").expectation, "not-applicable");
  });

  it("recording crate change requires verify+msrv+rt but not gw10/ui", () => {
    const r = reportFor(["crates/synvelis-recording/src/journal.rs"]);
    assert.deepEqual(requiredLanes(r), ["classify", "dep-guard", "msrv-verify", "rt-verify", "verify", "verify", "verify"]);
    assert.ok(notApplicable(r, "gw10-verify").length === 3);
    assert.ok(notApplicable(r, "ui-verify").length === 3);
    assert.deepEqual(r.crates.affected.sort(), ["recording-poc", "synvelis-player", "synvelis-recording", "synvelis-render"].sort());
  });

  it("project change propagates to gw10 (gateway dep) and rt", () => {
    const r = reportFor(["crates/synvelis-project/src/store.rs"]);
    assert.ok(requiredLanes(r).includes("gw10-verify"));
    assert.ok(requiredLanes(r).includes("rt-verify"));
    assert.ok(notApplicable(r, "ui-verify").length === 3);
  });

  it("gateway change requires gw10 but not rt (dep direction)", () => {
    const r = reportFor(["crates/synvelis-gateway/src/gateway.rs"]);
    assert.ok(requiredLanes(r).includes("gw10-verify"));
    assert.ok(!requiredLanes(r).includes("rt-verify"));
    assert.deepEqual(r.crates.affected.sort(), ["mcp-poc", "synvelis-app", "synvelis-gateway", "synvelis-workspace"].sort());
  });

  it("mcp-poc requires gw10 (transport suite)", () => {
    const r = reportFor(["crates/mcp-poc/src/main.rs"]);
    assert.ok(requiredLanes(r).includes("gw10-verify"));
  });

  it("ui/ change requires ui-verify only (not rust lanes)", () => {
    const r = reportFor(["ui/src/binding.ts", "ui/package.json"]);
    const req = requiredLanes(r);
    assert.ok(req.includes("ui-verify"));
    assert.ok(!req.includes("verify"));
    assert.ok(!req.includes("rt-verify"));
    assert.ok(!req.includes("msrv-verify"));
    assert.ok(!req.includes("gw10-verify"));
  });

  it("app change requires verify+msrv, no rt/gw10/ui", () => {
    const r = reportFor(["crates/synvelis-app/src/service.rs"]);
    const req = requiredLanes(r);
    assert.ok(req.includes("verify") && req.includes("msrv-verify"));
    assert.ok(!req.includes("rt-verify") && !req.includes("gw10-verify") && !req.includes("ui-verify"));
  });
});

describe("classify: trigger classes and fallback", () => {
  for (const f of ["Cargo.lock", "Cargo.toml", "crates/synvelis-player/Cargo.toml"]) {
    it(`${f} triggers full CI`, () => {
      const r = reportFor([f]);
      assert.equal(r.fallback_full_ci, true);
      assert.deepEqual(requiredLanes(r), ALL_LANES.slice().sort());
    });
  }
  for (const f of ["mise.toml", "mise.lock", ".github/workflows/ci.yml", "tools/ci-impact.mjs", ".pre-commit-config.yaml"]) {
    it(`${f} triggers full CI`, () => {
      const r = reportFor([f]);
      assert.equal(r.fallback_full_ci, true);
      assert.deepEqual(requiredLanes(r), ALL_LANES.slice().sort());
    });
  }
  it("unmatched path falls back to full CI", () => {
    const r = reportFor(["build.gradle.kts"]);
    assert.equal(r.fallback_full_ci, true);
    assert.deepEqual(requiredLanes(r), ALL_LANES.slice().sort());
  });
  it("full mode requires all lanes even for docs", () => {
    const r = reportFor(["docs/x.md"], "full");
    assert.deepEqual(requiredLanes(r), ALL_LANES.slice().sort());
  });
});

describe("classify: selective scheduling (Issue #157)", () => {
  const scheduledLegs = (r) => r.lanes.filter((l) => l.scheduled === true).map((l) => l.name).sort();

  it("selective schedules only required legs (ui change)", () => {
    const r = reportFor(["ui/src/x.ts"], "selective");
    assert.deepEqual(scheduledLegs(r), [
      "classify", "dep-guard",
      "ui-verify (macos-latest)", "ui-verify (ubuntu-latest)", "ui-verify (windows-latest)",
    ].sort());
  });

  it("selective docs-only schedules only classify", () => {
    const r = reportFor(["docs/x.md", "README.md"], "selective");
    assert.deepEqual(scheduledLegs(r), ["classify"]);
  });

  it("selective keeps RT/GW-10/MSRV/verify scheduled for owner-affecting changes", () => {
    const r = reportFor(["crates/synvelis-project/src/store.rs"], "selective");
    const legs = scheduledLegs(r);
    for (const n of ["verify (ubuntu-latest)", "verify (windows-latest)", "verify (macos-latest)",
      "rt-verify", "msrv-verify", "dep-guard", "classify"]) {
      assert.ok(legs.includes(n), n);
    }
    // synvelis-project propagates to gateway -> gw10 required too.
    assert.ok(legs.includes("gw10-verify (ubuntu-latest)"));
    assert.ok(!legs.includes("ui-verify (ubuntu-latest)"));
  });

  it("selective trigger/fallback schedules every leg", () => {
    for (const f of ["Cargo.toml", ".github/workflows/ci.yml", "tools/ci-impact.mjs", "mise.toml"]) {
      const r = reportFor([f], "selective");
      assert.equal(r.lanes.length, ALL_LANES.length);
      assert.ok(r.lanes.every((l) => l.scheduled === true), f);
    }
  });

  it("shadow and full modes schedule every leg (selective disabled)", () => {
    for (const m of ["shadow", "full"]) {
      const r = reportFor(["docs/x.md"], m);
      assert.ok(r.lanes.every((l) => l.scheduled === true), m);
    }
  });
});

describe("gate: fail-closed verdicts", () => {
  it("passes when every leg succeeded", () => {
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs: allPassJobs(), attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
  });

  it("fails on a failed leg", () => {
    const jobs = allPassJobs().map((j) => (j.name === "rt-verify" ? job("rt-verify", "failure") : j));
    const v = evaluateGate({ impact: reportFor(["crates/synvelis-recording/src/journal.rs"]), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("rt-verify")));
  });

  it("fails on cancelled leg", () => {
    const jobs = allPassJobs().map((j) => (j.name === "msrv-verify" ? job("msrv-verify", "cancelled") : j));
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("msrv-verify")));
  });

  it("skipped on a required leg is a violation (not Pass)", () => {
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs: allPassJobs(["verify (ubuntu-latest)"]), attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("skipped")));
  });

  it("missing leg is NotRun, not Pass", () => {
    const jobs = allPassJobs().filter((j) => j.name !== "dep-guard");
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("dep-guard")));
  });

  it("unknown job in run is a violation", () => {
    const jobs = [...allPassJobs(), job("brand-new-lane", "success")];
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("brand-new-lane")));
  });

  it("missing impact report fails closed", () => {
    const v = evaluateGate({ impact: null, jobs: allPassJobs(), attempt: 1 });
    assert.equal(v.ok, false);
  });

  it("shadow mode: not-applicable lane that ran+failed is a false negative", () => {
    // selective disabled: classifier marks ui-verify not-applicable for a rust
    // change, but the (still-running) lane failed -> both a violation and a
    // shadow false-negative record.
    const impact = reportFor(["crates/synvelis-app/src/service.rs"]);
    const jobs = allPassJobs().map((j) => (j.name === "ui-verify (ubuntu-latest)" ? job("ui-verify (ubuntu-latest)", "failure") : j));
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.shadow.some((s) => s.name === "ui-verify (ubuntu-latest)" && s.false_negative));
  });

  it("selective mode: skipped not-applicable leg is authorized", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const jobs = allPassJobs(["verify (ubuntu-latest)", "verify (windows-latest)", "verify (macos-latest)", "rt-verify", "msrv-verify",
      "gw10-verify (ubuntu-latest)", "gw10-verify (windows-latest)", "gw10-verify (macos-latest)"]);
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
  });

  it("selective mode: skipped required leg is a violation", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const v = evaluateGate({ impact, jobs: allPassJobs(["ui-verify (macos-latest)"]), attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("ui-verify (macos-latest)")));
  });

  it("picks the job entry for the requested attempt", () => {
    const jobs = [
      ...allPassJobs().filter((j) => j.name !== "verify (ubuntu-latest)"),
      job("verify (ubuntu-latest)", "failure", 1),
      job("verify (ubuntu-latest)", "success", 2),
    ];
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 2 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
    const v1 = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 1 });
    assert.equal(v1.ok, false);
  });
});
