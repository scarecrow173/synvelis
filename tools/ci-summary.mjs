// Issue #166 stage 1: Synvelis-owned fail-closed CI aggregation gate.
// Aligned with the ruviform #365 common contract — Full CI runs every
// lane on every trigger, and the `ci-summary` job is the single
// fail-closed verdict over the run's actual job results.
//
// The retired impact classifier (tools/ci-impact.mjs, Issue #148/#157)
// decided WHICH lanes should run; under Full CI every lane always runs,
// so this tool owns only the gate: required leg set vs jobs-API
// conclusions. skipped/cancelled/failure/timed_out/absent/unknown are
// never success. There is no NotApplicable and no shadow comparison —
// a leg that does not pass is a violation, period.
//
// Two scopes share this implementation:
//   mode=pr    — the `evidence` job (ruleset main-checkstatus required
//                context, kept alive until the owner lifts it). The
//                macOS dedicated jobs stay non-blocking observed legs,
//                preserving the Issue #157 rev.2 PR-gate boundary:
//                macOS runner starvation must not deadlock PRs while
//                pull_request triggers remain.
//   mode=full  — the `ci-summary` job. Every leg INCLUDING macOS is
//                required (Issue #166: a macOS failure is a Full CI
//                failure), and the `evidence` gate job itself is a
//                required entry so gate-infrastructure failure can
//                never be recorded as a green Full CI.
//
// Subcommands:
//   gate           jobs API results -> fail-closed verdict
//   expected-jobs  jobs-API entry-count floor for the collect retry
//   lanes          diagnostic: every leg name in the universe
//
// Usage:
//   node tools/ci-summary.mjs gate --jobs <path> [--mode pr|full]
//                                 [--attempt <n>] [--summary <path>] [--out <path>]
//   node tools/ci-summary.mjs expected-jobs [--mode pr|full]
//   node tools/ci-summary.mjs lanes
//
// Env fallbacks: CI_SUMMARY_MODE, CI_JOBS_FILE, CI_RUN_ATTEMPT,
// GITHUB_STEP_SUMMARY, GITHUB_OUTPUT.

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

// --- Leg universe ------------------------------------------------------------
// Every lane leg ci.yml schedules on every trigger, keyed by its exact
// GitHub check name. With no lane `if:` conditions a matrix lane always
// expands to one jobs-API entry per leg — skip-collapsed placeholders
// cannot occur and, if observed, indicate contract drift: they are
// rejected as unclassified jobs below.
const REQUIRED_LEGS = [
  { lane: "verify", os: "ubuntu-latest", name: "verify (ubuntu-latest)" },
  { lane: "verify", os: "windows-latest", name: "verify (windows-latest)" },
  { lane: "rt-verify", os: "ubuntu-latest", name: "rt-verify" },
  { lane: "gw10-verify", os: "ubuntu-latest", name: "gw10-verify (ubuntu-latest)" },
  { lane: "gw10-verify", os: "windows-latest", name: "gw10-verify (windows-latest)" },
  { lane: "msrv-verify", os: "ubuntu-latest", name: "msrv-verify" },
  { lane: "ui-verify", os: "ubuntu-latest", name: "ui-verify (ubuntu-latest)" },
  { lane: "ui-verify", os: "windows-latest", name: "ui-verify (windows-latest)" },
  { lane: "dep-guard", os: "ubuntu-latest", name: "dep-guard" },
];

// Issue #157 rev.2 / #166 stage 1: dedicated macOS track. Under `pr`
// scope these stay observed-only (recorded, never gating) so runner
// starvation cannot deadlock a PR; under `full` scope they are required
// — a macOS failure is a Full CI failure.
const MACOS_LEGS = [
  { lane: "verify", os: "macos-latest", name: "verify-macos" },
  { lane: "gw10-verify", os: "macos-latest", name: "gw10-verify-macos" },
  { lane: "ui-verify", os: "macos-latest", name: "ui-verify-macos" },
];

// Aggregating gate jobs are known names but never evaluated as legs —
// a job cannot verify itself. `evidence` is additionally a required
// entry under `full` scope (see mode table in the header).
const GATE_JOBS = ["evidence", "ci-summary"];

