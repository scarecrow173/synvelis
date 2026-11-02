---
id: "IMPL-MDATakeGroupCompEffectiveProjectionPlacement"
type: "impl"
status: "completed"
title: "MD-A TakeGroup/Comp・Effective Comp 投影・ClipPlacement/Playback 時間契約 (headless model PoC)"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/comp.rs"
      - type: "file"
        path: "crates/synvelis-project/src/placement.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
      - type: "file"
        path: "docs/adr/mda-normalized-placement-comp-schema.md"
      - type: "file"
        path: "docs/designs/mda-comp-effective-projection-placement-design.md"
      - type: "file"
        path: "docs/plans/mda-comp-model-poc.md"
      - type: "file"
        path: "docs/test-specs/mda-model-poc-acceptance.md"
      - type: "file"
        path: "docs/tasks/mda-task1-normalized-placement-schema.md"
      - type: "file"
        path: "docs/tasks/mda-task2-comp-schema-ops.md"
      - type: "file"
        path: "docs/tasks/mda-task3-effective-comp-projection-editor-targets.md"
      - type: "file"
        path: "docs/tasks/mda-task4-acceptance-tests-verification.md"
      - type: "file"
        path: "docs/impl/ir/mda-takegroup-comp-effective-projection-placement.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/tempo.rs"
      - type: "file"
        path: "crates/synvelis-project/src/time.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tempo.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/gate.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/plan.rs"
      - type: "file"
        path: "crates/synvelis-recording/tests/vs_d.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lowering.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/l1_e2e.rs"
      - type: "file"
        path: "crates/synvelis-render/src/export.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/l2_e2e.rs"
      - type: "file"
        path: "crates/synvelis-render/tests/x_e2e.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/plans/mda-comp-model-poc.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/adr/mda-normalized-placement-comp-schema.md"
    - "docs/test-specs/mda-model-poc-acceptance.md"
    - "docs/tasks/mda-task1-normalized-placement-schema.md"
    - "docs/tasks/mda-task2-comp-schema-ops.md"
    - "docs/tasks/mda-task3-effective-comp-projection-editor-targets.md"
    - "docs/tasks/mda-task4-acceptance-tests-verification.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# MD-A — TakeGroup/Comp・Effective Comp 投影・ClipPlacement/Playback 時間契約

## Summary

Issue #42（Issue #10 §7・#12 §13/§15 の設計合意）を headless model
PoC として実装した。`CompRegistry`（`TakeGroup`/`Comp`/`CompRegion`/
`CompTransition`）・`CompOp` 命令群・registry-wide 検証 oracle・
`project_comp` による非破壊 Effective Comp 投影・editor
display/editing target の型区別・および `ClipPlacement`/
`ClipPlayback` の混合時間ドメイン契約を `synvelis-project` に追加
し、`Clip` の旧 `anchor`/`source_offset` フィールドを正規化スキーマ
へ移行した。review-protocol で確認された P1/P2 を修正し、
MD-A 受入 23 テストを含む全ワークスペースが green。

## Context

- 録音済み Source/PCM は authoritative・非破壊。comp 状態は
  project/editor モデルに保持し Source を書き換えない。
- musical timeline と absolute/media timeline を型で区別し、
  source-native frame を保存する。tempo map 編集は保存済み
  anchor から placement を再導出し、録音 source frame を書き換え
  ない。
- comp region・transition overlap は明示的・決定的。全 project
  変更は single writer `ProjectController` の command/operation
  経路を通り、revision・undo/redo・persistence 意味を保つ。

## Implementation

### Changes

- `synvelis-project/src/placement.rs`（新規）: `TimeAnchor`
  （`Musical`/`Absolute`）、`ClipExtent`（`Musical`/`Media`）、
  `ClipPlacement`（保存 anchor + extent）、`ClipPlayback`
  （`source_region`+`mapping`）、`PlaybackMapping`
  （`Natural`/`Loop`/`SimpleStretch`）、`StretchRatio`、
  `ResolvedClip`。`resolve`/`resolve_span`/`resolve_musical_range`
  は tempo map 経由で anchor を再導出し、半開区間 `[start,end)` を
  維持、playback window を resolved media length に対して検証する
  （checked arithmetic のみ）。
