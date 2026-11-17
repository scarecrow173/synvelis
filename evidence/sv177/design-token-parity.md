# SV177 design-token parity evidence (SV-GPUI-03 slice)

Scope: synvelis#177 slice `SV177 product theme canonical semantic
vocabulary + headless↔palette parity + design fixtures`. Records the
semantic evidence this slice can honestly produce; pixel/native
evidence is explicitly out of scope (see NotRun).

- Branch: `feat/177-design-tokens`
- Upstream pin: ruviform `cbcc834` (`ruviform-ui-theme` resolves DTCG;
  `ruviform-ui-gpui::ViewPalette` is private — `render.rs`)
- Verified on: Windows, mise-managed toolchain (workspace rust;
  CI pin 1.99)

## 1. Canonical vocabulary → palette read keys

`ViewPalette::from_theme` reads 15 color keys + 2 length keys. All are
now declared in shipped DTCG as whole-value aliases to product
primitives (`docs/designs/issue-177-design-tokens.md` has the full
mapping table). Provisional direct values (pending native design
review, RF#631/SV#114): `state.disabled`, `viz.waveform_rms`, light
`color.bg-active`/`color.bg-input`, status ramps.

| palette key | alias target (mode docs) | dark value | light value |
| --- | --- | --- | --- |
| surface.bg | color.bg | #1b1d21 | #f5f6f8 |
| surface.raised | color.bg-panel | #23262c | #e8eaee |
| surface.border | color.border | #33373f | {color.bg-panel} |
| text.primary | color.fg | #e8e8ea | #1b1d21 |
| text.dim | color.fg-dim | #9aa0a8 | #5a6068 |
| accent.primary | color.accent | #6aa9e8 | #2a6bc2 |
| state.error | color.err | #e86a6a | #c23a3a |
| state.disabled | (direct, provisional) | #4e565f | #9aa0a8 |
| control.track | color.bg-input | #191b1f | #eaecf0 (provisional) |
| control.fill | color.accent | #6aa9e8 | #2a6bc2 |
| control.thumb | color.fg | #e8e8ea | #1b1d21 |
| viz.waveform | color.accent | #6aa9e8 | #2a6bc2 |
| viz.waveform_rms | (direct, provisional) | #8fb8e8 | #7aa8d8 |
| viz.baseline | color.border | #33373f | {color.bg-panel} |
| viz.cursor | color.fg | #e8e8ea | #1b1d21 |
| space.sm/md/page | spacing.{sm,md,lg} | per density | per density |

## 2. 2 theme × 2 density × 2 scale semantics

- **theme (mode)**: dark vs light diverge on every aliased color
  (asserted: `surface.bg` differs). `state.disabled`/
  `viz.waveform_rms` are mode-invariant provisional literals.
- **density**: `space.sm`/`space.md`/`space.page` resolve to the
  density document's px values (`LengthUnit::Px` preserved);
  compact/comfortable differ, asserted per combination.
- **scale (2 scale contexts, factor 1.0 / 2.0)**: `ScaleContext` is a
  mapper-side conversion hint only — `ResolvedTheme::with_scale`
  leaves every token value unchanged (asserted for all palette keys).
  Shipped dimensions are px units, so scale produces zero token-level
  diff; device-px conversion happens in the mapper, out of this lane.

## 3. Screen/component semantic fixtures

Fixture = stable semantic `ComponentKey`s each product screen mounts
(fixed in `crates/synvelis-app/tests/theme_parity.rs` and asserted
against `observe()` output):

| screen | representative keys | canonical token family |
| --- | --- | --- |
| chrome | screen.root, screen.toolbar, screen.tabs | surface.*/text.* |
| project toolbar | project.save/reload/undo/redo | control.* |
| tab strip | tab.arrangement…tab.project | surface.raised/border |
| arrangement | arrangement.table, viz.waveform{,.panel,.status} | viz.* |
| transport | transport.status/buttons/play/stop, seek.position | text.*/control.* |
| mixer | tab.mixer.strip, fader.gain, viz.meter{,.panel,.status} | control.*/viz.* |
| inspector | inspector.target/table, relations.* | text.*/surface.* |
| signal flow | signal_flow.{status,nodes,edges,probes} | viz.* |
| project tab | ppqn.{status,preview,effects,sites} | text.*/control.* |
| secondary window | screen2.root, transport2.*, seek2.position | same families |
| plugin dock frame | plugin.dock, plugin.editor | dock frame only — plugin GUI internals are SV#176/RF#630 scope |

Conditional keys `toggle.mute`/`toggle.solo` mount only when mixer
state supplies targets; not in the required fixture.

## 4. Executed verification (all Passed)

| command | result |
| --- | --- |
| `mise run test synvelis-theme` | 18 passed / 0 failed (incl. 3 new canonical tests) |
| `mise run test-app-gpui` | all suites pass; theme_parity.rs 5/5 |
| `mise run lint-app-gpui` | clean (`-D warnings`) |
| `mise run lint synvelis-theme` | clean |
| `mise run check-app-native` | clean |
| `mise run fmt-check` | clean |
| `mise run check-headless-deps` | pass |

## 5. NotRun / Unsupported (honest record)

- **Pixel/color golden, real OS window, real input, IME/DPI
  execution**: Unsupported in gpui-test lane — `DrawnPlan` DTOs carry
  geometry/text/semantic plans, not colors; themed vs fallback mounts
  produce identical plans (asserted). Owner gate: RF#631 + SV#114.
- **`ViewPalette` internal value inspection**: Unsupported —
  `ViewPalette` is private in `ruviform-ui-gpui`; parity is proven at
  the `ResolvedTheme`/`ViewMount` public boundary instead.
- **Plugin GUI internal theming**: out of scope — SV#176/RF#630.
- **Provisional value confirmation**: Deferred to native design
  review (RF#631/SV#114); values remain marked provisional.
- **`*-full` / `*-msrv` / 3-OS lanes**: NotRun — daily/manual Full CI
  scope per POLICIES.md §5.
