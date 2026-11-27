import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { win32 } from "node:path";

const [mode, abiSha, playerSha] = process.argv.slice(2);
assert(["--write", "--check"].includes(mode)
  && [abiSha, playerSha].every((sha) => /^[0-9a-f]{40}$/.test(sha ?? "")));

const read = (name) => readFileSync(new URL(name, import.meta.url), "utf8");
const summary = read("canonical-results.tsv").split(/\r?\n/).map((line) => line.split("\t"));
const prefix = "tim03-dynamic-tempo-undo-redo ";

function extract(command, osLabel, log) {
  assert(summary.some(([cmd, os, exit, kind, path]) =>
    cmd === command && os === osLabel && exit === "0"
    && kind === "canonical" && path === log), `No successful canonical receipt for ${log}`);
  const raw = read(log);
  assert.match(raw, /test result: ok\./);
  return raw.split(/\r?\n/).filter((line) => line.includes(prefix))
    .map((line) => line.slice(line.indexOf(prefix) + prefix.length));
}

const abiLog = "dynamic-child-provenance.log";
const abiRaw = read(abiLog);
const header = "tim03-child-provenance ";
assert(abiRaw.startsWith(header));
const run = JSON.parse(abiRaw.split(/\r?\n/, 1)[0].slice(header.length));
assert.equal(run.command, "mise run test-tim03-dynamic-musical-transport -- production_tempo_undo_redo");
assert.equal(run.tested_sha, abiSha);
assert.equal(run.exit_code, 0);
assert.equal(run.plugin_dir_override_present_at_launch, false);
assert.equal(run.os.architecture, "X64");
assert.equal(run.os.version.split(".").at(-1), run.os.build);
assert(Number.isInteger(run.os.update_build_revision) && run.os.display_version);
const osLabel = `${run.os.caption.replace(/^Microsoft /, "")} ${run.os.display_version} build ${run.os.build}.${run.os.update_build_revision} x86_64`;
assert.match(abiRaw, /test result: ok\. 2 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out/);

const abi = extract(run.command, osLabel, abiLog);
const player = extract("mise run test synvelis-player", "Windows x86_64", "test-player.log");
const production = abi.map(JSON.parse);
const rows = [...player.map(JSON.parse), ...production];
assert.deepEqual(rows.map(({ row_family }) => row_family).sort(), [
  "negative_and_constraint_evidence",
  "player_control_undo_redo_matrix",
  "production_real_abi_undo_redo_matrix",
  "production_real_abi_undo_redo_matrix",
]);
assert.deepEqual(production.map(({ format }) => format).sort(), ["clap", "vst3"]);
for (const row of rows) {
  assert.equal(row.tested_sha, production.includes(row) ? abiSha : playerSha);
  assert.equal(row.verdict, "pass");
}
const control = rows.find(({ row_family }) => row_family === "player_control_undo_redo_matrix");
assert.deepEqual(control.sequence.map(({ plan_tempo_revision }) => plan_tempo_revision), [10, 9, 10, 9, 10]);
assert(control.sequence.every((step) => step.plan_ppqn === 480 && step.last_plan_musical_is_current
  && Object.keys(step.refresh_errors).length === 0));
assert(control.staged_midi_preserved && control.events_preserved.some((event) => event.transport_generation === 1));
for (const row of production) {
  const { fixture, staged_fixture: staged, runner } = row.artifacts;
  const captured = run.artifacts[row.format];
  const child = run.observed_child_modules[row.format];
  assert.equal(child.is_executable_sibling, true);
  assert.equal(captured.fixture, fixture.path);
  assert.equal(child.child_exe, runner.path);
  assert.equal(captured.runner, runner.path);
  assert.equal(child.child_exe_sha256, runner.sha256);
  assert.equal(child.loaded_module, staged.path);
  assert.equal(captured.sibling, staged.path);
  assert.equal(staged.path, win32.join(win32.dirname(runner.path), `ruviform_plugin_test_${row.format}.dll`));
  assert.equal(child.loaded_sha256, fixture.sha256);
  for (const key of ["fixture_sha256_before", "fixture_sha256_after", "sibling_sha256_before", "sibling_sha256_after"]) {
    assert.equal(captured[key], fixture.sha256);
  }
  assert(child.child_pid > 0 && child.parent_pid > 0 && child.child_pid !== child.parent_pid);
  assert.match(child.parent_command_line, /tim03_l1_production_musical_transport-[a-f0-9]+\.exe.*production_tempo_undo_redo/);
  assert(Date.parse(run.start_utc) <= Date.parse(child.observed_utc)
    && Date.parse(child.observed_utc) <= Date.parse(run.end_utc));
  assert(abiRaw.includes(`test ${row.format}_production_tempo_undo_redo ... ${prefix}`));
  assert.notEqual(fixture.path, staged.path);
  assert.equal(fixture.sha256, staged.sha256);
  assert.equal(fixture.size, staged.size);
  for (const artifact of [fixture, staged, runner]) {
    assert.match(artifact.sha256, /^[0-9a-f]{64}$/);
    assert(artifact.path && artifact.size > 0);
  }
  const steps = row.operation_sequence;
  assert.deepEqual(steps.map(({ operation }) => operation), ["add", "undo", "redo", "undo", "divergent-add"]);
  for (const step of steps) {
    assert.equal(step.operation_transport.retroactive, false);
    assert(step.rt_install_receipts.some((receipt) => receipt.status === "Applied"
      && receipt.plan_generation === step.retained_live_plan_generation));
  }
  const measured = steps.filter(({ pcm }) => pcm);
  assert.deepEqual(measured.map(({ operation_transport }) => operation_transport.classification),
    ["before_measured_transport", "while_playing", "while_playing"]);
  assert.deepEqual(measured.map(({ pcm }) => pcm.observed_click_frames), [[640], [672], [622]]);
  row.run_provenance = {
    log: abiLog,
    command: run.command,
    exit_code: run.exit_code,
    start_utc: run.start_utc,
    end_utc: run.end_utc,
    os: run.os,
    plugin_dir_override_present_at_launch: run.plugin_dir_override_present_at_launch,
    child_module: child,
  };
}

const target = new URL("../dynamic-tempo-undo-redo.jsonl", import.meta.url);
const output = `${[...player, ...production.map((row) => JSON.stringify(row))].join("\n")}\n`;
if (mode === "--write") {
  writeFileSync(target, output);
} else {
  assert.equal(readFileSync(target, "utf8"), output, "JSONL must match the canonical stdout and run metadata");
}
console.log(`Verified ${production.length} ABI rows at ${abiSha} and ${player.length} player rows at ${playerSha}`);
