// Issue #601 D-layer Kani bounded-proof lane runner.
//
// Runs `cargo kani --tests` over the `#[cfg(kani)]` proof harnesses
// co-located with the slice-1 proptest suites
// (`crates/synvelis-gateway/tests/formal_journal.rs` +
// `crates/synvelis-recording/tests/formal_journal.rs`).
//
// Kani needs a Linux CBMC toolchain, so the lane env-detects:
//   1) a native `cargo kani` (Linux/macOS hosts),
//   2) then a WSL2 distro that has cargo + cargo kani (Kani cannot run
//      on native Windows). In WSL the repository is reached via
//      /mnt/<drive>, but CARGO_TARGET_DIR is forced onto the WSL native
//      filesystem — /mnt builds are far too slow, and the Windows-side
//      cargo output dir must stay untouched.
//
// Selective execution (Issue #659 slice 2):
//   - KANI_PACKAGE=synvelis-gateway|synvelis-recording selects one `-p`
//     package. When unset, both packages keep the original full-lane shape.
//   - KANI_HARNESS=<filter> is passed once as `cargo kani --harness`.
//     It is Kani's own filter value; this runner adds no multi-filter,
//     glob, regexp, or exact-match grammar of its own.
//
// Verdict discipline (verify-synvelis evidence contract):
//   - exit 0 + verdict `pass`        — verification ran and succeeded
//   - exit 0 + verdict `not_run`     — no usable environment
//     (`environment_blocked`), never a fake pass
//   - exit 1 + verdict `fail`        — invocation failed closed
//     (verification failure, zero matched harnesses, or infrastructure
//     failure; classify the cause from raw output/evidence)
//   - exit 2 + verdict `fail`        — invalid selective-run configuration
//     (`configuration_error`), before any verification starts
//
// Provision runbook: docs/specs/formal-verification.md.
// Invoked via `mise run test-formal-kani`. Portable Node — no POSIX
// text tools on the host side (WSL-side shell is bash inside the
// distro only).

import { spawnSync } from "node:child_process";

const LANE = "test-formal-kani";
const ALL_PACKAGES = ["synvelis-gateway", "synvelis-recording"];

function hasEnv(name) {
  return Object.prototype.hasOwnProperty.call(process.env, name);
}

let selectedPackages = ALL_PACKAGES;
let selectedHarness = null;

// Parallel CBMC solver processes each hold several GiB — on shared or
// small-RAM hosts, run e.g. `KANI_JOBS=2 mise run test-formal-kani`
// (or =1 for fully serial) to cap peak memory instead of lowering
// proof bounds. Default -j4 matches the rf806 precedent.
const KANI_JOBS = /^[1-9]\d*$/.test(process.env.KANI_JOBS ?? "")
  ? process.env.KANI_JOBS
  : "4";

function configurationError(message) {
  verdict("fail", `configuration_error: ${message}`, {
    environment: null,
    elapsedSeconds: 0,
    exitCode: null,
    harnesses: [],
  });
  process.exit(2);
}

if (hasEnv("KANI_PACKAGE")) {
  const raw = process.env.KANI_PACKAGE ?? "";
  if (!ALL_PACKAGES.includes(raw)) {
    configurationError(
      `KANI_PACKAGE must be exactly one of ${ALL_PACKAGES.join(", ")} ` +
        `when set; got ${JSON.stringify(raw)}`,
    );
  }
  selectedPackages = [raw];
}

if (hasEnv("KANI_HARNESS")) {
  const raw = process.env.KANI_HARNESS ?? "";
  if (raw.trim().length === 0) {
    configurationError("KANI_HARNESS must not be empty or whitespace-only");
  }
  selectedHarness = raw;
}

const KANI_ARGS = [
  "kani",
  "--tests",
  `-j${KANI_JOBS}`,
  "--output-format",
  "terse",
  ...(selectedHarness ? ["--harness", selectedHarness] : []),
  ...selectedPackages.flatMap((p) => ["-p", p]),
];

function verdict(v, reason, details = {}) {
  const payload = {
    lane: LANE,
    verdict: v,
    reason,
    ...(selectedPackages.length === 1 ? { package: selectedPackages[0] } : {}),
    packages: selectedPackages,
    harness: selectedHarness,
    jobs: Number(KANI_JOBS),
    environment: details.environment ?? null,
    elapsed_seconds: details.elapsedSeconds ?? 0,
    exit_code: details.exitCode ?? null,
    harnesses_matched: details.harnesses?.length ?? 0,
    harnesses: details.harnesses ?? [],
  };
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

function probe(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: "ignore", ...opts });
  return !r.error && r.status === 0;
}

function output(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, {
    stdio: ["ignore", "pipe", "ignore"],
    encoding: "utf8",
    ...opts,
  });
  return r.error ? "" : (r.stdout ?? "");
}

function shellQuote(s) {
  return `'${String(s).replace(/'/g, "'\\''")}'`;
}

function parseHarnesses(text) {
  const names = [];
  for (const match of text.matchAll(/Checking harness\s+([^\s]+?)\.\.\./g)) {
    if (!names.includes(match[1])) names.push(match[1]);
  }
  return names;
}

