import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";

const [mode, sha] = process.argv.slice(2);
assert(["--write", "--check"].includes(mode) && /^[0-9a-f]{40}$/.test(sha ?? ""));

const read = (name) => readFileSync(new URL(name, import.meta.url), "utf8");
const summary = read("canonical-results.tsv").split(/\r?\n/).map((line) => line.split("\t"));
const prefix = "tim03-dynamic-tempo-undo-redo ";

function extract(command, log) {
  assert(summary.some(([cmd, os, exit, kind, path]) =>
    cmd === command && os === "Windows x86_64" && exit === "0"
    && kind === "canonical" && path === log), `No successful canonical receipt for ${log}`);
  const raw = read(log);
  assert.match(raw, /test result: ok\./);
  return raw.split(/\r?\n/).filter((line) => line.includes(prefix))
    .map((line) => line.slice(line.indexOf(prefix) + prefix.length));
}

const abi = extract("mise run test-tim03-dynamic-musical-transport", "dynamic-distinct-fixtures.log");
const player = extract("mise run test synvelis-player", "test-player.log");
const lines = [...player, ...abi];
const rows = lines.map((line) => JSON.parse(line));
const production = rows.filter(({ row_family }) => row_family === "production_real_abi_undo_redo_matrix");
assert.deepEqual(rows.map(({ row_family }) => row_family).sort(), [
  "negative_and_constraint_evidence",
  "player_control_undo_redo_matrix",
  "production_real_abi_undo_redo_matrix",
  "production_real_abi_undo_redo_matrix",
]);
assert.deepEqual(production.map(({ format }) => format).sort(), ["clap", "vst3"]);
for (const row of rows) {
  assert.equal(row.tested_sha, sha);
  assert.equal(row.verdict, "pass");
}
const control = rows.find(({ row_family }) => row_family === "player_control_undo_redo_matrix");
assert.deepEqual(control.sequence.map(({ plan_tempo_revision }) => plan_tempo_revision), [10, 9, 10, 9, 10]);
assert(control.sequence.every((step) => step.plan_ppqn === 480 && step.last_plan_musical_is_current
  && Object.keys(step.refresh_errors).length === 0));
assert(control.staged_midi_preserved && control.events_preserved.some((event) => event.transport_generation === 1));
for (const row of production) {
  const { fixture, staged_fixture: staged, runner } = row.artifacts;
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
}

const target = new URL("../dynamic-tempo-undo-redo.jsonl", import.meta.url);
const output = `${lines.join("\n")}\n`;
if (mode === "--write") {
  writeFileSync(target, output);
} else {
  assert.equal(readFileSync(target, "utf8"), output, "JSONL must match the canonical stdout");
}
console.log(`Verified ${rows.length} ${sha} evidence rows from the two canonical lanes`);
