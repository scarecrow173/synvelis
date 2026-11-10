---
id: "IMPL-034VntRmtjh1jy2mJ1rPum"
type: "impl"
status: "completed"
title: "PC-C6 oracle R/PL/UI/COMP headless fixture 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c6_oracle_pl.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs"
      - type: "file"
        path: "docs/specs/pc-c6-oracle-rows.md"
      - type: "file"
        path: "docs/designs/pc-c6-oracle-rows-design.md"
      - type: "file"
        path: "docs/plans/pc-c6-oracle-rows-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c6-task1-oracle-rows.md"
      - type: "file"
        path: "docs/test-specs/pc-c6-oracle-rows.md"
      - type: "file"
        path: "docs/impl/ir/pc-c6-oracle-rows-slice.md"
    modified:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/tasks/pc-c6-task1-oracle-rows.md"
    - "docs/plans/pc-c6-oracle-rows-plan.md"
    - "docs/specs/pc-c6-oracle-rows.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/pc-c6-task1-oracle-rows.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c6-oracle-rel-tr-headless-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/designs/pc-c6-oracle-rows-design.md"
    - "docs/plans/pc-c6-oracle-rows-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  verified-by:
    - "docs/test-specs/pc-c6-oracle-rows.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C6 oracle R/PL/UI/COMP headless fixture 実装記録

## Summary

Issue #103（PC-C6 Control Oracle）の残り headless oracle 行 —
PC-C1-R01〜R08（Recording Safety）・PC-C4-PL01〜PL08（Plugin
Binding/Dispatch + RT noalloc）・PC-C5-UI01〜UI07（Control
Surface）— を独立 oracle fixture test として実装した。
`crates/*/src/`・既存 test・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
CI 設定は一切変更せず、新規 test 4 ファイルと doc チェーンのみ。
`synvelis-workspace` への新規 test 追加は Coordinator 承認済みの
範囲拡張（Orca message `msg_7802648feb93`）。

REL01〜07 / TR01〜08 は PR #212（`f329f48`）で main 統合済みのため
本 slice では回帰確認のみ（`mise run test` で既存
`pc_c6_oracle_rel`/`pc_c6_oracle_tr` が pass）。

## Context

Spec: docs/specs/pc-c6-oracle-rows.md
ADR: docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md（同一 oracle
方式の先例）
Design: docs/designs/pc-c6-oracle-rows-design.md
Plan: docs/plans/pc-c6-oracle-rows-plan.md
Test spec: docs/test-specs/pc-c6-oracle-rows.md
Task: docs/tasks/pc-c6-task1-oracle-rows.md

前提実装: PC-C1 recording transaction（`synvelis-workspace`
control/recording）、PC-C4 plugin binding/dispatch
（`synvelis-player`）、PC-C5 relation view/gesture
（`synvelis-workspace`）。fixture 構築は `tests/pc_c1.rs`・
`tests/pc_c4_binding.rs`・`tests/uic5.rs`・`tests/tim02_midi_rt.rs`
の流儀を踏襲。

## Implementation

### Changes

- `crates/synvelis-player/tests/pc_c6_oracle_pl.rs`（新規・8 tests）:
  `pc_c4_binding.rs` の Rig 流儀（FakeHostedPlugin +
  AttachClockContext + PluginBindingSession + ParamDispatch +
  delivery log）で PL01〜PL08 を照合。frame_base/identity/
  instance/node/capability gen/graph gen/param id の pin、
  shared param・stale record・plan staleness の typed rejection、
  QueueFull 型付き観測、batch atomic reject（1 stale で全体
  reject・staging なし）、generation drift/reload fail closed、
  wrong-axis typed rejection と delivery log 上の誤配送 0、
  headless boundary marker（実 capability 未測定を Pass にしない）
  をカバー。