- `synvelis-project/src/comp.rs`（新規）:
  - `TakeGroup`/`Comp`/`CompRegion`/`CompTransition` POD +
    `CompRegistry` + 新 ID（`TakeGroupId`/`CompId`/`CompRegionId`/
    `CompTransitionId`）。
  - `CompCandidateRef`（`Take`/`ExistingSource`/`Silence`）。
  - `CompOp` 命令群（register group/create comp/add・remove
    region/set range・candidate・mapping/add・remove・set
    transition/set fades/set active）。各 `validate` が対象・
    candidate 適法性を検査。
  - `validate_registry`（load 時 oracle と同一）: lane 所有・take
    membership・group/comp 所有・非空 range・region containment・
    順序/非重複・transition 隣接/共有境界/overlap containment・
    colliding overlap・handle 充足・key↔id coherence・listing
    completeness。
  - `project_comp`（純粋 read・決定的・非破壊）:
    `EffectiveComp`（`EffectiveSegment`+`SourceBinding`+
    `CompProvenance`・`EffectiveTransition`+fade shapes）。
  - `EditorDisplayTarget`/`EditorEditingTarget`/`SourceVersionRef`
    — raw capture（version 0）は型で address 不可。
- `synvelis-project/src/project.rs`: 旧 `TimeAnchor` を
  `ProjectRange` へ改名、`Clip` を `placement`/`playback` フィールド
  へ移行、`comp: CompRegistry` 追加、`validate_references` が comp
  oracle を含む。
- `synvelis-project/src/ops.rs`: `AddClip`/`MoveClip`/`SetClipExtent`
  を新契約へ。`validate_clip_contract` は構造/window 契約のみ —
  tempo 依存 resolvability は consumption に委譲（anchor が権威）。
- `synvelis-project/src/controller.rs`: `apply` が post-apply に
  `validate_references` を実行（後述 P1 修正）。
- `synvelis-project/src/store.rs`: `SCHEMA_VERSION` 1→2。
- `synvelis-project/src/time.rs`: `MediaTimeValue`/`MediaDuration`
  を private field + `new`/`value`/`numerator`/`denominator` +
  serde `try_from`/`into` repr 検証へ（後述 P1 修正）。
- consumer 適合: `synvelis-recording`（commit/gate/plan）、
  `synvelis-player`（lowering）、`synvelis-render`（export）、
  各 E2E テスト。

### Decisions During Implementation

- **検証は単一 oracle + writer 境界**: 各 `CompOp::validate` は
  op-local 契約を、`ProjectController::apply` は post-apply に
  `Project::validate_references`（load oracle と同一）を実行する。
  per-op apply の末尾で registry を再検査する旧実装は廃止 —
  invariant の所有は writer 側で、全 op 種に均一に効く。
- **`Loop` の repeat unit は containing handle interval**:
  `check_window` は `play_from ∈ union interval` を Loop にも要求し
  （overhang 豁免のみ）、`base_window`/`extended_binding` は Loop
  の window をその interval 自身として emit する。project extent や
  unbounded read を出さない。
- **tempo 依存 resolvability は apply 時に要求しない**: 保存済み
  anchor が権威であり、musical 範囲の media 解決は consumption
  （resolve/lowering）で行う。apply-time の構造検証に tempo map
  を要求すると、tempo map 未整備の正しい op まで拒否する。
- **`ClipPlacement.anchor` は `Musical`/`Absolute` の 2 分岐**:
  `Absolute` は media time で直接解決し tempo edit 不変。
  `Musical` は `TickTimeMapping` 経由で保存 anchor から再導出。

## Review Protocol（bounded, 2 reviewers + validator/judge）

review-protocol を実施し、2 reviewer を並行 dispatch。全
candidate を個別にコードで検証し、確認済み finding のみ修正:

| severity | finding | 対応 |
| --- | --- | --- |
| P1 | `AddTempoEvent` が comp handle を破壊し unloadable snapshot を commit し得た（post-apply 検査欠如） | `ProjectController::apply` で `next.validate_references()` を実行するよう修正（root cause は op 個別ではなく enforcement 境界の不在） |
| P1 | `MediaTimeValue`/`MediaDuration`/`StretchRatio` の pub field + serde 経由で den=0 を注入し得た（div-by-zero panic） | private field + constructor/`try_from` repr guard |
| P2 | `Loop` が `play_from ∈ handle` 検査を skip し unbounded window を emit | repeat-unit 契約へ修正（上記） |
| P2 | `region_ids` sort が media raw `value` 比較で time base を無視 | `point_cmp`/`region_start_cmp`/`resort_regions` で `lt` ベースの絶対時間 sort |
| P2 | `validate_registry` が key↔id coherence・listing completeness を見落とす | `key_id` + region/transition listing 検査追加 |
| P2 | window/extended-window の unchecked `i64`/`u64` 加算 | `checked_add`/`checked_sub`/`try_from` へ |
| P2 | `SourceVersionRef` pub field が `new()` guard を迂回 | private field + getter |

