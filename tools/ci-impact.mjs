// Issue #148 (SV-CI-OPT-02): Synvelis-owned change-impact classifier and
// fail-closed evidence gate. Aligned with the ruviform #333 generic impact
// contract vocabulary (Pass / Fail / NotRun / Unsupported / NotApplicable);
// while #333 ships no shared implementation, this file is the authoritative
// Synvelis mapping — see docs/specs/issue-148-impact-classification.md.
//
// Issue #157: selective execution enabled. classify additionally emits a
// `lanes` output — the lane ids ci.yml must schedule — consumed by each
// lane job's `if:` condition. Scheduling is lane-uniform (all legs of a
// lane share the classifier's expectation); leg-level audit stays in the
// gate. The gate contract is unchanged: `evidence` remains the single
// fail-closed verdict over the expected set vs the run's actual results.
//
// Issue #157 rev.2 (Issue #148/#145 2026-11-05 agreement): macOS legs are
// physically separated from the required universe — they run in dedicated
// non-required jobs (OBSERVED_LEGS), are never in the gate's expected set,
// and their results are recorded as observed verdicts (Pass/Fail/
// Cancelled/Skipped/Pending) without being counted as Pass or violations.
//
// Issue #157 rev.3: a matrix lane skipped by its job-level `if:` is not
// expanded into legs at all — GitHub records a single placeholder job named
// after the unexpanded name template ('<lane> (${{ matrix.os }})',
// conclusion=skipped). The gate classifies that name as a skip-collapsed
// placeholder: legal only while the impact report marks every leg of the
// lane scheduled=false, otherwise it is the same violation as a skipped
// required leg. An unknown prefix stays an unclassified-job violation.
// `expected-jobs` gives the evidence collect step the exact jobs-API entry
// count for the current report: scheduled lanes expand to legs, skipped
// lanes collapse to one entry each (placeholder for matrix lanes, a
// normal-named skipped entry for non-matrix lanes).
//
// Subcommands:
//   classify  changed files -> impact classes -> expected lane set (JSON)
//   gate      expected set + workflow run job results -> fail-closed verdict
//
// Contract:
//   - `skipped` is never implicitly Pass. In selective mode a skipped leg is
//     legal only when the classifier marked it `not-applicable` with a reason;
//     in shadow/full mode every leg is expected and `skipped` is a violation.
//   - Unknown files fall back to full CI (fail-safe, never under-verify).
//   - Unknown lane legs in the run are a violation: the lane universe here is
//     total over ci.yml, so a new lane without a mapping update fails closed.
//
// Usage:
//   node tools/ci-impact.mjs classify [--base <sha> --head <sha> | --files-file <f>]
//                                     [--mode shadow|full|selective] [--out <path>]
//                                     [--set-output <name>] [--lanes-output <name>]
//                                     [--summary <path>]
//   node tools/ci-impact.mjs gate --impact <path> --jobs <path>
//                                 [--attempt <n>] [--summary <path>] [--out <path>]
//   node tools/ci-impact.mjs lanes
//   node tools/ci-impact.mjs expected-jobs [--impact <path>]
//
// Env fallbacks: CI_IMPACT_MODE, CI_IMPACT_BASE, CI_IMPACT_HEAD,
// CI_IMPACT_FILES_FILE, CI_IMPACT_LANES_OUTPUT, GITHUB_EVENT_NAME,
// GITHUB_OUTPUT, GITHUB_STEP_SUMMARY.

import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const SCHEMA = "synvelis/ci-impact/v1";

// --- Lane universe -----------------------------------------------------------
// Every lane leg ci.yml can schedule, keyed by its exact GitHub check name.
// `evidence` is the aggregating gate itself and is not part of the evaluated
// universe (it cannot verify itself); it is handled by GATE_JOB below.
const LANE_LEGS = [
  { lane: "verify", os: "ubuntu-latest", name: "verify (ubuntu-latest)" },
  { lane: "verify", os: "windows-latest", name: "verify (windows-latest)" },
  { lane: "rt-verify", os: "ubuntu-latest", name: "rt-verify" },
  { lane: "gw10-verify", os: "ubuntu-latest", name: "gw10-verify (ubuntu-latest)" },
  { lane: "gw10-verify", os: "windows-latest", name: "gw10-verify (windows-latest)" },
  { lane: "msrv-verify", os: "ubuntu-latest", name: "msrv-verify" },
  { lane: "ui-verify", os: "ubuntu-latest", name: "ui-verify (ubuntu-latest)" },
  { lane: "ui-verify", os: "windows-latest", name: "ui-verify (windows-latest)" },
  { lane: "dep-guard", os: "ubuntu-latest", name: "dep-guard" },
  { lane: "classify", os: "ubuntu-latest", name: "classify" },
];
const GATE_JOB = "evidence";

