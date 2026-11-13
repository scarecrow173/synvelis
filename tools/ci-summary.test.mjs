// node --test tools/ci-summary.test.mjs
// Issue #166: unit tests for the fail-closed ci-summary gate. These pin
// the contract invariants under Full CI: every lane runs every time, so
// skipped/cancelled/failure/timed_out/absent/unknown are never success;
// `pr` scope keeps macOS legs observed-only while `full` scope promotes
// them (and the `evidence` gate job itself) to required.

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { evaluateGate, expectedJobs, verifySha } from "./ci-summary.mjs";

const REQUIRED_LEG_NAMES = [
  "verify (ubuntu-latest)", "verify (windows-latest)",
  "rt-verify",
  "gw10-verify (ubuntu-latest)", "gw10-verify (windows-latest)",
  "msrv-verify",
  "ui-verify (ubuntu-latest)", "ui-verify (windows-latest)",
  "dep-guard",
];
const MACOS_LEG_NAMES = ["verify-macos", "gw10-verify-macos", "ui-verify-macos"];

const job = (name, conclusion = "success", extra = {}) => ({
  name,
  status: "completed",
  conclusion,
  run_attempt: 1,
  started_at: "2026-11-04T00:00:00Z",
  completed_at: "2026-11-04T00:05:00Z",
  ...extra,
});

const legs = () => [...REQUIRED_LEG_NAMES, ...MACOS_LEG_NAMES].map((n) => job(n));

describe("gate: full CI pass conditions", () => {
  it("all legs pass -> ok (pr and full)", () => {
    // `pr` runs inside `evidence` itself (in_progress entry); `full` runs
    // inside `ci-summary` with `evidence` already completed (it is in
    // needs). Both model the real jobs-API view at gate time.
    const cases = {
      pr: [{ name: "evidence", status: "in_progress", conclusion: null, run_attempt: 1 }],
      full: [
        job("evidence"),
        { name: "ci-summary", status: "in_progress", conclusion: null, run_attempt: 1 },
      ],
    };
    for (const mode of ["pr", "full"]) {
      const jobs = legs().concat(cases[mode]);
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, true, `${mode} violations: ${v.violations}`);
      assert.equal(v.violations.length, 0);
    }
  });

  it("a later attempt's success supersedes an earlier attempt's failure", () => {
    const jobs = legs().concat([
      job("verify (ubuntu-latest)", "failure", { run_attempt: 1 }),
      job("verify (ubuntu-latest)", "success", { run_attempt: 2 }),
      job("evidence", "success", { run_attempt: 2 }),
    ]);
    const v = evaluateGate({ jobs, attempt: 2, mode: "full" });
    assert.equal(v.ok, true, `violations: ${v.violations}`);
  });
});

describe("gate: fail-closed violations (both modes)", () => {
  for (const mode of ["pr", "full"]) {
    it(`${mode}: required leg failure -> violation`, () => {
      const jobs = legs().map((j) => (j.name === "rt-verify" ? job("rt-verify", "failure") : j));
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("rt-verify") && x.includes("failure")));
    });

    it(`${mode}: required leg cancelled -> violation`, () => {
      const jobs = legs().map((j) => (j.name === "dep-guard" ? job("dep-guard", "cancelled") : j));
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("dep-guard") && x.includes("cancelled")));
    });

    it(`${mode}: required leg skipped -> violation (skipped is not Pass)`, () => {
      const jobs = legs().map((j) => (j.name === "msrv-verify" ? job("msrv-verify", "skipped") : j));
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("msrv-verify") && x.includes("skipped")));
    });

    it(`${mode}: required leg in_progress -> not-run violation`, () => {
      const jobs = legs().map((j) =>
        j.name === "verify (windows-latest)" ? { ...j, status: "in_progress", conclusion: null } : j,
      );
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("verify (windows-latest)") && x.includes("not-run")));
    });

    it(`${mode}: required leg absent -> violation`, () => {
      const jobs = legs().filter((j) => j.name !== "gw10-verify (ubuntu-latest)");
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("gw10-verify (ubuntu-latest)") && x.includes("absent")));
    });

    it(`${mode}: timed_out/unknown conclusion -> violation`, () => {
      for (const conclusion of ["timed_out", "action_required", "neutral", "stale", "mystery"]) {
        const jobs = legs().map((j) => (j.name === "ui-verify (ubuntu-latest)" ? job("ui-verify (ubuntu-latest)", conclusion) : j));
        if (mode === "full") jobs.push(job("evidence"));
        const v = evaluateGate({ jobs, mode });
        assert.equal(v.ok, false, `conclusion ${conclusion}`);
        assert.ok(v.violations.some((x) => x.includes("ui-verify (ubuntu-latest)")));
      }
    });

    it(`${mode}: unknown job name -> unclassified violation`, () => {
      const jobs = legs().concat([job("mystery-lane")]);
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("mystery-lane") && x.includes("unclassified")));
    });

    it(`${mode}: skip-collapsed matrix placeholder -> unclassified violation`, () => {
      // Lane `if:` is gone — a placeholder-shaped name is contract
      // drift and must fail closed rather than be tolerated.
      const jobs = legs()
        .filter((j) => !j.name.startsWith("ui-verify"))
        .concat([{ name: "ui-verify (${{ matrix.os }})", status: "completed", conclusion: "skipped", run_attempt: 1 }]);
      if (mode === "full") jobs.push(job("evidence"));
      const v = evaluateGate({ jobs, mode });
      assert.equal(v.ok, false);
      assert.ok(v.violations.some((x) => x.includes("unclassified")));
      assert.ok(v.violations.some((x) => x.includes("ui-verify (ubuntu-latest)")));
    });
  }
});

