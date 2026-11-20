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
// Verdict discipline (verify-synvelis evidence contract):
//   - exit 0 + verdict `pass`        — verification ran and succeeded
//   - exit 0 + verdict `not_run`     — no usable environment
//     (`environment_blocked`), never a fake pass
//   - exit 1 + verdict `fail`        — verification ran and FAILED
//     (fail-closed; a real defect or unwinding-assertion violation)
//
// Provision runbook: docs/specs/formal-verification.md.
// Invoked via `mise run test-formal-kani`. Portable Node — no POSIX
// text tools on the host side (WSL-side shell is bash inside the
// distro only).

import { spawnSync } from "node:child_process";

const LANE = "test-formal-kani";
const PACKAGES = ["synvelis-gateway", "synvelis-recording"];
// Parallel CBMC solver processes each hold several GiB — on shared or
// small-RAM hosts, run e.g. `KANI_JOBS=2 mise run test-formal-kani`
// (or =1 for fully serial) to cap peak memory instead of lowering
// proof bounds. Default -j4 matches the rf806 precedent.
const KANI_JOBS = /^[1-9]\d*$/.test(process.env.KANI_JOBS ?? "")
  ? process.env.KANI_JOBS
  : "4";
const KANI_ARGS = [
  "kani",
  "--tests",
  `-j${KANI_JOBS}`,
  "--output-format",
  "terse",
  ...PACKAGES.flatMap((p) => ["-p", p]),
];

function verdict(v, reason) {
  process.stdout.write(
    `${JSON.stringify({ lane: LANE, verdict: v, reason })}\n`,
  );
}

// run() inherits stdio (the kani output IS the evidence); probe() is
// silent. Both return false when the binary does not exist.
function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  return !r.error && r.status === 0;
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

const provisionHint =
  "provision Kani per docs/specs/formal-verification.md " +
  "(rustup + cargo install --locked kani-verifier + cargo kani setup)";

// 1) native cargo kani (Linux/macOS hosts, or a host where kani runs
//    natively)
if (probe("cargo", ["kani", "--version"])) {
  process.stdout.write(`[${LANE}] lane=native cargo kani\n`);
  if (run("cargo", KANI_ARGS)) {
    verdict("pass", "native cargo kani verification succeeded");
    process.exit(0);
  }
  verdict("fail", "native cargo kani verification failed");
  process.exit(1);
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
      );
      process.exit(0);
    }
    process.stdout.write(`[${LANE}] lane=WSL2 distro=${distro} repo=${repoWsl}\n`);
    const remote =
      `cd '${repoWsl}' && ` +
      `CARGO_TARGET_DIR="$HOME/.cache/synvelis-kani/target" ` +
      `cargo ${KANI_ARGS.map((a) => (a.includes(" ") ? `'${a}'` : a)).join(" ")}`;
    if (run("wsl.exe", ["-d", distro, "--", "bash", "-lc", remote])) {
      verdict("pass", `WSL2 (${distro}) cargo kani verification succeeded`);
      process.exit(0);
    }
    verdict("fail", `WSL2 (${distro}) cargo kani verification failed`);
    process.exit(1);
  }

  verdict(
    "not_run",
    `environment_blocked: wsl.exe present but no WSL2 distro has cargo kani — ${provisionHint}`,
  );
  process.exit(0);
}

verdict(
  "not_run",
  `environment_blocked: no native cargo kani and no wsl.exe — ${provisionHint}`,
);
process.exit(0);