// Standing declared-not-run verification rows: capability-separated
// checks that are not jobs in this workflow. Never counted as Pass —
// each carries a reason and is managed under separate acceptance.
const DECLARED_NOT_RUN = [
  { lane: "RF-GT-01 Browser DOM/Accessibility Playwright", status: "NotRun", reason: "ruviform harness landed (#310); synvelis lane adoption not implemented" },
  { lane: "RF-GT-02 IPC conformance fixture", status: "NotRun", reason: "ruviform #311 landed; synvelis lane adoption not implemented" },
  { lane: "RF-GT-03/04 Secure Test Host / OS Driver + SV-GUI-05", status: "NotRun", reason: "ruviform #312/#313, synvelis #114 not implemented" },
  { lane: "Visual Golden", status: "NotRun", reason: "ruviform #314 not implemented" },
  { lane: "synvelis-app webview feature real-window host", status: "Unsupported", reason: "capability-separated: real WebView needs a display host; headless surface is covered by verify (mise tasks check-app-webview/lint-app-webview/test-app-webview remain local)" },
  { lane: "Recording real Device/Plugin Binding/PPQN-MIDI/Automation feature lanes", status: "NotRun", reason: "feature implementations pending (#115-#134)" },
  { lane: "AudioLens consumer regression", status: "NotRun", reason: "ruviform #294 / audio-lens #142 ownership" },
];

const MODES = new Set(["pr", "full"]);

// --- Gate --------------------------------------------------------------------

const FAIL_CONCLUSIONS = new Set(["failure", "timed_out", "action_required", "startup_failure", "neutral", "stale"]);

function jobVerdict(job) {
  if (!job) return "not-run";
  if (job.status !== "completed") return "not-run";
  if (job.conclusion === "success") return "pass";
  if (job.conclusion === "skipped") return "skipped";
  if (job.conclusion === "cancelled") return "cancelled";
  if (FAIL_CONCLUSIONS.has(job.conclusion)) return "fail";
  return "fail"; // unknown conclusion -> fail-closed
}

function pickJob(entries, attempt) {
  if (!entries || entries.length === 0) return undefined;
  if (attempt !== undefined) {
    const eligible = entries.filter((e) => (e.run_attempt ?? 1) <= attempt);
    if (eligible.length > 0) return eligible.reduce((a, b) => ((a.run_attempt ?? 1) >= (b.run_attempt ?? 1) ? a : b));
  }
  return entries.reduce((a, b) => ((a.run_attempt ?? 1) >= (b.run_attempt ?? 1) ? a : b));
}

function durationMs(job) {
  if (!job?.started_at || !job?.completed_at) return null;
  return new Date(job.completed_at) - new Date(job.started_at);
}

/**
 * Fail-closed evaluation of the full lane universe against the workflow
 * run's actual job results. `mode` is "pr" (evidence compat: macOS legs
 * observed-only) or "full" (ci-summary: every leg required, `evidence`
 * itself required). Returns { ok, rows, violations, observed }.
 */
export function evaluateGate({ jobs, attempt, mode = "full" }) {
  if (!MODES.has(mode)) {
    return { ok: false, rows: [], violations: [`unknown gate mode '${mode}' — expected pr|full`], observed: [] };
  }
  const byName = new Map();
  for (const j of jobs ?? []) {
    const list = byName.get(j.name) ?? [];
    list.push(j);
    byName.set(j.name, list);
  }
  const violations = [];
  const rows = [];

  const jobLabel = (job) => (job ? `${job.status}/${job.conclusion ?? "-"}` : "absent");
  const evalLeg = (leg, required) => {
    const job = pickJob(byName.get(leg.name), attempt);
    const verdict = jobVerdict(job);
    rows.push({ name: leg.name, required, verdict, job });
    if (!required) return verdict;
    if (verdict === "skipped") violations.push(`'${leg.name}' was skipped — required lanes may never skip (skipped is not Pass)`);
    else if (verdict !== "pass") violations.push(`'${leg.name}' expected Pass but was ${verdict} (job: ${jobLabel(job)})`);
    return verdict;
  };

  for (const leg of REQUIRED_LEGS) evalLeg(leg, true);
  // `full` scope promotes the macOS legs to required; `pr` scope records
  // them as observed verdicts (pending when absent/queued/in progress).
  const observed = MACOS_LEGS.map((leg) => {
    if (mode === "full") {
      evalLeg(leg, true);
      return null;
    }
    const job = pickJob(byName.get(leg.name), attempt);
    const v = jobVerdict(job);
    return { name: leg.name, lane: leg.lane, os: leg.os, verdict: v === "not-run" ? "pending" : v, job };
  }).filter(Boolean);

  // `full` scope: the `evidence` gate job is itself a required entry —
  // a failed gate infrastructure must not be recorded as a green run.
  // `pr` scope: `evidence` is the evaluating job and cannot grade itself.
  if (mode === "full") {
    const job = pickJob(byName.get("evidence"), attempt);
    const verdict = jobVerdict(job);
    rows.push({ name: "evidence", required: true, verdict, job });
    if (verdict === "skipped") violations.push("'evidence' was skipped — the aggregation gate may never skip");
    else if (verdict !== "pass") violations.push(`'evidence' expected Pass but was ${verdict} (job: ${jobLabel(job)})`);
  }

  for (const name of byName.keys()) {
    const known =
      REQUIRED_LEGS.some((l) => l.name === name) ||
      MACOS_LEGS.some((l) => l.name === name) ||
      GATE_JOBS.includes(name);
    if (!known) violations.push(`unclassified job '${name}' in this run — the leg universe in tools/ci-summary.mjs is total over ci.yml`);
  }

  return { ok: violations.length === 0, rows, violations, observed };
}