- `crates/synvelis-player/tests/pc_c6_oracle_pl_rt.rs`（新規・
  1 test）: `stats_alloc` は process-global allocator を観測する
  ため独立 test binary。warm-up 8 回（256 frames）の後、64 回の
  `drive.process(256)` 計測 window で allocations=0/
  deallocations=0 を assert し、宣言 batch の全 event が宣言
  absolute frame に配送されること（min process frame offset）を
  delivery log で照合（`rt_pl05_param_delivery_alloc_free`）。
- `crates/synvelis-workspace/tests/pc_c6_oracle_rec.rs`（新規・
  8 tests）: `OracleModel`（宣言的 fixed context: revision/
  target/lane/epoch/scope/pass/seal）を保持し、
  `control_with` 経由で駆動した `RecordingTransaction` の全
  fixed field・seal 理由・commit/abort/rebase/preview outcome・
  undo 深さ・lane points/columns 保持を field 単位で照合。
  R01 fixed context、R02 Composite CAS/stale + 点列非消費、
  R03 構造的 rejection でのデータ保持、R04 commit 1 Undo
  exactly-once + abort 無適用、R05 明示 rebase → 再 preview →
  commit、R06 LoopWrap seal + pass 前進 + sealed commit、
  R07 mode/arm gating 行列、R08 固定 scope の project drift 耐性
  をカバー。
- `crates/synvelis-workspace/tests/pc_c6_oracle_ui.rs`（新規・
  7 tests）: fixture `ProjectOp` 列から導出した宣言 edge 集合
  （`declared_edges`）と `relation_rows`/`relation_rows_with` の
  RelationView（coverage・revision・rows 全件）を照合。UI01 唯一
  正本、UI02 RelationNav 双方向 round-trip、UI03 pinned stale →
  Stale + 空 rows、UI04 2 window 移動で gesture Stale drop +
  cancel 無 undo boundary、UI05 Begin/Preview/Commit/Cancel +
  成功 1 Undo、UI06/UI07 headless boundary marker をカバー。
- doc チェーン 5 件 + impl record + 生成 index 6 件。

### Decisions During Implementation

- PL fixture は `pc_c4_binding.rs` の Rig を test 内に再構成
  （共有 helper 化は既存 test 変更を伴うため行わず、新規 file
  内で完結）。fixture 値は先例と別系統にし独立 oracle 判定を担保。
- RT 計測は `tim02_midi_rt.rs` と同じ `stats_alloc` +
  `Region`/`INSTRUMENTED_SYSTEM` 方式。setup・plugin attach・
  dispatch staging・warm-up は全て計測 window 外。判定は
  allocations/deallocations = 0 のみ（timing 閾値なし・
  baseline 未確定のため）。
- Recording oracle は transaction の公開 field を直接比較する
  model-vs-live 方式 — `control_with`/`ControlSession` の公開面
  のみを駆動し内部状態機械に立ち入らない。
- UI oracle の edge 導出は fixture の `ProjectOp` 宣言から
  canonical direction で再構成（`RelationIndex` 非参照）。
  `RelationViewOptions` の default `max_depth=1` を踏まえ、
  全 edge 集合の照合は `max_depth: 2` を明示 — depth 1 では
  source→assignment hop が到達不能であることを観測として確認。
- `RelationNav::pop` は「pop した anchor を捨てて直前 anchor を
  復元して返す」契約 — oracle はその規則で期待値を導出。
- COMP01〜05: headless 実施可能部分（Gate A の AI Off 通常
  DAW headless 経路・Gate B の構成/観測/推定分離・Gate C 双方向
  探索の relation 証拠）は本 slice の R/PL/UI/REL/TR oracle Pass
  を構成証拠として参照。実 GUI/OS/plugin/両 Consumer を要する
  判定は `NotRun` — ledger 分類は下記 Validation に記録。

## Related Experiments

### Adopted

<!-- なし — 実験 log は未使用。 -->

### Rejected

<!-- なし。 -->

## Validation

canonical（`origin/main` = `0f15294`、merge 済み・差分なし。
実施 commit `fdf5807`、OS: Windows 11 x64 / Git Bash、
toolchain: mise rust 1.99.0）:

