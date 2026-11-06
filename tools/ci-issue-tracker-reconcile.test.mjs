// Consumer-side contract test for the issue-tracker reconcile entry.
// Pins the workflow-level contract (shared pin / mode separation / guard /
// entry triggers / unchanged event + daily + ci triggers) as structural
// text assertions — no YAML parser dependency. The shared evaluator's own
// judgement contract lives in Ruviform's ci/tracker tests (91 mock tests);
// this suite only asserts that Synvelis invokes that implementation
// correctly.
//
// Run: mise run test-ci-tracker-entry

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SHARED_PIN = "eed590e0a3837e61c943d9de1d3efcfd7df32cb0";
const OLD_PIN = "73d51d875e2876a75fb2077c471ea8d04478c082";
const SHARED_PATH = ".ci-shared/ruviform";
const CONCURRENCY_GROUP = "ci-issue-tracker-${{ github.repository }}";

function wf(name) {
  // Normalize CRLF so structural regexes do not depend on checkout EOL.
  return readFileSync(join(ROOT, ".github", "workflows", name), "utf8")
    .replaceAll("\r\n", "\n");
}

// Extract the `on:` block up to the next top-level key (a non-indented,
// non-comment line). Keeps assertions scoped to the trigger section.
function onBlock(text) {
  const m = text.match(/^on:\n((?:[ \t].*\n|#.*\n|\s*\n)*)/m);
  assert.ok(m, "workflow must contain an `on:` block");
  return m[1];
}

// Extract the shared `scarecrow173/ruviform` checkout `with:` mapping for
// pin assertions. Bounded by the indentation of the `repository:` key —
// sibling keys share that indent; a shallower line ends the mapping.
function sharedCheckoutBlock(text) {
  const m = text.match(
    /^(\s+)repository:\s*scarecrow173\/ruviform\n((?:\1\S[^\n]*\n)+)/m,
  );
  assert.ok(m, "must check out scarecrow173/ruviform");
  return m[2];
}

const daily = wf("daily-ci.yml");
const tracker = wf("ci-issue-tracker.yml");
const reconcile = wf("ci-issue-tracker-reconcile.yml");
const ci = wf("ci.yml");
const workflowFiles = readdirSync(join(ROOT, ".github", "workflows")).filter(
  (f) => /\.ya?ml$/.test(f),
);

test("shared pin: all three workflows pin ruviform checkout to eed590e", () => {
  for (const [name, text] of [
    ["daily-ci.yml", daily],
    ["ci-issue-tracker.yml", tracker],
    ["ci-issue-tracker-reconcile.yml", reconcile],
  ]) {
    const block = sharedCheckoutBlock(text);
    assert.match(
      block,
      new RegExp(`ref:\\s*${SHARED_PIN}`),
      `${name} must pin shared checkout to ${SHARED_PIN}`,
    );
    assert.match(
      block,
      new RegExp(`path:\\s*${SHARED_PATH.replaceAll("/", "\\/")}`),
      `${name} must check shared impl out to ${SHARED_PATH}`,
    );
    assert.match(
      block,
      /ssh-key:\s*\$\{\{\s*secrets\.CROSS_REPO_DEPLOY_KEY\s*\}\}/,
      `${name} must reuse CROSS_REPO_DEPLOY_KEY (no new secrets)`,
    );
    assert.match(
      block,
      /persist-credentials:\s*false/,
      `${name} shared checkout must not persist credentials`,
    );
  }
});

test("shared pin: old 73d51d8 pin is gone from every workflow", () => {
  assert.ok(workflowFiles.length > 0, "workflow directory must be non-empty");
  for (const name of workflowFiles) {
    assert.ok(
      !wf(name).includes(OLD_PIN),
      `${name} must not retain the old shared pin`,
    );
  }
});

test("reconcile entry: triggers are schedule + workflow_dispatch only", () => {
  const on = onBlock(reconcile);
  assert.match(on, /schedule:/);
  assert.match(on, /cron:\s*"47 \*\/6 \* \* \*"/);
  assert.match(on, /workflow_dispatch:/);
  assert.doesNotMatch(on, /pull_request/);
  assert.doesNotMatch(on, /^\s+push:/m);
  assert.doesNotMatch(on, /workflow_run/);
});

test("reconcile entry: same concurrency group as the event tracker", () => {
  for (const [name, text] of [
    ["ci-issue-tracker-reconcile.yml", reconcile],
    ["ci-issue-tracker.yml", tracker],
  ]) {
    assert.match(
      text,
      new RegExp(
        `group:\\s*${CONCURRENCY_GROUP.replace(/[$.*{}]/g, (c) => `\\${c}`)}`,
      ),
      `${name} must share the tracker concurrency group`,
    );
    assert.match(text, /cancel-in-progress:\s*false/);
  }
});

test("reconcile entry: schedule runs unconditionally, manual non-default ref skips", () => {
  assert.match(
    reconcile,
    /if:\s*github\.event_name == 'schedule' \|\| github\.ref_name == github\.event\.repository\.default_branch/,
    "job guard must pass schedule runs and skip non-default-ref dispatch",
  );
  // The schedule clause is unconditional so the guard never depends on
  // the schedule payload shape — payloads have changed historically
  // (`repository` was only added in 2022-09 and is absent on older GHES),
  // where a ref comparison would resolve empty and silently skip every
  // cron tick. The ref comparison exists for dispatch only.
  assert.doesNotMatch(
    reconcile,
    /github\.event_name == 'workflow_dispatch'/,
    "guard must not gate on workflow_dispatch (schedule needs its own clause)",
  );
  assert.match(reconcile, /timeout-minutes:\s*15/);
});

test("reconcile entry: least privilege — job perms match track job, no new secrets", () => {
  assert.match(reconcile, /^permissions:\s*\{\}/m, "deny by default at workflow level");
  const perms = reconcile.match(/permissions:\s*\n(\s+\w+:\s*\w+\n)+/g) ?? [];
  const jobPerms = perms.find((p) => p.includes("issues: write"));
  assert.ok(jobPerms, "job must declare permissions");
  for (const line of ["contents: read", "actions: read", "issues: write"]) {
    assert.ok(jobPerms.includes(line), `job permissions must include ${line}`);
  }
  assert.doesNotMatch(
    reconcile,
    /secrets\.(?!GITHUB_TOKEN|CROSS_REPO_DEPLOY_KEY)[A-Z_]+/,
    "must not reference secrets beyond GITHUB_TOKEN and CROSS_REPO_DEPLOY_KEY",
  );
});

test("reconcile entry: trusted default-branch checkout of own repo", () => {
  const withBlocks = [
    ...reconcile.matchAll(/with:\n((?:\s+\S[^\n]*\n)+)/g),
  ].map((m) => m[1]);
  const ownWith = withBlocks.find((b) =>
    b.includes("github.event.repository.default_branch"),
  );
  assert.ok(
    ownWith,
    "own-repo checkout must pin the default branch with explicit fallback",
  );
  assert.match(
    ownWith,
    /ref:\s*\$\{\{\s*github\.event\.repository\.default_branch \|\| 'main'\s*\}\}/,
    "own-repo checkout must pin the default branch",
  );
  assert.match(
    ownWith,
    /persist-credentials:\s*false/,
    "own-repo checkout must not persist credentials",
  );
});

test("reconcile entry: mode separation — no event path, explicit --reconcile", () => {
  assert.doesNotMatch(
    reconcile,
    /^\s*TRACKER_EVENT_PATH\s*:/m,
    "reconcile must not receive an event path env key",
  );
  assert.match(
    reconcile,
    /run:\s*mise exec -- node \.ci-shared\/ruviform\/ci\/tracker\/tracker\.mjs --reconcile --report ci-issue-tracker-report\.json/,
    "must invoke the shared evaluator in reconcile mode with explicit report",
  );
  // The reconcile step's env must be GH_TOKEN only — no event path or
  // other ambient mode leaks into the evaluator.
  const envBlock = reconcile.match(
    /env:\n((?:(?:\s+\w+:[^\n]*|\s+#[^\n]*)\n)+)\s*run:\s*mise exec -- node \.ci-shared\/ruviform\/ci\/tracker\/tracker\.mjs --reconcile/,
  );
  assert.ok(envBlock, "reconcile step must declare an env block");
  const envKeys = [...envBlock[1].matchAll(/^\s+(\w+):/gm)].map((m) => m[1]);
  assert.deepEqual(
    envKeys,
    ["GH_TOKEN"],
    "reconcile step env must contain GH_TOKEN only",
  );
  assert.match(
    envBlock[1],
    /GH_TOKEN:\s*\$\{\{\s*secrets\.GITHUB_TOKEN\s*\}\}/,
    "GH_TOKEN must come from secrets.GITHUB_TOKEN",
  );
});

test("reconcile entry: shared evaluator self-test runs before the reconcile", () => {
  const selfTest = reconcile.indexOf(
    "run: mise exec -- node --test .ci-shared/ruviform/ci/tracker/",
  );
  const run = reconcile.indexOf(
    "run: mise exec -- node .ci-shared/ruviform/ci/tracker/tracker.mjs --reconcile",
  );
  assert.ok(selfTest !== -1, "shared tracker self-test step must exist");
  assert.ok(run !== -1, "reconcile run step must exist");
  assert.ok(
    selfTest < run,
    "self-test must precede the reconcile run (fail-closed on broken evaluator)",
  );
  // Shared revision is recorded for diagnosis of which pin actually ran.
  assert.match(reconcile, /git -C \.ci-shared\/ruviform rev-parse HEAD/);
});

test("reconcile entry: report artifact is uploaded under a distinct name", () => {
  assert.match(
    reconcile,
    /name:\s*ci-issue-tracker-reconcile-report-attempt-\$\{\{ github\.run_attempt \}\}/,
  );
  assert.match(reconcile, /path:\s*ci-issue-tracker-report\.json/);
  assert.match(reconcile, /if:\s*always\(\)/);
});

test("event entry unchanged: ci-issue-tracker.yml stays workflow_run-only", () => {
  const on = onBlock(tracker);
  assert.match(on, /workflow_run:/);
  assert.doesNotMatch(on, /schedule:/);
  assert.doesNotMatch(on, /workflow_dispatch:/);
  assert.doesNotMatch(on, /pull_request/);
  assert.doesNotMatch(on, /^\s+push:/m);
  assert.match(
    tracker,
    /TRACKER_EVENT_PATH:\s*\$\{\{ github\.event_path \}\}/,
    "event entry keeps passing the event payload",
  );
  assert.doesNotMatch(
    tracker,
    /--reconcile/,
    "event entry must not invoke reconcile mode",
  );
});

test("daily entry unchanged: no tracker responsibilities mixed in", () => {
  assert.doesNotMatch(daily, /tracker\.mjs/);
  assert.doesNotMatch(daily, /--reconcile/);
});

test("ci.yml is workflow_dispatch-only (Issue #145 trigger removal)", () => {
  const on = onBlock(ci);
  assert.match(on, /workflow_dispatch:/);
  for (const banned of [
    /pull_request/,
    /pull_request_target/,
    /^\s+push:/m,
    /schedule:/,
    /workflow_run/,
  ]) {
    assert.doesNotMatch(
      on,
      banned,
      `ci.yml must not carry trigger matching ${banned}`,
    );
  }
  // The expected_sha input is the receive-side of daily-ci's dispatch
  // contract — it must survive trigger removal.
  assert.match(on, /expected_sha:/);
  assert.match(on, /required:\s*false/);
});

test("no workflow retains PR/push triggers (negative scan)", () => {
  for (const name of workflowFiles) {
    const on = onBlock(wf(name));
    for (const banned of [/pull_request/, /pull_request_target/, /^\s+push:/m]) {
      assert.doesNotMatch(
        on,
        banned,
        `${name} must not carry trigger matching ${banned}`,
      );
    }
  }
});

test("ci.yml lane structure and fail-closed gates are unchanged", () => {
  for (const job of [
    "verify",
    "verify-macos",
    "rt-verify",
    "gw10-verify",
    "gw10-verify-macos",
    "msrv-verify",
    "ui-verify",
    "ui-verify-macos",
    "dep-guard",
    "evidence",
    "ci-summary",
  ]) {
    assert.match(
      ci,
      new RegExp(`^  ${job}:\\n`, "m"),
      `job ${job} must still exist`,
    );
  }
  // Aggregation modes: evidence keeps the non-macOS pr-mode gate,
  // ci-summary keeps the full-universe verdict including evidence.
  assert.match(ci, /CI_SUMMARY_MODE:\s*pr/);
  assert.match(ci, /CI_SUMMARY_MODE:\s*full/);
  assert.match(ci, /needs:\s*\[[^\]]*evidence[^\]]*\]/);
  // expected_sha checkout pin + verify-sha contract must survive.
  assert.match(
    ci,
    /ref:\s*\$\{\{\s*inputs\.expected_sha \|\| github\.sha\s*\}\}/,
  );
  assert.match(ci, /ci-summary\.mjs verify-sha/);
});

test("mise task entry point exists", () => {
  const mise = readFileSync(join(ROOT, "mise.toml"), "utf8")
    .replaceAll("\r\n", "\n");
  assert.match(
    mise,
    /\[tasks\.test-ci-tracker-entry\]/,
    "mise.toml must define tasks.test-ci-tracker-entry",
  );
});
