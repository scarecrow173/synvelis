---
id: "TSPEC-cb02AudioClipTakeCompT"
type: "test-spec"
status: "approved"
title: "CB-02 Audio Clip/Take/Comp 候補 Adapter fixture tests"
created: "2026-11-14"
updated: "2026-11-14"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/279"
  verifies:
    - "docs/specs/creative-branch.md"
  derives-from:
    - "docs/designs/creative-branch-design.md"
  supersedes: []
  superseded_by: []
---

# CB-02 Audio Clip/Take/Comp 候補 Adapter tests

## Purpose

synvelis#279 CB-02 スライスの headless fixture が保証すべき
振る舞いを固定する。`VariantScope::AudioClip` /
`VariantScope::TakeUsage` の実ドメイン `DomainAdapter` が、
Raw media（`Source`/`Asset`/`RecordingSession`/`Take`/capture
系）を不変のまま、clip-local 編集と comp registry の候補
編集・比較・adopt materialize を行う契約を検証する。
実 OS/実 Plugin/実 RT/実 Render・実録音 pipeline に依存する
判定は本 spec の対象外（`evidence/sv279/` で
NotRun/Deferred 記録）。spec/design が CB-03 以降で再設計
された場合でも、ここに記す不変条件（Raw 不変・scope 閉じ込め・
fail-closed・capability 誠実性）は維持されるべき保証であり、
本 test spec はその判定基準として残す。

## Feature

`AudioClipAdapter` と `TakeUsageAdapter` による
`VariantScope::AudioClip` / `VariantScope::TakeUsage` の
branch candidate 編集と、`adopt_branch_adapted` 経由の
comping entity materialize。candidate は live-space 予約 ID・
CAS revision・scope digest を CB-01 契約どおり継承する。

## Rules

- **Rule CB-CLIP-RAW**: 同一 Raw Asset を共有する 2 Clip の
  加工候補を Fork/Save/Reload でき、元 Blob SHA と Source
  Metadata は bitwise 不変。候補編集は clip-local entity のみ
  を変更し、source/asset/lane 他 clip を汚染しない。
  Examples: `cb_clip_01_shared_raw_asset_candidates_survive_fork_save_reload`.
- **Rule CB-CLIP-CAPABILITY**: Trim/Fade/Stretch/Pitch 等の
  表示 Capability と実処理 Capability が一致し、未サポート
  mapping（`Loop`/`SimpleStretch`）や pitch/fade/external
  recipe を成功扱いしない — edit guard と能力報告が同一
  判定を共有する。
  Examples: `cb_clip_02_capability_report_matches_edit_enforcement`.
- **Rule CB-CLIP-SCOPE**: clip scope 外の entity（他 clip・
  lane・source・processor 等）を対象とする `ProjectOp` は
  typed reject であり、live・candidate のいずれも変更されない。
  Examples: `cb_clip_02_capability_report_matches_edit_enforcement`
  （scope 外 op 拒否を含む）。
- **Rule CB-TAKE-DISTINCT**: 同一 Take の複数加工
  （processing edit）と異なる Take 選択（selection edit）は
  区別され、別々の scope content digest を持つ。Comp
  Region/Handle/Fade/Transition の無断上書きはない — 未編集
  region の candidate・project_range・transition は bit 同一。
  Examples: `cb_take_01_take_selection_and_processing_are_distinct_scoped_edits`.
- **Rule CB-TAKE-ADOPT**: Adopt は宣言された take usage scope
  の comp closure のみを 1 CAS + 1 Undo で materialize し、
  Raw Recording/Recovery データと別 lane の take group は
  bitwise 不変。undo/redo で live comp state・採用 pointer・
  render identity が一体で復元される。
  Examples: `cb_take_02_adopt_writes_only_scoped_comp_entities_single_cas_undo`.
- **Rule CB-TAKE-SAFE-REJECT**: 録音中 session が対象 take
  group に関与する adopt は `VariantError::RecordingActive`
  で型付き拒否。missing source・存在しない take 参照・
  candidate handle 外の source 時刻は typed reject であり、
  live に部分変更を残さない。scope 外・未サポートの comp op
  も同様に reject。
  Examples: `cb_take_03_recording_active_adopt_rejects_typed`,
  `cb_take_03_missing_source_and_unknown_source_time_reject`,
  `cb_take_03_out_of_scope_and_unsupported_comp_ops_reject`.
- **Rule CB-TAKE-CANDIDATE-LIFECYCLE**: 非採用候補は明示
  Discard まで再比較可能で、candidate payload は immutable
  参照 + digest を維持する。Undo/Redo/Save/Reload で同一
  Source lineage（branch-local ID・minted entity の provenance）
  が復元される。
  Examples: `cb_take_04_candidate_reusable_until_discard_and_lineage_survives_reload`.
- **Rule CB-AUDIO-RCP**: scope content digest は branch 横断・
  adopt 前後で render-reuse identity として一致し、scope 内
  編集は部分失効として digest を動かす。shared context
  （scope 外 entity）は digest に影響せず、adopt 後の render
  pin は新 live scope 内容を指す。composed render view は
  candidate comp state を live shared raw material 上に解決
  し、take/source/asset entity を複製しない。
  Examples: `cb_audio_rcp_01_clip_branch_render_reuse_and_partial_invalidation`,
  `cb_audio_rcp_01_take_branch_render_reuse_and_partial_invalidation`.

## Examples

上記 Rules の Examples に列挙した test 関数が具体 scenario。
fixture は `crates/synvelis-project/tests/cb02_audio_take.rs`
（`clip_fixture`/`take_fixture` + `Fx` helper）。

## Guarantees

- Raw media（`Source`/`Asset`/`RecordingSession`/`Take`/
  `RecordingPass`/`CaptureStream`/`CaptureSegment`）は候補
  payload にも adopt 対象にもならず、branch 編集・adopt・
  undo/redo を通じて bitwise 不変。
- comp entity の materialize は domain-validated candidate
  経路（variant adopt）に限定され、VCS `apply_selected` の
  fail-closed 境界を侵さない。
- 未サポート操作・scope 外操作・録音中採用・missing/unknown
  source 参照は常に型付き拒否であり、成功の偽装・推測
  正規化・部分適用を行わない。
- scope content digest は render pin/RCP reuse identity として
  安定し、scope 内編集のみが部分失効を起こす。

## Non-goals

- 実ドメイン Adapter の残り（CB-03〜08 = Issue #280〜285）、
  Compare Orchestrator（#285）、GPUI（#286）、Action/Gateway
  統合（#287）、Cross-Gate/実機受入（#288）。
- 実 OS/実 Plugin/実 RT/実 Render pipeline・実録音/Import
  pipeline・workspace/GPUI projection での判定 —
  `evidence/sv279/` で NotRun/Deferred。
- `Loop`/`SimpleStretch` mapping の実処理（lowering が
  `Natural` のみ受理する現行契約では Unsupported が正しい
  verdict）。
- Semantic Merge/Blend・全 Branch 常時 RT 駆動・実測なしの
  性能保証（Issue の対象外範囲）。
- comp 変更の VCS `apply_selected` 経路（`CompOp` replay 設計は
  別 Issue）。

## Risk

- これらの保証を落とすと、Raw capture data の破壊・scope 外
  汚染・未サポート処理の成功偽装・録音中の暗黙切替・stale
  参照の推測適用が起こり得る。特に Raw 不変と fail-closed
  拒否は CB-03 以降の全 adopt 経路と Cross-Gate 受入が依存
  する土台であり、退行は critical path 全体を危うくする。