- `mise run fmt-check` → exit 0
- `mise run check` → exit 0
- `mise run lint` → exit 0（clippy `-D warnings`、新規 file の
  type_complexity/used_underscore_binding/match_like_matches/
  assert_is_empty/cloned_instead_of_copied を修正済み）
- `mise run test` → exit 0（workspace 全 binary `test result: ok`、
  失敗 0。新規: `pc_c6_oracle_pl` 8 tests・`pc_c6_oracle_pl_rt`
  1 test・`pc_c6_oracle_rec` 8 tests・`pc_c6_oracle_ui` 7 tests
  全 pass。PR #212 の `pc_c6_oracle_rel`/`pc_c6_oracle_tr` も
  pass = 回帰確認）
- `mise run test-rt` → exit 0（`rt_` filter で
  `rt_pl05_param_delivery_alloc_free` が選択・pass）
- `mise run test-msrv` → exit 0（rust@1.89.0 で workspace 全
  test pass、新規 4 binary も実行・pass）

補助証拠（canonical = mise、直接 cargo は型確認のみ）:
`cargo check -p synvelis-workspace --tests` /
`cargo check -p synvelis-player --tests` → warning なし。

### 証拠 ledger（固定 Case 分類）

| Case 群 | 分類 | 根拠 |
| --- | --- | --- |
| PC-C1-R01〜R08 | Pass（headless oracle） | `pc_c6_oracle_rec.rs` 8 tests pass。実 RT device/実機録音は NotRun |
| PC-C4-PL01〜PL08 | Pass（headless oracle + fake plugin） | `pc_c6_oracle_pl.rs` 8 tests + `pc_c6_oracle_pl_rt.rs` 1 test pass。実 VST3/CLAP capability・実 plugin GUI は NotRun |
| PC-C5-UI01〜UI07 | Pass（headless oracle） | `pc_c6_oracle_ui.rs` 7 tests pass。実 GPUI/OS focus/IME/plugin GUI は NotRun |
| PC-C2-REL01〜07 | Pass（回帰） | PR #212 実装済み・`mise run test` で pass |
| PC-C3-TR01〜TR08 | Pass（回帰） | 同上 |
| PC-C6-COMP01〜COMP05 | NotRun（実機判定部分） | headless 実施可能部分は上記 oracle Pass を構成証拠として記録。実 GUI/OS/plugin/両 Consumer・候補 SHA 横断判定は後続 gate |

Fixture: Synvelis commit `fdf5807`（base `0f15294`）、
Cargo.lock Ruviform SHA `0f5b57372e9a43f84415f7964b2f9636774174f0`、
OS Windows 11 x64 / Git Bash、Backend headless（FakeWindowHost・
fake hosted plugin・GraphRuntime）。CI/PR URL は PR 本文を参照。

review-protocol: objective checks（上記 6 コマンド）全 pass 後、
cooperative/adversarial の 2 pass で docs+code を自己レビュー。
CONFIRMED blocker なし → ACCEPT 相当。

## Risks

- oracle は契約文書の独立再実装であり、文書自体の誤読は両者に
  共通し得る（test-spec の case 表と impl の doc comment を照合
  済み）。実装側を oracle に合わせる変更は行っていない。
- `synvelis-workspace` 追加は承認済み範囲内の新規 file のみ。
  manifest/`src/` への変更なし（`git status` で確認済み）。
- doc graph router は main 既存の broken relation で引き続き
  blocked の可能性あり（本 slice の文書チェーンに新規 finding
  なし — 全 relation path を実在 file に解決済み）。

## Follow-ups

- PR 本文に検証結果・NotRun 一覧・review-protocol 結果を記録。
- 残件: 実 GUI/OS/plugin/RT device/両 Consumer 実機照合（#114
  gate）、`synvelis-app` GUI scenario 対応表（別 Worker）、
  TRACE-C3-05 dispatch ledger 結合（別 dispatch）、COMP 実機判定
  （全行統合後）、Ruviform #28/#219/#294 consumer 照合（別 gate）
  — 全て spec `defers` に記録済み。
