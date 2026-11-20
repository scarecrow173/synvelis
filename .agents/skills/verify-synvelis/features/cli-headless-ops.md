# Headless CLI ops

`synvelis-cli` is the non-GUI user path: capability discovery,
project inspection, session management, and script static analysis —
all in-process, with a versioned JSON envelope and deterministic exit
codes. This feature covers the offline-verifiable read surface; the
mutating `invoke`/`run` paths keep their own authority rules.

## Sub-features

- `capability-list` enumerates the registered capabilities —
  `action list`.
- `capability-describe` prints one capability's descriptor —
  `action describe <id[@major.minor]>`; an unknown id fails as
  `cli.unsupported` (exit `5`) and names `action list` as the fix.
- `project-info` reads project metadata — `project info --project
  <path>`; omitting the flag exits `2` (`cli.usage`).
- `session-list` shows issued sessions — `session list` (uses the
  `--session-store` default path); an empty store is a valid result.
- `script-analyze` statically analyzes a Luau/Rhai script file —
  `script analyze --file <path>`; it never executes
  (AUTX-SCR-ANA-01) and mints no authority. `.lua` is explicitly
  unsupported (`cli.unsupported`, exit `5`) — Luau (`.luau`) and
  PoC-only Rhai (`.rhai`) are the covered languages.

## How to get to it (user POV)

- Run `synvelis-cli <command>` in a terminal — after `mise run
  build-cli`, the binary is `target/debug/synvelis-cli`.
- `--format human|json|ndjson` selects output (json = one versioned
  envelope); `--help` / `-h` prints usage; `--version` / `-V` prints
  the tool version.
- `--dry-run` requests a plan without side effects — read-only
  commands reject it by name rather than pretending to plan.

## Driving it with synvelis-cli

Preconditions:

- `mise run build-cli` has produced `target/debug/synvelis-cli`
  (headless — no GUI deps, no display needed).
- A scratch `--session-store` path for session commands if you don't
  want to touch the default store; read-only commands need none.

- **Discover capabilities.** Run `synvelis-cli --format json action
  list`. Exit `0`; the envelope's `result.capabilities` lists
  registered capability ids and versions — the catalog every other
  command resolves against.
- **Describe one.** Run `synvelis-cli --format json action describe
  <id>` for a listed id (e.g. `synvelis.asset.register`). Exit `0`;
  the envelope's `result.matches` records the descriptor. An unknown
  id exits `5` (`cli.unsupported`) naming `action list` as the fix —
  an honest refusal, not a crash.
- **Project info.** Run `synvelis-cli --format json project info
  --project <path>` on an existing project dir. The envelope carries
  project metadata; a missing `--project` exits `2` (`cli.usage`).
- **Session list.** Run `synvelis-cli --format json session list`.
  Read-only inventory of the session store — `result.sessions: []`
  on an untouched store is a valid, honest result.
- **Analyze a script.** Run `synvelis-cli --format json script
  analyze --file <script.luau>` on a Luau source. Exit `0`; the
  envelope reports `analyzer.analyzer_id` plus `diagnostics` — it
  never runs the script. A `.lua` file exits `5` by design.
- **Dry-run honesty.** Run `synvelis-cli --dry-run action list`.
  Exit `2` with a named rejection — `--dry-run` is refused on
  read-only commands rather than silently ignored.
- **Proof.** Capture command + stdout envelope + exit code in
  `evidence/sv600/` (or the issue's ledger) — the envelope's
  versioned shape is the proof, not the human rendering.

## Gotchas

- Exit codes are per-outcome, not per-command: `0` completed, `2`
  CLI misuse, `5` an unsupported/unknown capability — the full map
  lives in `src/exitcode.rs` and an `accepted` async op is not `0`.
- `--dry-run` on a read-only command is a named error by design —
  don't read it as a broken flag.
- `action invoke`/`action run`/`workflow run`/`render` mint and
  spend authority — keep to the read surface above unless the recipe
  owns a disposable project/session store.
- `--format json` is the proof format; the `human` rendering is for
  people and may drift.
- `script analyze` is intentionally not `script run` — there is no
  script execution entry in this CLI (AUTX-CLI-FILE-01); execution
  goes through `action run --file`.
