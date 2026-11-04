// node --test tools/ci-impact.test.mjs
// Issue #148: unit tests for the impact classifier and the fail-closed
// evidence gate. These pin the contract invariants: no under-verification,
// skipped is never Pass, and unknown inputs fall back to full CI.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { buildImpactReport, evaluateGate, expectedJobs } from "./ci-impact.mjs";

// Required lane legs — Issue #157 rev.2: macOS legs are physically
// separated into the non-blocking OBSERVED track, so each matrix lane
// contributes ubuntu+windows legs only.
const ALL_LANES = [
  "verify", "verify",
  "rt-verify",
  "gw10-verify", "gw10-verify",
  "msrv-verify",
  "ui-verify", "ui-verify",
  "dep-guard", "classify",
];

const REQUIRED_LEG_NAMES = [
  "verify (ubuntu-latest)", "verify (windows-latest)",
  "rt-verify",
  "gw10-verify (ubuntu-latest)", "gw10-verify (windows-latest)",
  "msrv-verify",
  "ui-verify (ubuntu-latest)", "ui-verify (windows-latest)",
  "dep-guard", "classify",
];

const OBSERVED_LEG_NAMES = ["verify-macos", "gw10-verify-macos", "ui-verify-macos"];

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

const allPassJobs = (skip = [], observed = OBSERVED_LEG_NAMES.map((n) => job(n))) =>
  REQUIRED_LEG_NAMES
    .map((n) => job(n, skip.includes(n) ? "skipped" : "success"))
    .concat(observed)
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
    assert.deepEqual(requiredLanes(r), ["classify", "dep-guard", "msrv-verify", "rt-verify", "verify", "verify"]);
    assert.ok(notApplicable(r, "gw10-verify").length === 2);
    assert.ok(notApplicable(r, "ui-verify").length === 2);
    assert.deepEqual(r.crates.affected.sort(), ["recording-poc", "synvelis-player", "synvelis-recording", "synvelis-render"].sort());
  });

  it("project change propagates to gw10 (gateway dep) and rt", () => {
    const r = reportFor(["crates/synvelis-project/src/store.rs"]);
    assert.ok(requiredLanes(r).includes("gw10-verify"));
    assert.ok(requiredLanes(r).includes("rt-verify"));
    assert.ok(notApplicable(r, "ui-verify").length === 2);
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
      "ui-verify (ubuntu-latest)", "ui-verify (windows-latest)",
    ].sort());
  });

  it("selective docs-only schedules only classify", () => {
    const r = reportFor(["docs/x.md", "README.md"], "selective");
    assert.deepEqual(scheduledLegs(r), ["classify"]);
  });

  it("selective keeps RT/GW-10/MSRV/verify scheduled for owner-affecting changes", () => {
    const r = reportFor(["crates/synvelis-project/src/store.rs"], "selective");
    const legs = scheduledLegs(r);
    for (const n of ["verify (ubuntu-latest)", "verify (windows-latest)",
      "rt-verify", "msrv-verify", "dep-guard", "classify"]) {
      assert.ok(legs.includes(n), n);
    }
    // synvelis-project propagates to gateway -> gw10 required too.
    assert.ok(legs.includes("gw10-verify (ubuntu-latest)"));
    assert.ok(!legs.includes("ui-verify (ubuntu-latest)"));
    // macOS legs are not part of the required universe at all (rev.2).
    assert.ok(!r.lanes.some((l) => l.os === "macos-latest"));
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
    const jobs = allPassJobs(["verify (ubuntu-latest)", "verify (windows-latest)", "rt-verify", "msrv-verify",
      "gw10-verify (ubuntu-latest)", "gw10-verify (windows-latest)"]);
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
  });

  it("selective mode: skipped required leg is a violation", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const v = evaluateGate({ impact, jobs: allPassJobs(["ui-verify (windows-latest)"]), attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("ui-verify (windows-latest)")));
  });

  it("Issue #157 rev.2: observed macOS legs never gate — cancelled and failed stay non-blocking but recorded", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const observed = [
      job("verify-macos", "cancelled"),
      job("gw10-verify-macos", "failure"),
      { name: "ui-verify-macos", status: "in_progress", conclusion: null, run_attempt: 1 },
    ];
    const v = evaluateGate({ impact, jobs: allPassJobs([], observed), attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
    assert.equal(v.observed.find((o) => o.name === "verify-macos").verdict, "cancelled");
    assert.equal(v.observed.find((o) => o.name === "gw10-verify-macos").verdict, "fail");
    assert.equal(v.observed.find((o) => o.name === "ui-verify-macos").verdict, "pending");
  });

  it("Issue #157 rev.2: observed legs absent from the API view are recorded pending, not violations", () => {
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"], "selective"), jobs: allPassJobs([], []), attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
    assert.ok(v.observed.every((o) => o.verdict === "pending"));
  });

  it("Issue #157 rev.2: required Linux/Windows/RT/MSRV leg cancelled still fails the gate", () => {
    const jobs = allPassJobs().map((j) => (j.name === "gw10-verify (windows-latest)" ? job("gw10-verify (windows-latest)", "cancelled") : j));
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"]), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("gw10-verify (windows-latest)")));
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

// Issue #157 rev.3: a matrix lane skipped by its job-level `if:` is not
// expanded into legs at all — GitHub records a single placeholder job whose
// name is the unexpanded name template ('<lane> (${{ matrix.os }})'). The
// fixture below is the real jobs API payload of run 41894049161 (PR #161),
// where ui-verify was the only unscheduled lane: 13 entries — 8 scheduled
// legs + 1 placeholder + 3 observed macOS jobs + evidence itself.
const RUN_41894049161_JOBS = JSON.parse(
  readFileSync(new URL("./fixtures/run-41894049161-jobs.json", import.meta.url), "utf8"),
);

// PR #161's changed file list — synvelis-project sources propagate to
// verify/rt-verify/gw10-verify/msrv-verify/dep-guard while ui-verify stays
// not-applicable, reproducing the report that run's classify emitted.
const PR_161_FILES = [
  "crates/synvelis-project/src/control.rs",
  "crates/synvelis-project/src/ops.rs",
  "crates/synvelis-project/tests/aut_02.rs",
  "docs/adr/aut-02-boundary-sentinel-anchor-placement.md",
  "docs/designs/aut-02-region-replace-boundary-design.md",
  "docs/impl/ir/aut-02-region-replace-boundary.md",
  "docs/plans/aut-02-region-replace-boundary.md",
  "docs/specs/aut-02-region-replace-boundary-curve-preservation.md",
  "docs/tasks/aut-02-task1-region-replace-boundary.md",
  "docs/test-specs/aut-02-e2e-region-replace-boundary.md",
];

describe("gate: skipped matrix lane placeholders (Issue #157 rev.3)", () => {
  it("placeholder for an unscheduled lane is authorized — run 41894049161 replay", () => {
    const impact = reportFor(PR_161_FILES, "selective");
    const v = evaluateGate({ impact, jobs: RUN_41894049161_JOBS, attempt: 1 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
    const ph = v.rows.find((r) => r.name === "ui-verify (${{ matrix.os }})");
    assert.equal(ph?.verdict, "skipped-placeholder");
    // The lane's real legs stay absent+not-required — no violation either way.
    for (const leg of ["ui-verify (ubuntu-latest)", "ui-verify (windows-latest)"]) {
      const row = v.rows.find((r) => r.name === leg);
      assert.equal(row?.verdict, "not-run");
    }
  });

  it("placeholder for a scheduled lane is a required-lane-skipped violation", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const jobs = allPassJobs()
      .filter((j) => !j.name.startsWith("ui-verify ("))
      .concat([job("ui-verify (${{ matrix.os }})", "skipped")]);
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("ui-verify (${{ matrix.os }})") && m.includes("skipped")));
  });

  it("placeholder-shaped name with an unknown prefix stays an unclassified violation", () => {
    const impact = reportFor(["ui/src/x.ts"], "selective");
    const jobs = [...allPassJobs(), job("foo (${{ matrix.os }})", "skipped")];
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("unclassified") && m.includes("foo")));
  });

  it("placeholder while shadow/full schedules everything is a violation", () => {
    const impact = reportFor(["docs/x.md"], "shadow");
    const jobs = allPassJobs().concat([job("ui-verify (${{ matrix.os }})", "skipped")]);
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, false);
  });

  it("stale placeholder from an earlier attempt is superseded by later legs", () => {
    // Rollback path: attempt 1 ran selective with ui-verify unscheduled
    // (placeholder recorded); re-run-all under CI_SELECTIVE_DISABLE emits a
    // shadow report scheduling every lane, so attempt 2 expands and passes
    // ui-verify's legs. The old placeholder must not retro-fail that run.
    const impact = reportFor(PR_161_FILES, "shadow");
    const jobs = allPassJobs()
      .map((j) => ({ ...j, run_attempt: 2 }))
      .concat([job("ui-verify (${{ matrix.os }})", "skipped", 1)]);
    const v = evaluateGate({ impact, jobs, attempt: 2 });
    assert.equal(v.ok, true, JSON.stringify(v.violations));
  });

  it("placeholder-shaped entry without a skipped conclusion is a violation", () => {
    // A placeholder is by definition a record of a skip; any other
    // conclusion means the API no longer matches the assumed semantics —
    // fail closed rather than accept an unrecognized record.
    const impact = reportFor(PR_161_FILES, "selective");
    const jobs = RUN_41894049161_JOBS.map((j) =>
      j.name === "ui-verify (${{ matrix.os }})" ? { ...j, conclusion: "cancelled" } : j,
    );
    const v = evaluateGate({ impact, jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("ui-verify (${{ matrix.os }})") && m.includes("expected skipped")));
  });

  it("job entries without a string name degrade to unclassified, not a crash", () => {
    const jobs = [...allPassJobs(), { status: "completed", conclusion: "success" }];
    const v = evaluateGate({ impact: reportFor(["ui/src/x.ts"], "selective"), jobs, attempt: 1 });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((m) => m.includes("unclassified")));
  });
});

