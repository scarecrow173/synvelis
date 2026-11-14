---
id: "IMPL-034XaaoFK7IUzs8UMi97aN"
type: "impl"
status: "completed"
title: "SF-05 residual headless coverage — SF-UX-01..07 棚卸し + 欠落6件補完 + evidence 台帳"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added:
      - type: "file"
        path: "docs/impl/ir/sf05-editor.md"
      - type: "file"
        path: "docs/impl/exp/sf05-editor.md"
      - type: "file"
        path: "evidence/sv108/coverage.md"
      - type: "file"
        path: "evidence/sv108/verification.md"
      - type: "file"
        path: "evidence/sv108/external-gates.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/tests/sf_05_s2.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "index"
        path: "docs/impl/ir/README.md"
      - type: "index"
        path: "docs/impl/exp/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
    - "docs/plans/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
    - "docs/adr/sf-05-slice2-routing-gesture-inspection-persist-boundary.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
    - "docs/tasks/sf-05-slice1-signal-flow-headless.md"
    - "docs/tasks/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/tasks/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-05-slice1-signal-flow-headless.md"
    - "docs/plans/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  verified-by:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
    - "docs/test-specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sf05-editor.md"
    rejected: []
---

# SF-05 residual headless coverage — 実装記録

## Summary

Issue #108（SF-05 Signal Flow Editor / Semantic+Visual Hierarchy /
Inspector・Automation・Mixer 同期）の残存 headless 被覆棚卸し。
slice1（PR#216 系: hierarchy 投影・visual-only 不変・状態表示）、
slice2（SF-UX-03/04/07: routing gesture・inspection・persist）、
slice3（probe subscriber/capture dedup）は main 統合済みであり、
本タスクは新規 canonical 文書チェーンを作らず、既存 spec/design/
plan/task/test-spec を正本として参照する（Coordinator 指示）。

監査の結果、SF-UX-01/02/05/06 は既存テストで十分に被覆されている
ことを確認し、SF-UX-03/04/07 の headless 検証可能な欠落 6 件に
対応する 7 テストを `crates/synvelis-workspace/tests/sf_05_s2.rs`
に追加した（dock/close retention は move+pool と close で性質が
異なるため 2 テストに分割）。src 変更は不要で、全て既存
public API の契約を固定するテストのみ。

## Context

- 対象: synvelis#108 SF-05 残存 headless 被覆（Issue 受入条件
  SF-UX-01〜07 の棚卸し + 欠落補完 + Evidence 台帳）。
- 正本文書チェーン（新規作成せず参照）:
  - spec: `docs/specs/sf-05-signal-flow-headless-hierarchy.md`、
    `docs/specs/sf-05-slice2-signal-flow-ux0307.md`、
    `docs/specs/sf-05-slice3-probe-subscriber-wiring.md`
  - adr: `docs/adr/sf-05-slice1-...md`、
    `docs/adr/sf-05-slice2-...md`
  - design/plan/task/test-spec: 同名 slice1..3 各文書
  - 実装記録: `docs/impl/ir/sf-05-slice1-...md`、
    `docs/impl/ir/sf-05-slice3-...md`
- 先行統合: sv#102（UI-C5 ops）、sv#107（SF-04 probe adapter）、
  slice5 subscriber lease（synvelis-workspace）。
- 編集範囲: `evidence/sv108/`、`crates/synvelis-app/`、
  `crates/synvelis-workspace/`、本 ir、`docs/impl/exp/sf05-editor.md`。
  `crates/synvelis-project/` は read-only、Ruviform 共通基盤は
  複製・変更禁止。

## Implementation

### Changes

- `crates/synvelis-workspace/tests/sf_05_s2.rs`（修正・テスト 7 件
  追加、既存 18 件は不変）:
  - `routing_gesture_concurrent_window_revision_second_commit_is_stale`:
    2 window・2 view が同一 project revision に fixate した gesture
    を並行実行。先着 confirm が 1 route を commit した後、後着は
    `Stale` で drop — retarget せず、undo 1 回で committed route
    のみが戻る（SF-UX-03 concurrent window revision）。
  - `routing_gesture_capability_generation_drift_drops_stale`:
    gesture 中に resolved processor binding を
    `capability_revision` bump で差し替え
    `port_capability_generation` が動くことを確認した上で、
    confirm が `Stale` を返し project を変更しない
    （SF-UX-03 plugin port reconfigure 相当の generation drift）。
  - `routing_gesture_terminal_state_rejects_late_input`:
    Applied 終端後の late hover/drop/confirm/cancel が全て
    `NoGesture` で fail-closed。project/revision 不変、gesture slot
    は解放され新規 gesture が開始できる（SF-UX-03 late receipt）。
  - `input_context_loss_drops_gesture_without_misdelivering_focus`:
    TextInput/Ime/PluginGui/Modal への `set_input_context` 遷移で
    View→非 View の度に進行中 gesture が drop され、late confirm
    も `NoGesture`。editing selection・arrangement inspection・
    Signal Flow inspection の 3 focus channel が誤転送されない
    （SF-UX-04 Keyboard/IME/Plugin GUI focus analogue）。
  - `signal_flow_state_survives_dock_moves_and_pool_roundtrip`:
    `ViewInstanceId` key の Signal Flow state（inspection target +
    visual position）が dock move・`detach_to_pool`・`reattach` を
    越えて保持され、sibling view の state に漏れない
    （SF-UX-04 Dock/Close/Restore）。
  - `closing_view_mid_gesture_discards_preview_without_touching_project`:
    dropped-target 状態で `remove_view` すると preview は view と
    共に破棄され、project/revision/sibling inspection は不変。
    生存 view の gesture は継続可能（SF-UX-04 Close）。
  - `visual_layout_is_excluded_from_project_semantic_diff`:
    visual mutation 全 surface（move/group/fold/zoom/pan/link_focus/
    scope）+ inspection 変更の前後で `compare_snapshots` が
    `ComparisonEquality::Equal`・total 0。直後の `SetProjectName`
    で diff が出ることを確認し oracle が空虚でないことを保証
    （SF-UX-07 semantic diff 除外）。