describe("gate: mode promotion boundary", () => {
  it("pr: macOS failure is observed, not a violation", () => {
    const jobs = legs().map((j) => (j.name === "verify-macos" ? job("verify-macos", "failure") : j));
    const v = evaluateGate({ jobs, mode: "pr" });
    assert.equal(v.ok, true, `violations: ${v.violations}`);
    const o = v.observed.find((x) => x.name === "verify-macos");
    assert.equal(o.verdict, "fail");
  });

  it("pr: absent macOS leg is recorded pending, still not a violation", () => {
    const jobs = legs().filter((j) => j.name !== "gw10-verify-macos");
    const v = evaluateGate({ jobs, mode: "pr" });
    assert.equal(v.ok, true, `violations: ${v.violations}`);
    const o = v.observed.find((x) => x.name === "gw10-verify-macos");
    assert.equal(o.verdict, "pending");
  });

  it("full: macOS failure/cancelled/absent is a violation", () => {
    for (const [name, conclusion] of [["verify-macos", "failure"], ["gw10-verify-macos", "cancelled"]]) {
      const jobs = legs().map((j) => (j.name === name ? job(name, conclusion) : j)).concat([job("evidence")]);
      const v = evaluateGate({ jobs, mode: "full" });
      assert.equal(v.ok, false, name);
      assert.ok(v.violations.some((x) => x.includes(name)));
    }
    const absent = legs().filter((j) => j.name !== "ui-verify-macos").concat([job("evidence")]);
    const v2 = evaluateGate({ jobs: absent, mode: "full" });
    assert.equal(v2.ok, false);
    assert.ok(v2.violations.some((x) => x.includes("ui-verify-macos")));
  });

  it("full: evidence gate job failure/absent is a violation", () => {
    const failed = legs().concat([job("evidence", "failure")]);
    const v1 = evaluateGate({ jobs: failed, mode: "full" });
    assert.equal(v1.ok, false);
    assert.ok(v1.violations.some((x) => x.includes("'evidence'") && x.includes("failure")));

    const missing = legs();
    const v2 = evaluateGate({ jobs: missing, mode: "full" });
    assert.equal(v2.ok, false);
    assert.ok(v2.violations.some((x) => x.includes("'evidence'") && x.includes("absent")));
  });

  it("full: evidence skipped is a violation", () => {
    const jobs = legs().concat([job("evidence", "skipped")]);
    const v = evaluateGate({ jobs, mode: "full" });
    assert.equal(v.ok, false);
    assert.ok(v.violations.some((x) => x.includes("'evidence'") && x.includes("skipped")));
  });

  it("pr: ci-summary job may be absent and is never unclassified", () => {
    const withSummary = legs().concat([{ name: "ci-summary", status: "queued", conclusion: null, run_attempt: 1 }]);
    const v1 = evaluateGate({ jobs: withSummary, mode: "pr" });
    assert.equal(v1.ok, true, `violations: ${v1.violations}`);
    const withoutSummary = legs();
    const v2 = evaluateGate({ jobs: withoutSummary, mode: "pr" });
    assert.equal(v2.ok, true, `violations: ${v2.violations}`);
  });
});

describe("expected-jobs count", () => {
  it("pr -> 13 (12 legs + evidence itself)", () => {
    assert.equal(expectedJobs("pr"), 13);
  });
  it("full -> 14 (12 legs + evidence + ci-summary itself)", () => {
    assert.equal(expectedJobs("full"), 14);
  });
  it("unknown mode falls back to the full universe floor", () => {
    assert.equal(expectedJobs("bogus"), 14);
  });
});