上記すべてに regression test を追加（md_a.rs `regression_*` 7 本）。
P3（empty-streams commit lock・load 時 tempo map 未 compile・
load 時 clip contract 未再検査・`Raw` take adoption・
`SetCompRegionMapping`/`SynvelisOp::Comp` 未配線・stretch floor
off-by-one・`AddClip` overlap policy・`ProjectController::new`
trust）は follow-up として記録し、本件では未対応（protocol 上
P0/P1 以外は fix loop 対象外）。

## Acceptance Matrix Traceability

| AC | evidence | test |
| --- | --- | --- |
| MD-A-01 | comp 構築 → `project_comp` が決定的・非破壊（project 不変・再投影で完全一致）、provenance が take/existing/silence を区別 | `md_a_01_comp_projection_reproducible_and_non_destructive` |
| MD-A-02 | 全 validation rule: overlap/transition 隣接・共有境界・overlap containment・colliding zone・handle 不足・所有/ownership・zero-rate/空 range を reject | `md_a_02_*`（6 本） |
| MD-A-03 | musical anchor は tempo edit 後に保存 anchor から再導出・absolute は不変・mixed anchor/extent・stretch 比率・move/extent op が契約保持・Source Native Frame 不変 | `md_a_03_*`（5 本） |
| MD-A-04 | 半開区間 `[start,end)`: 境界 position は右 region 所属 | `md_a_04_boundary_positions_belong_to_right_region` |
| MD-A-05 | undo/redo が ID・状態を復元、save/reload で schema v2・registry・projection が安定 | `md_a_05_*`（2 本） |
| MD-A-06 | `EditorDisplayTarget`/`EditorEditingTarget`/`SourceVersionRef` の型区別・version 0 は ctor+serde 双方で reject | `md_a_06_editor_targets_are_type_level_distinct` |
| review regression | P1/P2 修正の再現条件（tempo brick・den=0・Loop window・media sort・registry corruption・overflow） | `regression_*`（7 本） |

## Validation

全て exit code 0 で実測:

- `mise run fmt` / `mise run fmt-check`: clean。
- `mise run check`（`cargo check --workspace --all-targets`）: clean。
- `mise run lint`（`cargo clippy --workspace --all-targets -- -D warnings`）: clean。
- `mise run test`（`cargo test --workspace --locked`）: **24 suite 全
  green / 0 fail**。`md_a` 23/23（受入 16 + regression 7）。
- Toolchain: `rustc 1.99.0`（workspace `rust-version = "1.89"`）。
- Ruviform 側は未変更（git 依存・lock 記録 revision のまま）。

## Related Experiments

なし（experiment log 対象の試行は発生しなかった — 設計経路は
plan/design で事前に固定され、採否の分岐はレビュー finding の
採否として review-protocol 節に記録）。

## Risks

- `SCHEMA_VERSION` 2 への bump: 旧 v1 snapshot の load/migration は
  PoC スコープ外（既存 project file との後方互換は未保証 —
  PoC 合意どおり）。
- `AddTempoEvent` 等で comp が invalid になる変更は commit 時点で
  reject される（bricking 不可）が、「tempo 編集に伴い comp を
  追随再配置する」高レベル UX は存在しない — editor 側が明示的に
  region を直す前提。
- Effective Comp 投影は model 層のみ — player/render への
  lowering は未接続（follow-up）。

## Known Limitations / Report

- `PlaybackMapping::SimpleStretch` は定比のみ。marker ベースの
  time-warp は型に表現せず（Issue #42 スコープ）。
- Comp MIDI Phrase 詳細・実 GUI・Warp DSP はスコープ外（明示）。
- `SynvelisOp::Comp` 統合 variant・`SetCompRegionMapping` op は
  `CompOp` 群として提供済みだが、上位 composite op への配線は
  follow-up（P3 として記録）。

## Follow-ups

- Effective Comp → player/render lowering（`ResolvedClip` 経路と
  `SourceBinding`/`EffectiveTransition` の接続）。
- Load 時の tempo map compile 検証・clip contract 再検査（P3
  finding: 現状 load oracle は comp/entity 整合のみ）。
- `Raw` take の comp candidate adoption・empty-streams commit
  gate・`AddClip` overlap policy の明文化。
- v1→v2 snapshot migration（必要になった時点で ADR 化）。

## Rollback

- 追加物（`comp.rs`/`placement.rs`/`md_a.rs`/docs chain）は削除で
  完全 revert 可能。`Clip` スキーマ変更は `SCHEMA_VERSION` bump と
  一体 — revert 時は version も戻すこと。consumer 適合は旧
  `anchor`/`source_offset` への単純逆置換で戻せる。