// Issue #157 rev.2: non-blocking macOS track. These are dedicated jobs in
// ci.yml (not legs of the required lanes) so runner starvation on
// macos-latest can never gate a PR. They keep running on PR/main and the
// gate records their real conclusions as observed verdicts — runner
// shortage is recorded as pending/cancelled, never as NotApplicable, and
// a real failure is surfaced, not hidden. They are also never "unknown
// job" violations.
const OBSERVED_LEGS = [
  { lane: "verify", os: "macos-latest", name: "verify-macos" },
  { lane: "gw10-verify", os: "macos-latest", name: "gw10-verify-macos" },
  { lane: "ui-verify", os: "macos-latest", name: "ui-verify-macos" },
];

// Issue #157 rev.3: a job-level `if:` skip collapses a matrix job into one
// placeholder entry carrying the unexpanded name template — observed as
// 'ui-verify (${{ matrix.os }})' (job id 126798807824, run 41894049161).
// Tolerates expression/whitespace drift; the prefix must still name a lane.
const PLACEHOLDER_NAME_RE = /^(.+?)\s*\(\s*\$\{\{[\s\S]*?\}\}\s*\)\s*$/;

// --- Internal crate dependency graph (path deps from crates/*/Cargo.toml) -----
const CRATE_DEPS = {
  "synvelis-project": [],
  "synvelis-recording": ["synvelis-project"],
  "recording-poc": ["synvelis-recording", "synvelis-project"],
  "synvelis-player": ["synvelis-recording", "synvelis-project"],
  "synvelis-render": ["synvelis-player", "synvelis-recording", "synvelis-project"],
  "synvelis-gateway": ["synvelis-project"],
  "mcp-poc": ["synvelis-gateway", "synvelis-project"],
  "synvelis-workspace": ["synvelis-gateway", "synvelis-project"],
  "synvelis-app": ["synvelis-workspace", "synvelis-project"],
};

// Crates hosting rt_ contract tests (`cargo test rt_` selection). A change in
// an owner crate or any crate it depends on can break an RT contract, so the
// guard lives on affected-set membership, not on file name.
const RT_OWNERS = new Set(["synvelis-project", "synvelis-recording"]);
// Crates whose verification runs through the GW-10 harness lane
// (`mise run gw10` = synvelis-gateway gw10_* tests + mcp-poc gw10_transport).
const GW_OWNERS = new Set(["synvelis-gateway", "mcp-poc"]);

// Headline impact class per crate. Classes are audit vocabulary (the issue's
// Project/Recording/…/toolchain list); lane requirements derive from crate
// membership + propagation, not from class names.
const CRATE_CLASSES = {
  "synvelis-project": ["project", "control", "automation"],
  "synvelis-recording": ["recording", "recovery"],
  "recording-poc": ["recording", "recovery"],
  "synvelis-player": ["playback", "graph", "plugin"],
  "synvelis-render": ["graph", "playback"],
  "synvelis-gateway": ["gateway", "ipc"],
  "mcp-poc": ["gateway", "ipc"],
  "synvelis-workspace": ["control", "ipc"],
  "synvelis-app": ["webview", "ipc"],
};