describe("expected-jobs count (Issue #157 rev.3)", () => {
  it("selective run with one unscheduled lane expects 13 jobs", () => {
    // 8 scheduled legs + 1 placeholder + 3 observed + 1 evidence — matches
    // the real 13-entry payload of run 41894049161.
    const impact = reportFor(PR_161_FILES, "selective");
    assert.equal(expectedJobs(impact), 13);
  });

  it("fully scheduled runs expect 14 jobs", () => {
    for (const m of ["shadow", "full"]) {
      assert.equal(expectedJobs(reportFor(["docs/x.md"], m)), 14, m);
    }
    assert.equal(expectedJobs(reportFor(["Cargo.toml"], "selective")), 14);
  });

  it("docs-only selective run expects 11 jobs", () => {
    // classify only + 6 unscheduled lane entries + 3 observed + 1 evidence.
    assert.equal(expectedJobs(reportFor(["docs/x.md", "README.md"], "selective")), 11);
  });

  it("missing/malformed report falls back to the full universe (fail-safe over-expect)", () => {
    assert.equal(expectedJobs(null), 14);
    assert.equal(expectedJobs({}), 14);
    assert.equal(expectedJobs({ lanes: "bogus" }), 14);
    // Semantically malformed: a report that does not cover the full lane
    // universe is treated the same as an unreadable one — under-expecting
    // could end the bounded retry while jobs are still transiently absent.
    assert.equal(expectedJobs({ lanes: [] }), 14);
    assert.equal(expectedJobs({ lanes: [{ name: "verify (ubuntu-latest)", scheduled: false }] }), 14);
  });
});