// runVerification tees captured stdout/stderr back to this process after
// completion. Capture lets the runner fail closed if a selective harness
// filter matched zero proof harnesses even when a future Kani version
// exits successfully for that input.
function runVerification(cmd, args, opts = {}) {
  const start = process.hrtime.bigint();
  const r = spawnSync(cmd, args, {
    stdio: ["ignore", "pipe", "pipe"],
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    ...opts,
  });
  const elapsedSeconds =
    Number(process.hrtime.bigint() - start) / 1_000_000_000;
  if (r.stdout) process.stdout.write(r.stdout);
  if (r.stderr) process.stderr.write(r.stderr);
  const text = `${r.stdout ?? ""}\n${r.stderr ?? ""}`;
  return {
    ok: !r.error && r.status === 0,
    exitCode: r.status,
    elapsedSeconds: Number(elapsedSeconds.toFixed(3)),
    harnesses: parseHarnesses(text),
  };
}

function finishRun(environment, result, successReason, failureReason) {
  const details = {
    environment,
    elapsedSeconds: result.elapsedSeconds,
    exitCode: result.exitCode,
    harnesses: result.harnesses,
  };
  if (result.ok && (!selectedHarness || result.harnesses.length > 0)) {
    verdict("pass", successReason, details);
    process.exit(0);
  }
  const zeroMatch =
    result.ok && selectedHarness && result.harnesses.length === 0;
  verdict(
    "fail",
    zeroMatch
      ? `${failureReason}: harness filter matched zero proof harnesses`
      : failureReason,
    details,
  );
  process.exit(1);
}

const provisionHint =
  "provision Kani per docs/specs/formal-verification.md " +
  "(rustup + cargo install --locked kani-verifier + cargo kani setup)";

// 1) native cargo kani (Linux/macOS hosts, or a host where kani runs
//    natively)
if (probe("cargo", ["kani", "--version"])) {
  process.stdout.write(
    `[${LANE}] lane=native cargo kani packages=${selectedPackages.join(",")} ` +
      `harness=${selectedHarness ?? "<all>"} jobs=${KANI_JOBS}\n`,
  );
  const result = runVerification("cargo", KANI_ARGS);
  finishRun(
    { kind: "native" },
    result,
    "native cargo kani verification succeeded",
    "native cargo kani verification failed",
  );
}

// 2) WSL2: find a distro that has cargo + cargo kani. `wsl.exe -l -q`
//    emits UTF-16 unless WSL_UTF8=1 is set.
if (probe("wsl.exe", ["--version"])) {
  const listOut = output("wsl.exe", ["-l", "-q"], {
    env: { ...process.env, WSL_UTF8: "1" },
  });
  const distros = listOut
    .split(/\r?\n/)
    .map((d) => d.replace(/\0/g, "").trim())
    .filter((d) => d.length > 0 && !d.startsWith("docker-desktop"));

  let distro = "";
  for (const d of distros) {
    if (
      probe("wsl.exe", [
        "-d",
        d,
        "--",
        "bash",
        "-lc",
        "command -v cargo >/dev/null 2>&1 && cargo kani --version >/dev/null 2>&1",
      ])
    ) {
      distro = d;
      break;
    }
  }

  if (distro) {
    // wslpath takes a Windows path — but a backslash-separated argv
    // value gets its backslashes eaten by wsl.exe argument
    // translation, so pass the forward-slash form (`G:/...`), which
    // wslpath accepts equally.
    const repoWin = process.cwd().replace(/\\/g, "/");
    const repoWsl = output("wsl.exe", [
      "-d",
      distro,
      "--",
      "wslpath",
      "-a",
      repoWin,
    ])
      .replace(/\r?\n/g, "")
      .trim();
    if (!repoWsl.startsWith("/")) {
      verdict(
        "not_run",
        `environment_blocked: WSL distro ${distro} cannot map repo path ` +
          `${repoWin} — run the lane from a drive the distro automounts (/mnt/<letter>)`,
        { environment: { kind: "wsl2", distro }, harnesses: [] },
      );
      process.exit(0);
    }
    process.stdout.write(
      `[${LANE}] lane=WSL2 distro=${distro} repo=${repoWsl} ` +
        `packages=${selectedPackages.join(",")} ` +
        `harness=${selectedHarness ?? "<all>"} jobs=${KANI_JOBS}\n`,
    );
    const remote =
      `cd ${shellQuote(repoWsl)} && ` +
      `CARGO_TARGET_DIR="$HOME/.cache/synvelis-kani/target" ` +
      `cargo ${KANI_ARGS.map(shellQuote).join(" ")}`;
    const result = runVerification("wsl.exe", [
      "-d",
      distro,
      "--",
      "bash",
      "-lc",
      remote,
    ]);
    finishRun(
      { kind: "wsl2", distro },
      result,
      `WSL2 (${distro}) cargo kani verification succeeded`,
      `WSL2 (${distro}) cargo kani verification failed`,
    );
  }

  verdict(
    "not_run",
    `environment_blocked: wsl.exe present but no WSL2 distro has cargo kani — ${provisionHint}`,
    { environment: null, harnesses: [] },
  );
  process.exit(0);
}

verdict(
  "not_run",
  `environment_blocked: no native cargo kani and no wsl.exe — ${provisionHint}`,
  { environment: null, harnesses: [] },
);
process.exit(0);