/**
 * Jobs-API entry-count floor for the collect retry. Every lane runs on
 * every trigger, so the universe is fixed: 12 legs plus the gate jobs
 * guaranteed present when the collecting job runs (`pr`: evidence
 * itself = 13; `full`: evidence + ci-summary itself = 14 — ci-summary
 * `needs` evidence, so its entry always exists by then).
 */
export function expectedJobs(mode = "full") {
  if (!MODES.has(mode)) return REQUIRED_LEGS.length + MACOS_LEGS.length + GATE_JOBS.length;
  const gateCount = mode === "full" ? GATE_JOBS.length : 1;
  return REQUIRED_LEGS.length + MACOS_LEGS.length + gateCount;
}

// --- Reporting ---------------------------------------------------------------

function mdEscape(s) {
  return String(s).replaceAll("|", "\\|");
}

function summaryMarkdown({ verdict, mode, context }) {
  const lines = [];
  const jobName = mode === "full" ? "ci-summary" : "evidence";
  lines.push(`# ${jobName} — ${context.event} ${context.ref}`);
  lines.push("");
  lines.push(`- run: ${context.runUrl} (attempt ${context.attempt})`);
  lines.push(`- commit: ${context.headSha}`);
  if (context.baseSha) lines.push(`- base: ${context.baseSha}`);
  lines.push(`- ruviform resolved SHA: ${context.ruviformSha}`);
  lines.push(`- gate verdict: **${verdict.ok ? "pass" : "FAIL"}**`);
  lines.push("");
  lines.push(`## SHA contract`);
  lines.push("");
  lines.push(`| field | value |`);
  lines.push(`|---|---|`);
  lines.push(`| expected_sha input | ${context.expectedSha ?? "<unset>"} |`);
  lines.push(`| checked-out HEAD | ${context.resolvedHead ?? context.headSha} |`);
  lines.push(`| github.sha | ${context.headSha} |`);
  lines.push(`| ruviform resolved SHA | ${context.ruviformSha} |`);
  lines.push("");
  const requiredRows = verdict.rows.filter((r) => r.required);
  lines.push(`| lane leg | actual | verdict |`);
  lines.push(`|---|---|---|`);
  for (const r of requiredRows) {
    const job = r.job ? `${r.job.status}/${r.job.conclusion ?? "-"}` : "absent";
    lines.push(`| ${r.name} | ${job} | ${r.verdict} |`);
  }
  const observedRows = verdict.observed ?? [];
  if (observedRows.length > 0) {
    lines.push("");
    lines.push(`## Observed macOS legs (non-blocking — recorded, never counted as Pass/Fail)`);
    lines.push("");
    lines.push(`| lane leg | actual | verdict |`);
    lines.push(`|---|---|---|`);
    for (const o of observedRows) {
      const job = o.job ? `${o.job.status}/${o.job.conclusion ?? "-"}` : "absent";
      lines.push(`| ${o.name} | ${job} | ${o.verdict} |`);
    }
  }
  if (verdict.violations.length > 0) {
    lines.push("");
    lines.push(`## Violations`);
    lines.push("");
    for (const v of verdict.violations) lines.push(`- ${v}`);
  }
  lines.push("");
  lines.push(`## Declared NotRun/Unsupported lanes (never counted as Pass)`);
  lines.push("");
  lines.push(`| lane | status | reason |`);
  lines.push(`|---|---|---|`);
  for (const d of DECLARED_NOT_RUN) lines.push(`| ${mdEscape(d.lane)} | ${d.status} | ${mdEscape(d.reason)} |`);
  const totalMs = requiredRows.reduce((acc, r) => acc + (durationMs(r.job) ?? 0), 0);
  lines.push("");
  lines.push(`Total lane runner time: ${(totalMs / 60000).toFixed(1)} min`);
  return lines.join("\n");
}

