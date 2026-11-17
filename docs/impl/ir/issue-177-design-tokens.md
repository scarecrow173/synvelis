---
id: "IMPL-034YXYrqo8gm6LNI7SUJwM"
type: "impl"
status: "completed"
title: "SV177 design tokens implementation record"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/tasks/issue-177-design-tokens.md"
    - "https://github.com/scarecrow173/synvelis/issues/177"
  changes:
    added:
      - "assets/themes: canonical semantic token groups (surface/text/accent/state/control/viz/space)"
      - "crates/synvelis-app/tests/theme_parity.rs"
      - "evidence/sv177/design-token-parity.md"
    modified:
      - "assets/themes/synvelis-dark.dtcg.json"
      - "assets/themes/synvelis-light.dtcg.json"
      - "assets/themes/synvelis-density-compact.dtcg.json"
      - "assets/themes/synvelis-density-comfortable.dtcg.json"
      - "crates/synvelis-theme/tests/theme_contract.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-177-design-tokens.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-177-design-tokens.md"
    - "docs/test-specs/issue-177-design-tokens.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-177-design-tokens.md"
  derived-by: []
  verifies:
    - "docs/test-specs/issue-177-design-tokens.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV177 design tokens implementation record

## Summary

Shipped DTCG product themes now carry the canonical semantic vocabulary
that `ruviform-ui-gpui::ViewPalette::from_theme` reads (`surface.*`,
`text.*`, `accent.*`, `state.*`, `control.*`, `viz.*`, `space.*`),
expressed as whole-value DTCG aliases to existing `color.*`/`spacing.*`
primitives so `ruviform-ui-theme` remains the sole interpreter. Headless
contract tests prove all 4 mode/density combinations resolve product
values (not palette fallback literals), and a new gpui-test lane proves
`ViewMount.theme = Some(resolved)` mounts and settles through the same
composition path `synvelis-native` uses. Screen/component fixtures and
2theme×2density×2scale semantic evidence are recorded in
`evidence/sv177/design-token-parity.md`.

## Context

Follows `docs/tasks/issue-177-design-tokens.md` (spec
`docs/specs/issue-177-design-tokens.md`, design
`docs/designs/issue-177-design-tokens.md`, ADR
`docs/adr/issue-177-design-tokens.md`, test-spec
`docs/test-specs/issue-177-design-tokens.md`) for synvelis#177 slice
SV-GPUI-03. `ViewPalette` is private in `ruviform-ui-gpui` — parity is
proven at the public `ResolvedTheme` + `ViewMount` boundary only.

## Implementation

### Changes

- `assets/themes/synvelis-{dark,light}.dtcg.json`: added `surface`,
  `text`, `accent`, `state`, `control`, `viz` groups. Whole-value
  aliases to `color.*` primitives; direct provisional values only for
  `state.disabled`, `viz.waveform_rms`, and new light-mode
  `color.bg-active`/`color.bg-input` primitives.
- `assets/themes/synvelis-density-{compact,comfortable}.dtcg.json`:
  added `space.{sm,md,page}` aliasing `spacing.*`.
- `crates/synvelis-theme/tests/theme_contract.rs`: three new contract
  tests — canonical color values across all 4 mode/density pairs,
  non-equality with `ViewPalette::fallback` literals, alias-tracks-
  primitive checks, and `space.*` px resolution per density.
- `crates/synvelis-app/tests/theme_parity.rs`: five gpui-test tests —
  4-combination mount/observe/drawn acceptance, scale-context and
  mode-divergence semantics, main+secondary screen fixture keys,
  plugin-dock frame fixture keys, and an explicit negative test that
  `DrawnPlan` carries no color (theme vs fallback plans identical).

### Decisions During Implementation

- Alias layer inside the product DTCG documents over duplicating
  values — keeps primitives authoritative and avoids a second
  interpreter (per ADR).
- Parity proven via public boundary (`ResolvedTheme` accessors +
  `ViewMount.theme` mount) rather than exposing `ViewPalette` —
  upstream change out of scope.
- `DrawnPlan` equality between themed and fallback mounts is asserted
  as honest documentation of the observability limit, not a defect.

## Related Experiments

### Adopted

None recorded — `docs/impl/exp/issue-177-design-tokens-exp.jsonl`
records verification runs; no rejected alternatives.

### Rejected

None.

## Validation

All on Windows (worktree `sv177-design-tokens`), mise-managed toolchain
(rust 1.99 CI pin / workspace toolchain):

- `mise run test synvelis-theme` — 18 passed, 0 failed (incl. 3 new
  canonical vocabulary tests).
- `mise run test-app-gpui` — all suites pass; `theme_parity.rs` 5/5.
- `mise run lint-app-gpui` — clean after doc-backtick fixes.
- `mise run lint synvelis-theme` — clean.
- `mise run check-app-native` — clean (native lane non-regression).
- `mise run fmt-check` — clean.
- `mise run check-headless-deps` — pass.

## Risks

- Provisional values (`state.disabled`, `viz.waveform_rms`, light
  `bg-active`/`bg-input`, status ramps) await native design review
  (RF#631/SV#114 gate) — recorded as provisional in assets and
  evidence.
- Pixel/color parity is unobservable in gpui-test lane; real-window
  golden comparison remains Deferred to RF#631+SV#114.

## Follow-ups

- Native design review to confirm provisional values.
- Pixel golden / real OS window verification under RF#631+SV#114.
- docs index regeneration deferred to Coordinator at integration.