describe("verify-sha: expected_sha revision contract", () => {
  const SHA_A = "a".repeat(40);
  const SHA_B = "b".repeat(40);

  it("unset expected_sha -> ok, contract not engaged", () => {
    for (const expected of [undefined, null, "", "  "]) {
      const r = verifySha({ expected, head: SHA_A, githubSha: SHA_A });
      assert.equal(r.ok, true);
      assert.equal(r.skipped, true);
      assert.equal(r.errors.length, 0);
    }
  });

  it("expected == head == github.sha -> ok", () => {
    const r = verifySha({ expected: SHA_A, head: SHA_A, githubSha: SHA_A });
    assert.equal(r.ok, true);
    assert.equal(r.skipped, false);
    assert.equal(r.errors.length, 0);
  });

  it("expected != github.sha -> revision mismatch error (stale dispatch)", () => {
    // The ref moved after the scheduler computed the candidate SHA: the
    // workflow file was loaded from github.sha's revision, so the run
    // cannot certify expected_sha — fail closed, never a warning.
    const r = verifySha({ expected: SHA_A, head: SHA_A, githubSha: SHA_B });
    assert.equal(r.ok, false);
    assert.equal(r.errors.length, 1);
    assert.ok(r.errors[0].includes("workflow revision"));
    assert.ok(r.errors[0].includes(SHA_B));
  });

  it("expected != checked-out HEAD -> checkout mismatch error", () => {
    const r = verifySha({ expected: SHA_A, head: SHA_B, githubSha: SHA_A });
    assert.equal(r.ok, false);
    assert.equal(r.errors.length, 1);
    assert.ok(r.errors[0].includes("checked-out HEAD"));
  });

  it("both mismatches are reported together", () => {
    const SHA_C = "c".repeat(40);
    const r = verifySha({ expected: SHA_A, head: SHA_B, githubSha: SHA_C });
    assert.equal(r.ok, false);
    assert.equal(r.errors.length, 2);
  });

  it("a ref name / short sha as expected_sha fails strict equality", () => {
    for (const expected of ["main", SHA_A.slice(0, 7), "HEAD"]) {
      const r = verifySha({ expected, head: SHA_A, githubSha: SHA_A });
      assert.equal(r.ok, false, `input ${expected} must not pass`);
      assert.ok(r.errors[0].includes("workflow revision"));
    }
  });

  it("hex comparison is case-insensitive", () => {
    const r = verifySha({ expected: SHA_A.toUpperCase(), head: SHA_A, githubSha: SHA_A });
    assert.equal(r.ok, true);
  });
});

describe("gate: malformed inputs stay fail-closed", () => {
  it("empty/missing jobs -> violations for every required leg", () => {
    const v = evaluateGate({ jobs: [], mode: "full" });
    assert.equal(v.ok, false);
    assert.equal(v.violations.length, 12 + 1); // 12 legs + evidence
  });
  it("unknown mode -> not ok", () => {
    const v = evaluateGate({ jobs: legs(), mode: "bogus" });
    assert.equal(v.ok, false);
  });
});


describe("CI workflow: Linux-only shared lint with native OS-L1 coverage", () => {
  const ci = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
  function job(id) {
    const start = ci.indexOf("\n  " + id + ":\n");
    assert.ok(start >= 0, "missing job " + id);
    const rest = ci.slice(start + 1);
    const next = rest.slice(1).search(/\n  [A-Za-z][\w-]*:\n/);
    return next < 0 ? rest : rest.slice(0, next + 1);
  }
  it("runs fmt, workflow lint, Clippy and prek only on Linux", () => {
    const v = job("verify");
    const mac = job("verify-macos");
    for (const [task, label] of [
      ["mise exec -- prek run --all-files", "Check config files with prek"],
      ["mise run lint-actions", "Lint workflows (mise run lint-actions)"],
      ["mise run fmt-check", "Check formatting (mise run fmt-check)"],
      ["mise run lint-full", "Lint workspace (mise run lint-full)"],
      ["mise run lint-app-native", "Lint synvelis-app native feature (mise run lint-app-native)"],
      ["mise run lint-app-gpui", "Lint synvelis-app gpui-test feature (mise run lint-app-gpui)"],
    ]) {
      const from = v.indexOf("      - name: " + label + "\n");
      assert.ok(from >= 0, "missing Linux step: " + label);
      const to = v.indexOf("\n      - name:", from + 1);
      const step = v.slice(from, to < 0 ? v.length : to);
      assert.ok(step.includes("if: matrix.os == 'ubuntu-latest'"), label + " not Linux-only");
      assert.ok(step.includes("run: " + task), label + " task mismatch");
      assert.ok(!mac.includes("run: " + task), task + " duplicated in macOS");
    }
  });
  it("keeps per-OS type-check, native binary Link and GPUI scenario tests", () => {
    for (const j of ["verify", "verify-macos"]) {
      const block = job(j);
      for (const task of ["check-full", "test-full", "check-app-native",
        "build-app-native", "check-app-gpui", "test-app-gpui"]) {
        assert.ok(block.includes("run: mise run " + task), j + " missing " + task);
      }
      assert.ok(block.includes("gpui-test scenario tests:"), j + " missing test guard");
    }
  });
});