// File-level class refinements inside crates (reporting granularity only).
const FILE_CLASS_RULES = [
  { re: /^crates\/synvelis-project\/src\/(control|controller|midi)\.rs$/, classes: ["control", "automation"] },
  { re: /^crates\/synvelis-project\/tests\/(ctl_01|pc_b_rt|tempo|midi_source)\.rs$/, classes: ["control", "automation"] },
  { re: /^crates\/synvelis-project\/tests\/pc_b_rt\.rs$/, classes: ["rt"] },
  { re: /^crates\/synvelis-recording\/src\/(journal|recovery|faultinj|commit)\.rs$/, classes: ["recovery"] },
  { re: /^crates\/synvelis-recording\/src\/(gate|capture|clock|engine|handoff|bounded|mock|plan|ops)\.rs$/, classes: ["rt"] },
  { re: /^crates\/synvelis-recording\/tests\/(vs_b.*|recording)\.rs$/, classes: ["rt"] },
  { re: /^crates\/synvelis-recording\/tests\/(vs_c|vs_d)\.rs$/, classes: ["recovery"] },
  { re: /^crates\/synvelis-app\/src\/(service|ui_ops|host|app)\.rs$/, classes: ["ipc"] },
  { re: /^crates\/synvelis-app\/src\/(assets|host)\.rs$|^crates\/synvelis-app\/build\.rs$/, classes: ["webview"] },
];

