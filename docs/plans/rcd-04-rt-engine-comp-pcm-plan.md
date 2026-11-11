---
id: "PLAN-034VC2HhruQrlFFlRTEMNY"
type: "plan"
status: "approved"
title: "RCD-04 RT engine comp PCM plan"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-04-rt-engine-comp-pcm.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 RT engine comp PCM plan Implementation Plan

## Goal

`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs` に、実 RT
engine（`HeadlessDriver::run` + `RoutingEngine::run`）と offline
render 実装（`synvelis_render::export`）の comp 再生 PCM を
bit-exact 照合する integration test を追加し、crate fixture の
test 内複写 renderer では検出できない drift gap を塞ぐ。

## Scope

単一 stream・単一 task。変更は新規 test 1 file と `docs/` チェーン
6 file のみ。manifest 変更なし（coordinator 決定 (ii)）。
`synvelis-player/src` の最小補修は検証が実欠陥を露呈した場合に限
り許容（design Risks 参照）。

## Upstream Documents

- `docs/specs/rcd-04-rt-engine-comp-pcm.md`
- `docs/designs/rcd-04-rt-engine-comp-pcm-design.md`

## Design Inputs

- `docs/designs/rcd-04-rt-engine-comp-pcm-design.md` — oracle 方針
  (a)・配置決定 (ii)・fixture 幾何・S2 fail-closed 記録。
- 参照実装:
  - `crates/synvelis-player/tests/md_b.rs` — comp 構築（recording
    ops → take → TakeGroup → Comp → Region → Transition）、
    `HeadlessDriver` 駆動、fade 幾何の断言パターン。
  - `crates/synvelis-render/tests/x_e2e.rs` — `export` +
    `expect_pcm` hook、playback-vs-export bit-exact 前例、
    `scratch`/`controller_with`/`apply`/`prefetch`/`engine`
    helpers、`assert_planes`。
  - `crates/synvelis-player/tests/sf03_l6.rs` —
    `RoutingCompileRequest`/`RoutingDeps`/`prepare`→`publish`→
    `run`、`add_track_strip`。
  - `crates/synvelis-recording/tests/rcd_04_take_comp_pcm.rs` —
    comp 条件の fixture 幾何（参照のみ、renderer は複写しない）。

## File Map

- `crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs` — 新規。
  harness（scratch/controller/apply/rop/cop/engine/prefetch/
  canonical）+ fixture builders（import、take 登録×3、comp
  builder S1/S2、lane clip）+ drivers（`run_driver`/
  `run_engine`/`run_export`）+ plan-derived anchor
  （`expected_at` で Σ window 写像 × `gain_at`）+ 2 `#[test]`。
- `docs/specs/rcd-04-rt-engine-comp-pcm.md` — spec。
- `docs/designs/rcd-04-rt-engine-comp-pcm-design.md` — design
  （(a)/(ii) 決定記録）。
- `docs/plans/rcd-04-rt-engine-comp-pcm-plan.md` — 本 plan。
- `docs/test-specs/rcd-04-rt-engine-comp-pcm.md` — test-spec。
- `docs/tasks/rcd-04-task-rt-engine-comp-pcm.md` — task。
- `docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md` — impl record。
- `docs/impl/ir/README.md` — hand-curated index に新規行を追記
  （生成 marker がなく script が更新しないため手動）。

## Implementation Sequence

- [x] 1. test harness + fixture builder: `write_fixture` stereo
  WAV → `import_local` → `AddTrack`/`AddTrackStrip`/`AddLane`/
  `RegisterClockEpoch` → session/stream/pass/segment/take ×3 →
  `MarkTakeVerified` → `RegisterTakeGroup`/`CreateComp`/
  `AddCompRegion`×6/`SetActiveComp`/`AddCompTransition`×3 →
  `AddTempoEvent`(2880→90BPM) → lane clip（superseded 用）。
- [x] 2. drivers: `run_driver`（`lower`→`prepare`→activate→
  `run`→trim）、`run_engine`（`plan_playbacks`→`RoutingDeps`→
  `prepare`→`expect_built`→`publish`→`run`→trim）、
  `run_export`（`RenderSnapshot::freeze`→`export`→`expect_pcm`
  capture→`Completed`）。
- [x] 3. S1 `rcd_04_rt_engine_comp_pcm_matches_offline_export`:
  plan inventory 断言 → 3 経路 PCM bit-exact + crc 一致 → 反復
  実行 crc 再現 → plan-derived anchor（identity/overlap probe/
  silence）→ revision/source bytes 不変。
- [x] 4. S2 `rcd_04_rt_engine_unsupported_mapping_fails_closed`:
  Loop+SimpleStretch comp → legacy skip 記録+沈黙再生、
  `RoutingEngine::prepare` → `UnsupportedFormatOrTimeMap`、
  `export` → `Failed{Preflight,UnrenderableComp}`。
- [x] 5. docs チェーン残り（test-spec・task・impl record・
  impl/ir README 行）を実内容で populate。
- [ ] 6. `mise run fmt` → targeted test → canonical 全 lane。

## Dependencies and Risks

- `RoutingDeps.clips` は `lower_semantic` の `lowered.playbacks`
  順と一致必須 → `plan_playbacks` 出力順 == lowered 順（
  `emit_playbacks` が plan 順に emit）なので plan 順に
  `resolver::prepare`。
- ClipPlayer↔ClipRenderer の実装差で不一致が出る → 不一致
  frame/sample を証拠に源を特定し `synvelis-player/src` 最小補修
  （許容範囲内）。超える場合は ask。
- `SimpleStretch(2:1)` region の source 読取長が handle を超え得
  る → S2 の region geometry は skip されるため実 read なし;
  candidate window の検証は `AddCompRegion` validate に依存する
  ので source coverage 内に収める。
- tempo event 位置（2880）と T3 overlap（[2820,2940)）が重なる
  → overlap 解決は `TickTimeMapping` 経由で両経路同一。断言は
  plan の `window` 値から読むので mapping 依存で安全。

## Task Breakdown

- `docs/tasks/rcd-04-task-rt-engine-comp-pcm.md`（本 slice 全体
  を 1 task として扱う）。

## Verification

- [x] `cargo test -p synvelis-render --test rcd_04_rt_comp_pcm`
  — 2 tests pass（補助証拠; canonical は mise）。
- [ ] `mise run fmt-check` — canonical。
- [ ] `mise run check` — canonical。
- [ ] `mise run lint` — canonical。
- [ ] `mise run test` — canonical（workspace 全体回帰）。
- [ ] `mise run test-rt` — canonical。
- [ ] `mise run test-msrv` — canonical。

## Open Questions / Gaps

- 実 device/OS GUI/AudioLens f64 経路 — 本 slice では NotRun /
  Unsupported。#114・Ruviform #294 で後続検証。
- fixture test の複写 renderer — 本 slice では維持。統合判断は
  別 slice。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 test file・依存直列）。
- Independent work streams: none。
- User approval required before dispatch: yes — review-protocol
  review を承認として扱う。
