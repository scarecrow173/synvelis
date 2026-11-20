# Script editor and script analysis

The Script Editor surface is a mounted view driven by its own
`svscript.*` command targets — source display, diagnostics,
completions, and the API-verdict table — and the headless
`synvelis-cli script analyze` statically analyzes a script file
without executing it.

## Sub-features

- `editor-view` — `script_editor_screen_spec` renders source text,
  diagnostics, completions, and the API table for a mounted
  `ScriptEditorScreenState`.
- `editor-actions` — `svscript.editor` decodes `analyze` and
  `toggle-api`; `svscript.diagnostics`/`svscript.completions`/
  `svscript.api` decode `select` row gestures into view-local
  actions.
- `analyze-feedback` — `Analyze` feeds the analyzer result back
  through `ScriptEditorDocument`; unknown targets/actions/malformed
  rows decode to `None` honestly.
- `cli-analyze` — `synvelis-cli script analyze --file <path>` runs
  static analysis only (AUTX-SCR-ANA-01): `.luau` and PoC `.rhai`
  covered, `.lua` explicitly `cli.unsupported` (exit 5).

## How to get to it (user POV)

- The Script Editor is a view surface the app mounts —
  `mount_script_editor_view` projects it like the other product
  views.
- `synvelis-cli script analyze --file <script.luau>` is the headless
  entry — no window, no execution.

## Driving it with synvelis-cli and the test lanes

Preconditions:

- `mise run build-cli` produced `target/debug/synvelis-cli`;
  `mise run test synvelis-app` lanes run headless on any host.
- A `.luau` fixture file for the live analyze command.

- **Editor contract.** `mise run test synvelis-app --test
  svscript04_script_editor` proves the decode contract (unknown
  targets/actions/malformed rows → `None`), the view-local select
  actions, and the analyze feedback path.
- **Static analyze.** `synvelis-cli --format json script analyze
  --file <script.luau>` exits 0 with the envelope's
  `analyzer.analyzer_id` + `diagnostics` — it never executes the
  script.
- **Unsupported honesty.** `synvelis-cli --format json script
  analyze --file <script.lua>` exits 5 (`cli.unsupported`) — `.lua`
  is named-unsupported, not silently analyzed.

## Gotchas

- `script analyze` is not `script run` — this CLI has no script
  execution entry (AUTX-CLI-FILE-01); execution goes through
  `action run --file` authority.
- The editor commands ride `svscript.*` view-local targets decoded by
  `decode_script_editor_command` — they are not `syn.*` service ops,
  so there is no `drive --op syn.script.*` recipe; dispatch goes
  through `syn.control.dispatch` (mutating).
- Analyze mints no authority — it is a read surface even though it
  takes a file path.
- The receipt journal is metadata-only — where a control dispatch
  drives the editor, emit proves delivery only; editor semantics stay
  in `svscript04_script_editor`.