// Ordered path rules. `trigger` means the change can invalidate any lane's
// assumptions (lockfile/toolchain/CI config) -> full CI, matching the
// issue's "ambiguous/shared change -> Full CI" fallback.
const PATH_RULES = [
  { re: /^Cargo\.lock$/, classes: ["cargo-lock"], trigger: true },
  { re: /^Cargo\.toml$/, classes: ["cargo-lock"], trigger: true },
  { re: /^crates\/[^/]+\/Cargo\.toml$/, classes: ["cargo-lock"], trigger: true },
  { re: /^(mise\.toml|mise\.lock|rust-toolchain.*|\.mise\/.*)$/, classes: ["toolchain"], trigger: true },
  { re: /^\.github\//, classes: ["ci"], trigger: true },
  { re: /^tools\//, classes: ["ci"], trigger: true },
  { re: /^\.pre-commit-config\.yaml$/, classes: ["ci"], trigger: true },
  { re: /^ui\//, classes: ["solidjs-gui"] },
  { re: /^crates\/([^/]+)\//, crateGroup: 1 },
  { re: /^(docs\/.*|.*\.md|LICENSE.*|assets\/.*|\.gitignore|\.gitattributes|\.worktreeinclude|apm\.yml|apm\.lock\.yaml|\.serena\/.*|\.claude\/.*|\.codex\/.*|\.cursor\/.*|\.windsurf\/.*|\.vscode\/.*|\.mcp\.json)$/, classes: ["docs"] },
];

// Standing declared-not-run verification rows. These are capability-separated
// lanes the issue's vocabulary calls NotRun/Unsupported: they are not jobs in
// this workflow, they are never counted as Pass, and each carries a reason.
const DECLARED_NOT_RUN = [
  { lane: "RF-GT-01 Browser DOM/Accessibility Playwright", status: "NotRun", reason: "ruviform harness landed (#310); synvelis lane adoption not implemented" },
  { lane: "RF-GT-02 IPC conformance fixture", status: "NotRun", reason: "ruviform #311 landed; synvelis lane adoption not implemented" },
  { lane: "RF-GT-03/04 Secure Test Host / OS Driver + SV-GUI-05", status: "NotRun", reason: "ruviform #312/#313, synvelis #114 not implemented" },
  { lane: "Visual Golden", status: "NotRun", reason: "ruviform #314 not implemented" },
  { lane: "synvelis-app webview feature real-window host", status: "Unsupported", reason: "capability-separated: real WebView needs a display host; headless surface is covered by verify (mise tasks check-app-webview/lint-app-webview/test-app-webview remain local)" },
  { lane: "Recording real Device/Plugin Binding/PPQN-MIDI/Automation feature lanes", status: "NotRun", reason: "feature implementations pending (#115-#134)" },
  { lane: "AudioLens consumer regression", status: "NotRun", reason: "ruviform #294 / audio-lens #142 ownership" },
];

// --- Classification ----------------------------------------------------------

function classifyFile(path) {
  for (const rule of PATH_RULES) {
    const m = path.match(rule.re);
    if (!m) continue;
    if (rule.crateGroup !== undefined) {
      const crate = m[rule.crateGroup];
      if (!(crate in CRATE_DEPS)) return { classes: [], trigger: true, unknown: `unknown crate dir '${crate}'` };
      const classes = [...(CRATE_CLASSES[crate] ?? [])];
      for (const fr of FILE_CLASS_RULES) {
        if (fr.re.test(path)) for (const c of fr.classes) if (!classes.includes(c)) classes.push(c);
      }
      return { classes, crate, trigger: false };
    }
    return { classes: rule.classes, trigger: rule.trigger ?? false };
  }
  return { classes: [], trigger: true, unknown: "no rule matched" };
}

/** Transitive dependents of `crates` plus the crates themselves. */
function affectedCrates(changed) {
  const affected = new Set(changed);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [crate, deps] of Object.entries(CRATE_DEPS)) {
      if (!affected.has(crate) && deps.some((d) => affected.has(d))) {
        affected.add(crate);
        grew = true;
      }
    }
  }
  return affected;
}

function legsOf(lane) {
  return LANE_LEGS.filter((l) => l.lane === lane);
}

function intersects(a, b) {
  for (const x of a) if (b.has(x)) return true;
  return false;
}

/**
 * Build the impact report. `files` is a list of repo-relative paths;
 * `opts.mode` is shadow|full|selective.
 */
export function buildImpactReport({ files, mode, base, head, event }) {
  const classFiles = new Map();
  const changedCrates = new Set();
  const fallbackReasons = [];
  for (const f of files) {
    const r = classifyFile(f);
    for (const c of r.classes) {
      const list = classFiles.get(c) ?? [];
      list.push(f);
      classFiles.set(c, list);
    }
    if (r.crate) changedCrates.add(r.crate);
    if (r.trigger) fallbackReasons.push(`${f}: ${r.unknown ?? `trigger class '${r.classes[0] ?? "unknown"}'`}`);
  }

  const classes = [...classFiles.keys()].sort();
  const affected = affectedCrates([...changedCrates]);
  const fallback = fallbackReasons.length > 0;
  const docsOnly = classes.length > 0 && classes.every((c) => c === "docs");

  // Lane requirements under selective semantics. `classify` always runs;
  // dep-guard guards the dependency contract the classifier itself relies on
  // and is required for every non-docs change. Trigger classes, fallback and
  // full mode collapse the expected set to every lane.
  const requiredLanes = new Set(["classify"]);
  if (!docsOnly || fallback || mode === "full") requiredLanes.add("dep-guard");
  if (fallback || mode === "full") {
    for (const l of LANE_LEGS) requiredLanes.add(l.lane);
  } else if (!docsOnly) {
    if (affected.size > 0) {
      requiredLanes.add("verify");
      requiredLanes.add("msrv-verify");
    }
    if (intersects(affected, RT_OWNERS)) requiredLanes.add("rt-verify");
    if (intersects(affected, GW_OWNERS)) requiredLanes.add("gw10-verify");
    if (classFiles.has("solidjs-gui")) requiredLanes.add("ui-verify");
  }

  const laneReasons = (name) => {
    if (requiredLanes.has(name.lane)) {
      if (fallback || mode === "full") return { expectation: "required", reason: fallback ? "full CI (trigger/fallback)" : "full CI (mode)" };
      if (name.lane === "classify") return { expectation: "required", reason: "impact gate input" };
      if (name.lane === "dep-guard") return { expectation: "required", reason: "dependency contract guard (always-on for non-docs changes)" };
      if (name.lane === "verify" || name.lane === "msrv-verify") return { expectation: "required", reason: `affected crates: ${[...affected].join(", ")}` };
      if (name.lane === "rt-verify") return { expectation: "required", reason: "rt_ contract owner affected (synvelis-project/synvelis-recording)" };
      if (name.lane === "gw10-verify") return { expectation: "required", reason: "GW-10 owner affected (synvelis-gateway/mcp-poc via dep graph)" };
      if (name.lane === "ui-verify") return { expectation: "required", reason: "ui/ (solidjs-gui) impact" };
    }
    return { expectation: "not-applicable", reason: docsOnly ? "docs-only change" : `no ${name.lane}-relevant impact (classes: ${classes.join(", ") || "none"})` };
  };

  const lanes = LANE_LEGS.map((leg) => {
    const lr = laneReasons(leg);
    // Issue #157: scheduling contract for ci.yml lane `if:` conditions.
    // selective mode schedules only `required` legs; shadow/full keep every
    // leg running so unauthorized `skipped` stays a gate violation.
    // Scheduling is lane-uniform: job-level `if` cannot see `matrix`, and
    // laneReasons is lane-scoped, so all OS legs of a lane agree.
    return { name: leg.name, lane: leg.lane, os: leg.os, ...lr, scheduled: mode !== "selective" || lr.expectation === "required" };
  });

  return {
    schema: SCHEMA,
    event: event ?? null,
    mode,
    selective_enabled: mode === "selective",
    base: base ?? null,
    head: head ?? null,
    changed_files: files.length,
    fallback_full_ci: fallback,
    fallback_reasons: fallbackReasons,
    classes: classes.map((id) => ({ id, files: classFiles.get(id) })),
    crates: { changed: [...changedCrates].sort(), affected: [...affected].sort() },
    lanes,
    declared_not_run: DECLARED_NOT_RUN,
    generated_at: new Date().toISOString(),
  };
}

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
 * Fail-closed evaluation of the expected verification set against the
 * workflow run's actual job results. Returns { ok, rows, violations, shadow }.
 */
export function evaluateGate({ impact, jobs, attempt }) {
  if (!impact || !Array.isArray(impact.lanes)) {
    return { ok: false, rows: [], violations: ["impact report missing or malformed — cannot evaluate the expected verification set"], shadow: [] };
  }
  const byName = new Map();
  for (const j of jobs ?? []) {
    const list = byName.get(j.name) ?? [];
    list.push(j);
    byName.set(j.name, list);
  }
  const expected = new Map(impact.lanes.map((l) => [l.name, l]));
  const selective = impact.selective_enabled === true;
  const violations = [];
  const rows = [];
  const shadow = [];

  for (const leg of LANE_LEGS) {
    const job = pickJob(byName.get(leg.name), attempt);
    const exp = expected.get(leg.name) ?? { expectation: "required", reason: "absent from impact report" };
    const required = selective ? exp.expectation === "required" : true;
    const verdict = jobVerdict(job);
    rows.push({ name: leg.name, expectation: exp.expectation, reason: exp.reason, verdict, job });

    // Shadow comparison: every leg the classifier would skip is recorded
    // against its real full-CI result, in every mode — this is the
    // false-negative audit trail that gates enabling selective execution.
    if (exp.expectation === "not-applicable") {
      shadow.push({
        name: leg.name,
        would_skip: true,
        actual: verdict,
        false_negative: verdict === "fail" || verdict === "cancelled",
      });
    }

    if (required) {
      if (verdict === "skipped") violations.push(`'${leg.name}' was skipped — required lanes may never skip (skipped is not Pass)`);
      else if (verdict !== "pass") violations.push(`'${leg.name}' expected Pass but was ${verdict} (job: ${job?.conclusion ?? "absent"})`);
    } else if (verdict === "fail" || verdict === "cancelled") {
      violations.push(`'${leg.name}' classified not-applicable but produced a real ${verdict} — classifier false negative`);
    }
  }

  // Issue #157 rev.2: non-blocking macOS track. Their real conclusions are
  // recorded as observed verdicts (pending = still queued/in progress or
  // absent from the API view). Never required, never a violation — but a
  // real failure or cancellation is surfaced, not hidden.
  const observed = OBSERVED_LEGS.map((leg) => {
    const job = pickJob(byName.get(leg.name), attempt);
    const v = jobVerdict(job);
    return { name: leg.name, lane: leg.lane, os: leg.os, verdict: v === "not-run" ? "pending" : v, job };
  });

  for (const name of byName.keys()) {
    const known = LANE_LEGS.some((l) => l.name === name) || OBSERVED_LEGS.some((l) => l.name === name) || name === GATE_JOB;
    if (known) continue;

    // Issue #157 rev.3: a job-level `if:` skip collapses a matrix lane into
    // a single placeholder job ('<lane> (${{ matrix.os }})') instead of
    // legs. It is legal only while the impact report scheduled nothing in
    // that lane; a placeholder for a scheduled lane is the same violation
    // as a skipped required leg. Unknown prefixes stay unclassified.
    const ph = typeof name === "string" ? name.match(PLACEHOLDER_NAME_RE) : null;
    if (!ph) {
      violations.push(`unclassified job '${name}' in this run — add it to LANE_LEGS/OBSERVED_LEGS in tools/ci-impact.mjs`);
      continue;
    }
    const prefix = ph[1].trim();
    if (!LANE_LEGS.some((l) => l.lane === prefix)) {
      violations.push(`unclassified job '${name}' in this run — add it to LANE_LEGS/OBSERVED_LEGS in tools/ci-impact.mjs`);
      continue;
    }
    const phJob = pickJob(byName.get(name), attempt);
    // Attempt-scoped: a placeholder recorded in an earlier attempt is stale
    // once a later attempt actually expanded the lane's legs (e.g. a re-run
    // after CI_SELECTIVE_DISABLE flipped the scheduling). It records only
    // history then — not evidence of a skip in the evaluated attempt.
    const superseded = LANE_LEGS.filter((l) => l.lane === prefix)
      .flatMap((l) => byName.get(l.name) ?? [])
      .some((e) => (e.run_attempt ?? 1) > (phJob?.run_attempt ?? 1));
    if (superseded) continue;
    const reportLegs = impact.lanes.filter((l) => l?.lane === prefix);
    const laneUnscheduled = reportLegs.length > 0 && reportLegs.every((l) => l?.scheduled === false);
    if (!laneUnscheduled) {
      violations.push(`'${name}' is a skip-collapsed matrix placeholder for lane '${prefix}', but that lane has required/scheduled legs — required lanes may never skip (skipped is not Pass)`);
    } else if (jobVerdict(phJob) !== "skipped") {
      // The placeholder is by definition a record of a skip — any other
      // conclusion means the API semantics drifted; fail closed rather
      // than accept a record we no longer understand.
      violations.push(`'${name}' is a placeholder for unscheduled lane '${prefix}' but has conclusion '${phJob?.conclusion ?? "absent"}' (expected skipped)`);
    } else {
      rows.push({
        name,
        expectation: reportLegs[0]?.expectation ?? "not-applicable",
        reason: `lane '${prefix}' not scheduled — skip-collapsed matrix placeholder`,
        verdict: "skipped-placeholder",
        job: phJob,
      });
    }
  }

  return { ok: violations.length === 0, rows, violations, shadow, observed };
}

/**
 * Expected jobs-API entry count for a run under `impact`: scheduled lanes
 * expand to one entry per leg, each skipped lane collapses to a single
 * entry (a matrix placeholder or a normal-named skipped job), the observed
 * macOS jobs always produce one entry each, and `evidence` itself is one.
 * Missing/malformed reports fall back to the full universe — over-expecting
 * only costs bounded retries, under-expecting can hide absent jobs.
 */
export function expectedJobs(impact) {
  const lanes = Array.isArray(impact?.lanes) ? impact.lanes : null;
  // Only a report covering the full lane universe is trustworthy — an empty
  // or partial `lanes` array is semantically malformed and falls back to
  // the same full-universe count as an unreadable report.
  const wellFormed = lanes !== null && LANE_LEGS.every((leg) => lanes.some((l) => l?.name === leg.name));
  if (!wellFormed) return LANE_LEGS.length + OBSERVED_LEGS.length + 1;
  const scheduledLegs = lanes.filter((l) => l?.scheduled !== false).length;
  const unscheduledLanes = new Set(lanes.filter((l) => l?.scheduled === false).map((l) => l?.lane)).size;
  return scheduledLegs + unscheduledLanes + OBSERVED_LEGS.length + 1;
}

// --- Reporting ---------------------------------------------------------------

function mdEscape(s) {
  return String(s).replaceAll("|", "\\|");
}

function classifyMarkdown(report) {
  const lines = [];
  lines.push(`# CI impact classification — ${report.event ?? "local"}`);
  lines.push("");
  lines.push(`- mode: **${report.mode}** (selective execution ${report.selective_enabled ? "enabled" : "disabled — shadow/full still runs all lanes"})`);
  if (report.base?.sha || report.head?.sha) lines.push(`- range: \`${report.base?.sha ?? "?"}\` ... \`${report.head?.sha ?? "?"}\``);
  lines.push(`- changed files: ${report.changed_files}`);
  if (report.fallback_full_ci) {
    lines.push(`- **fallback to full CI**: ${report.fallback_reasons.map(mdEscape).join("; ")}`);
  }
  lines.push("");
  lines.push(`| class | files |`);
  lines.push(`|---|---|`);
  for (const c of report.classes) lines.push(`| ${c.id} | ${c.files.length} |`);
  lines.push("");
  lines.push(`- crates changed: ${report.crates.changed.join(", ") || "none"}`);
  lines.push(`- crates affected (dep propagation): ${report.crates.affected.join(", ") || "none"}`);
  lines.push("");
  lines.push(`| lane leg | expectation | reason |`);
  lines.push(`|---|---|---|`);
  for (const l of report.lanes) lines.push(`| ${l.name} | ${l.expectation} | ${mdEscape(l.reason)} |`);
  return lines.join("\n");
}

function gateMarkdown({ verdict, impact, context }) {
  const lines = [];
  lines.push(`# CI evidence — ${context.event} ${context.ref}`);
  lines.push("");
  lines.push(`- run: ${context.runUrl} (attempt ${context.attempt})`);
  lines.push(`- commit: ${context.headSha}`);
  if (context.baseSha) lines.push(`- base: ${context.baseSha}`);
  lines.push(`- ruviform resolved SHA: ${context.ruviformSha}`);
  lines.push(`- impact mode: ${impact.mode} (selective ${impact.selective_enabled ? "enabled" : "disabled"})${impact.fallback_full_ci ? " — fallback full CI" : ""}`);
  lines.push(`- gate verdict: **${verdict.ok ? "pass" : "FAIL"}**`);
  lines.push("");
  lines.push(`| lane leg | expected | actual | verdict |`);
  lines.push(`|---|---|---|---|`);
  for (const r of verdict.rows) {
    const job = r.job ? `${r.job.status}/${r.job.conclusion ?? "-"}` : "absent";
    const mark = r.expectation === "required" ? "required" : `not-applicable (${mdEscape(r.reason)})`;
    lines.push(`| ${r.name} | ${mark} | ${job} | ${r.verdict} |`);
  }
  const shadowRows = verdict.shadow ?? [];
  if (shadowRows.length > 0) {
    lines.push("");
    lines.push(`## Shadow comparison (classifier vs full CI — false-negative audit)`);
    lines.push("");
    lines.push(`| lane leg | would skip | actual | result |`);
    lines.push(`|---|---|---|---|`);
    for (const s of shadowRows) {
      lines.push(`| ${s.name} | yes | ${s.actual} | ${s.false_negative ? "**FALSE NEGATIVE**" : "consistent"} |`);
    }
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
  for (const d of impact.declared_not_run ?? DECLARED_NOT_RUN) lines.push(`| ${mdEscape(d.lane)} | ${d.status} | ${mdEscape(d.reason)} |`);
  const totalMs = verdict.rows.reduce((acc, r) => acc + (durationMs(r.job) ?? 0), 0);
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

function changedFilesFromGit(base, head) {
  for (const spec of [`${base}...${head}`, `${base} ${head}`]) {
    try {
      const out = execFileSync("git", ["diff", "--name-only", ...spec.split(" ")], { encoding: "utf8" });
      return { files: out.split("\n").map((s) => s.trim()).filter(Boolean), error: null };
    } catch (e) {
      continue;
    }
  }
  return { files: null, error: `git diff ${base}...${head} failed` };
}

function resolveMode(args) {
  const m = args.mode ?? process.env.CI_IMPACT_MODE;
  if (m) return m;
  const event = args.event ?? process.env.GITHUB_EVENT_NAME;
  return event === "pull_request" ? "shadow" : "full";
}

function main() {
  const [cmd, ...rest] = process.argv.slice(2);
  const args = parseArgs(rest);

  if (cmd === "lanes") {
    // Every job name the run can produce besides the gate itself — the full
    // job universe (required legs + observed macOS track). Diagnostic aid;
    // the evidence collect step sizes the expected payload via
    // `expected-jobs`, which accounts for skip-collapsed lanes.
    for (const l of [...LANE_LEGS, ...OBSERVED_LEGS]) process.stdout.write(`${l.name}\n`);
    return;
  }

  if (cmd === "expected-jobs") {
    // Exact jobs-API entry count for this run's impact report — the
    // evidence collect step retries until it has this many entries.
    const impactPath = args.impact ?? process.env.CI_IMPACT_REPORT_FILE;
    let impact = null;
    try {
      impact = JSON.parse(readFileSync(impactPath, "utf8"));
    } catch {
      impact = null;
    }
    process.stdout.write(`${expectedJobs(impact)}\n`);
    return;
  }

  if (cmd === "classify") {
    const mode = resolveMode(args);
    const base = args.base ?? process.env.CI_IMPACT_BASE ?? null;
    const head = args.head ?? process.env.CI_IMPACT_HEAD ?? null;
    const filesFile = args["files-file"] ?? process.env.CI_IMPACT_FILES_FILE;
    let files = [];
    let diffError = null;
    if (filesFile) {
      files = readFileSync(filesFile, "utf8").split("\n").map((s) => s.trim()).filter(Boolean);
    } else if (base && head) {
      const r = changedFilesFromGit(base, head);
      if (r.files === null) diffError = r.error;
      else files = r.files;
    }
    const report = buildImpactReport({
      files, mode, event: args.event ?? process.env.GITHUB_EVENT_NAME ?? null,
      base: base ? { sha: base } : null, head: head ? { sha: head } : null,
    });
    if (diffError) {
      report.fallback_full_ci = true;
      report.fallback_reasons.push(diffError);
      for (const l of report.lanes) {
        l.expectation = "required";
        l.reason = "full CI (diff unavailable)";
        l.scheduled = true;
      }
    }
    const json = JSON.stringify(report, null, 2);
    const out = args.out ?? process.env.CI_IMPACT_OUT;
    if (out) writeFileSync(out, json + "\n");
    else process.stdout.write(json + "\n");
    const md = classifyMarkdown(report);
    const summaryPath = args.summary ?? process.env.GITHUB_STEP_SUMMARY;
    if (summaryPath) appendFileSync(summaryPath, md + "\n");
    const setOutput = args["set-output"] ?? process.env.CI_IMPACT_SET_OUTPUT;
    if (setOutput && process.env.GITHUB_OUTPUT) {
      // Slim variant for job outputs: gate only needs lanes/mode/declared
      // not-run; per-class file lists are dropped to bound the payload.
      const slim = { ...report, classes: report.classes.map((c) => ({ id: c.id, count: c.files.length })) };
      appendFileSync(process.env.GITHUB_OUTPUT, `${setOutput}=${JSON.stringify(slim)}\n`);
    }
    const lanesOutput = args["lanes-output"] ?? process.env.CI_IMPACT_LANES_OUTPUT;
    if (lanesOutput && process.env.GITHUB_OUTPUT) {
      // Lane ids to schedule this run (JSON array). ci.yml lane `if:`
      // conditions membership-test their own lane id against this list.
      const lanes = [...new Set(report.lanes.filter((l) => l.scheduled === true).map((l) => l.lane))].sort();
      appendFileSync(process.env.GITHUB_OUTPUT, `${lanesOutput}=${JSON.stringify(lanes)}\n`);
    }
    return;
  }

  if (cmd === "gate") {
    const impactPath = args.impact ?? process.env.CI_IMPACT_REPORT_FILE;
    const jobsPath = args.jobs ?? process.env.CI_JOBS_FILE;
    let impact = null;
    try {
      const raw = readFileSync(impactPath, "utf8");
      impact = raw.trim().startsWith("{") ? JSON.parse(raw) : null;
    } catch {
      impact = null;
    }
    let jobs = [];
    try {
      const raw = JSON.parse(readFileSync(jobsPath, "utf8"));
      jobs = Array.isArray(raw) ? raw : raw.jobs;
    } catch {
      jobs = [];
    }
    const attempt = Number(args.attempt ?? process.env.CI_RUN_ATTEMPT ?? 0) || undefined;
    const verdict = evaluateGate({ impact, jobs, attempt });
    const context = {
      event: process.env.GITHUB_EVENT_NAME ?? "local",
      ref: process.env.GITHUB_REF_NAME ?? "",
      runUrl: process.env.CI_RUN_URL ?? `run ${process.env.GITHUB_RUN_ID ?? "?"}`,
      attempt: attempt ?? "?",
      headSha: process.env.CI_HEAD_SHA ?? process.env.GITHUB_SHA ?? "?",
      baseSha: process.env.CI_BASE_SHA ?? null,
      ruviformSha: process.env.CI_RUVIFORM_SHA ?? "n/a",
    };
    const md = gateMarkdown({ verdict, impact: impact ?? { mode: "unknown", lanes: [] }, context });
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
    process.stdout.write("ci-impact gate: all expected lanes passed; evidence recorded\n");
    return;
  }

  process.stderr.write("usage: ci-impact.mjs classify|gate|lanes|expected-jobs — see file header\n");
  process.exit(2);
}

const isMain = process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (isMain) main();