// --- CLI ---------------------------------------------------------------------

function parseArgs(argv) {
  const args = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const eq = a.indexOf("=");
      if (eq !== -1) args[a.slice(2, eq)] = a.slice(eq + 1);
      else if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) args[a.slice(2)] = argv[++i];
      else args[a.slice(2)] = true;
    } else args._.push(a);
  }
  return args;
}

function resolveMode(args) {
  const m = args.mode ?? process.env.CI_SUMMARY_MODE ?? "full";
  if (!MODES.has(m)) {
    process.stderr.write(`ci-summary: unknown mode '${m}' — expected pr|full\n`);
    process.exit(2);
  }
  return m;
}

function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  const args = parseArgs(rest);

  if (cmd === "lanes") {
    // Every leg name the run can produce besides the gate jobs —
    // diagnostic aid mirroring the retired tool's `lanes` subcommand.
    for (const l of [...REQUIRED_LEGS, ...MACOS_LEGS]) process.stdout.write(`${l.name}\n`);
    return;
  }

  if (cmd === "expected-jobs") {
    // Jobs-API entry-count floor for the collect step's bounded retry.
    process.stdout.write(`${expectedJobs(resolveMode(args))}\n`);
    return;
  }

  if (cmd === "gate") {
    const mode = resolveMode(args);
    const jobsPath = args.jobs ?? process.env.CI_JOBS_FILE;
    let jobs = [];
    try {
      const raw = JSON.parse(readFileSync(jobsPath, "utf8"));
      jobs = Array.isArray(raw) ? raw : raw.jobs;
    } catch {
      jobs = [];
    }
    if (!Array.isArray(jobs)) jobs = [];
    const attempt = Number(args.attempt ?? process.env.CI_RUN_ATTEMPT ?? 0) || undefined;
    const verdict = evaluateGate({ jobs, attempt, mode });
    const context = {
      event: process.env.GITHUB_EVENT_NAME ?? "local",
      ref: process.env.GITHUB_REF_NAME ?? "",
      runUrl: process.env.CI_RUN_URL ?? `run ${process.env.GITHUB_RUN_ID ?? "?"}`,
      attempt: attempt ?? "?",
      headSha: process.env.CI_HEAD_SHA ?? process.env.GITHUB_SHA ?? "?",
      baseSha: process.env.CI_BASE_SHA ?? null,
      ruviformSha: process.env.CI_RUVIFORM_SHA ?? "n/a",
      expectedSha: process.env.CI_EXPECTED_SHA || null,
      resolvedHead: process.env.CI_RESOLVED_HEAD || null,
    };
    const md = summaryMarkdown({ verdict, mode, context });
    const summaryPath = args.summary ?? process.env.GITHUB_STEP_SUMMARY;
    if (summaryPath) appendFileSync(summaryPath, md + "\n");
    else process.stdout.write(md + "\n");
    if (args.out) writeFileSync(args.out, JSON.stringify({ ...verdict, rows: verdict.rows.map(({ job, ...r }) => r), observed: (verdict.observed ?? []).map(({ job, ...o }) => o) }, null, 2) + "\n");
    for (const v of verdict.violations) process.stderr.write(`::error::${v}\n`);
    for (const o of verdict.observed ?? []) {
      if (o.verdict === "fail" || o.verdict === "cancelled") {
        process.stderr.write(`::warning::observed macOS leg '${o.name}' ${o.verdict} — non-blocking, recorded for audit\n`);
      }
    }
    if (!verdict.ok) process.exit(1);
    process.stdout.write(`ci-summary gate (${mode}): all required lanes passed; evidence recorded\n`);
    return;
  }

  process.stderr.write("usage: ci-summary.mjs gate|expected-jobs|lanes — see file header\n");
  process.exit(2);
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (isMain) main();