- `docs/impl/exp/sf05-editor.md`（新規）: 監査で特定した欠落と
  各テスト設計の判断記録。
- `evidence/sv108/{coverage,verification,external-gates}.md`
  （新規）: SF-UX-01..07 の headless 被覆写像、検証実測、
  GUI-L2/EXT-L3/実 plugin/device の外部 Gate 記録。
- `docs/impl/ir/README.md`・`docs/impl/exp/README.md`: index を
  `sync_impl_index.js --apply` で再生成（生成物）。

### Decisions During Implementation

- canonical 文書チェーンは新規作成しない（Coordinator 明示指示）。
  既存 slice1..3 文書を ir/exp/evidence から参照する形に留め、
  `docs/` 配下の新規 spec/design/plan/task/test-spec は作らない。
- テストは全て `synvelis-workspace` の public API
  （`routing_gesture_*`、`signal_flow_*`、`WorkspaceManager` の
  dock/pool/focus ops、`synvelis_project::diff` の semantic diff、
  `port_capability_generation`）経由。`crates/synvelis-project/`
  の routing/source-map 実装には触れず、public read API のみ使用。
- capability-generation staleness は「gesture 中に project op で
  resolved binding の `capability_revision` を bump」する形で
  再現。window owner・revision も同時に進むが、confirm の検査順
  （window → capability → CAS revision）により capability 分岐が
  proximal cause となる。観測契約は `Stale` + project 不変で同一。
- late receipt は Applied 終端後の 4 gesture API 呼出しで検証。
  既存テストが Cancelled 終端の `NoGesture` を部分被覆するため、
  本テストは Applied 終端 + gesture slot 解放の確認に集中。
- input-context 喪失は `InputContext::captures_input()` の
  View→非 View 遷移が gesture を drop する実装契約を、
  Keyboard/IME/PluginGui/Modal の 4 variant で反復検証。
  実 OS の focus event ではなく headless の context 遷移として
  記録し、GUI-L2 は external-gates へ分離。
- semantic diff 除外は `compare_snapshots`（`ComparisonDomains`
  既定 = project semantic 全体）で before/after `ImmutableSnapshot`
  を比較。visual state は `ViewState` にあり project snapshot に
  含まれない構造的事実を、mutation 全 surface 適用後の `Equal`
  で固定。`SetProjectName` による陽性対照で oracle の非空虚性を
  保証。

## Related Experiments

### Adopted

- `docs/impl/exp/sf05-editor.md` — 被覆監査で特定した 6 件の
  headless 欠落と各 fixture 設計の判断を記録。実装は全て採用。

### Rejected

なし。

## Validation

`origin/main` 取り込み後の HEAD で実施。canonical = mise。
詳細は `evidence/sv108/verification.md`。

| コマンド | exit status |
| --- | --- |
| `mise run fmt-check` | 0 |
| `mise run check synvelis-workspace` | 0 |
| `mise run lint synvelis-workspace` | 0 |
| `mise run test synvelis-workspace` | 0（`sf_05_s2` 25 tests 全 pass、6 件追加分含む） |
| `mise run check synvelis-app` | 0 |
| `mise run lint synvelis-app` | 0 |
| `mise run test synvelis-app` | 0（全 test target green） |

## Risks

- capability-generation テストは window owner と revision も同時に
  進むため、「capability 分岐で drop した」ことの直接証明は
  内部状態を覗かない限り不可。観測可能な契約（`Stale`・project
  不変・retarget なし）は固定済みで、分岐の識別はコード上の
  検査順に依存する。
- input-context テストは `set_input_context` の API 遷移を使う
  headless 模擬であり、実 OS の keyboard/IME/plugin GUI focus
  event そのものではない。GUI-L2 実機検証は `external-gates.md`
  の NotRun/Deferred に記録。
- `sf_05_s2.rs` は既に 19 テストあり肥大化傾向。さらなる SF-05
  追加は分割を検討する余地あり（本タスクでは既存ファイルの
  慣例に従い追記）。

## Follow-ups

- SF-UX-08（実 OS/GPUI Native lifecycle・HighDPI・A11y・IME・
  実 window）: **NotRun** — #114 系の外部 Gate。
- 実 plugin（#176）・実 device の port reconfigure/probe:
  **NotRun/Deferred** — headless は `StaticProcessorHost`/
  `ProcessorBinding::Resolved` fixture のみ。
- SF-UX-01..07 の headless Pass を製品受入へ昇格しない。
  merge 後 main SHA の必須 CI 結果が統合検証の証拠。
