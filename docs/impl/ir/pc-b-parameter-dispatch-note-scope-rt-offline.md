---
id: "IMPL-034PCB07Kq2wXyZmAbCdEf"
type: "impl"
status: "completed"
title: "PC-B parameter dispatch/note-scope/RT/offline headless PoC"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/188"
    - "https://github.com/scarecrow173/ruviform/issues/197"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/dispatch.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/pc_b_rt.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_b.rs"
      - type: "file"
        path: "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
      - type: "file"
        path: "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
      - type: "file"
        path: "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
      - type: "file"
        path: "docs/plans/pc-b-parameter-dispatch.md"
      - type: "file"
        path: "docs/tasks/pc-b-task1-dispatch-note-rt-offline.md"
      - type: "file"
        path: "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
      - type: "file"
        path: "docs/impl/ir/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-player/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
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
    generated: []
  implements:
    - "docs/plans/pc-b-parameter-dispatch.md"
    - "docs/tasks/pc-b-task1-dispatch-note-rt-offline.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-b-e2e-parameter-dispatch.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-B parameter dispatch/note-scope/RT/offline headless PoC

## Summary

Issue #43 — PC-A (issue #40) が「fixture capability までの正直な
実装」で終えた parameter-control model の残件 4 領域を実装した。
(1) host-resolved value を実 ruviform plugin bridge 経路
(`PluginHost` → `attach` → `PluginNode` → plan install → endpoint
`ParamEvent`)へ dispatch する `synvelis-player::ParamDispatch`、
(2) `ControlScope::Note` の note entity + note-relative 座標解決、
(3) prepared `EvalPlan::eval_rt` + `lower_event` 成功パスの
allocation-free を `stats_alloc` で計測する rt_ テスト、
(4) playback drive と offline `render()` の配送イベント一致。
project `SCHEMA_VERSION` は 3 → 4。

## Context

Spec: docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md
ADR: docs/adr/pc-b-dispatch-boundary-note-entity-prepared-eval.md
Design: docs/designs/pc-b-parameter-dispatch-note-scope-design.md
Plan: docs/plans/pc-b-parameter-dispatch.md / Test spec:
docs/test-specs/pc-b-e2e-parameter-dispatch.md /
Task: docs/tasks/pc-b-task1-dispatch-note-rt-offline.md。

## Implementation

### Changes

- `Cargo.toml` / `crates/synvelis-player/Cargo.toml`:
  `ruviform-plugin-core`/`ruviform-plugin-host`/
  `ruviform-audio-plugin-bridge`/`ruviform-exec` を player の
  依存に追加 (実行系 crate が plugin bridge 境界を持つ配置)。
  `synvelis-project` dev-deps に `stats_alloc` (rt_ 計測)。
- `synvelis-project/src/project.rs`: `Note` entity 追加 — stable
  `NoteId`、owning `ClipId`、key (u8)、clip-relative `start`/`len`
  (`BeatPoint`、len>0)。`Project.notes: BTreeMap` は
  `#[serde(default)]`。`validate_references` に note.clip 存在・
  len 有限・target/source owner の note 存在を追加。
- `synvelis-project/src/ops.rs`: `AddNote`/`RemoveNote` を追加。
  `RemoveClip` は保有 note を cascade 削除。`validate_scope` が
  `ControlScope::Note` の実体存在を要求 (missing → `NotFound`)。
  note-scoped target は note→clip 参照を pin し、target 残存時の
  `RemoveClip` は post-apply oracle で atomic reject。
- `synvelis-project/src/control.rs`: `resolve_scope_pos` の
  `Note` arm — project pos → clip-relative → `[start,start+len)`
  内なら note-relative `BeatPoint`、外は `ScopeUnavailable`、
  note 不在は `NotFound`。`EvalPlan` を prepared 化 (owner
  scope/base/kind/merged sorted series/assignment/depth-source
  index を compile 時に固定) + `EvalScratch` (caller 所有
  `values`/`computed` buffer) + `eval_rt` — 成功パス alloc-free の
  control-rate 評価。`lower_event` は PC-A のまま。
- `synvelis-project/src/store.rs`: `SCHEMA_VERSION` 3 → 4
  (notes 追加。v3 body は deserialize 可能だが `load()` の厳格
  gate で `UnsupportedSchema` — no-migration ポリシー継続)。
- `synvelis-workspace/src/control.rs`: `gesture_end`/
  `record_write`/`record_points` が `&Project` を受け、記録点を
  `resolve_scope_pos` で scope 座標へ正規化してから格納。
  scope 境界 crossing は `ScopeUnavailable` 点を skip、他の
  project error は伝播。
- `synvelis-workspace/src/session.rs`: `control_gesture_end` が
  `control_with(|c,p| c.gesture_end(p, target))` 経由に。
- `synvelis-player/src/dispatch.rs`: `ParamDispatch` — target→
  `ParamId` binding (`check_param_write` で bind 時検証)、
  `TimedParamEvent` 列の `install_plan` 経由 bridge 投入、
  `DispatchStage` ledger (`Staged`/`Dispatched`/rejection)、
  `observe` (span 窓の配送観測)。`DispatchError::WriteRejected`
  は upstream rejection token (`param_unknown`/`param_mismatch`/
  `param_unresolved`/`param_capability`/`queue_full`)を保持した
  まま上位へ伝播。
- `crates/synvelis-player/tests/pc_b.rs`: 実 `PluginHost`+fixture
  factory+`attach` での PC-B-01 (実経路配送・bind/dispatch/queue
  reject 伝播・queue-full 型付き報告) と PC-B-04 (playback
  `RtRuntime` drive vs offline `render()` で endpoint 観測
  (param, abs frame, value) 完全一致)。
- `crates/synvelis-project/tests/pc_b_rt.rs`: `stats_alloc`
  `Region` で eval_rt→lower_event→pre-reserved push の測定窓を
  64 回転し allocations/deallocations==0 を assert + 同一 test
  内で `evaluate_control` との等価性を検証 (測定の並列
  cross-talk を防ぐため単一 test に集約)。
- `crates/synvelis-workspace/tests/pc_b.rs`: PC-B-02 — note-relative
  評価/記録、clip move で domain 追従、domain 外
  `ScopeUnavailable`、RemoveClip pin/cascade。
- `crates/synvelis-workspace/tests/pc_a.rs` /
  `crates/synvelis-project/tests/md_a.rs`: PC-B で有効化された
  note scope/schema v4 に合わせ期待値更新 (note target missing →
  `NotFound`、schema pin 3→4)。

### Decisions During Implementation

- dispatch 境界は `synvelis-player` に配置 — 実行系 crate が
  plugin bridge/adapter を所有し、`synvelis-project` は
  upstream 型に依存しない純粋 model のまま (ADR どおり)。
- `eval_rt` は成功パスのみ alloc-free を契約化。エラーパスの
  `format!`/`NotFound(String)` は ruviform RT 契約テスト慣行
  どおり測定対象外。`computed` flag は呼出ごとに reset し
  stale cache を防ぐ。
- offline render 一致性は upstream gate を正直に扱う:
  `PluginNode::offline_support()` は `Unknown` のまま (bridge
  node の offline 等価性は ruviform 未検証 — rf#197 系統)。
  テスト側は stateless・transport 決定論的な fixture
  インスタンスに限り `OfflineRenderSupport::Verified` を宣言する
  委譲ラッパを `NodeFactory` 経由で装着し、宣言の検証根拠を
  一致性アサート自体とした (instance-level verification は
  tri-state 契約の意図する使い方)。
- note-scoped target の clip 削除 pin は post-apply
  `validate_references` oracle に任せ、dangling owner を特別扱い
  しない (clip-scoped target と同じ規則)。
- RT 計測は binary 内の並列 test が global allocator を共有する
  ため単一 test に集約 (等価性 assert を同じ test に fold)。

### Review Fixes Applied

review-protocol の adversarial レビューで指摘され、コードで
確認のうえ修正したもの:

- **`ParamDispatch::dispatch` の部分 staging(P1)**: `install_plan`
  成功後に `request(Play)` が `QueueFull` を返すと、エラー返却
  なのに plan が node に残り ledger と乖離した。`Play` を先に
  要求する順に変更 — Play は冪等 transport 状態なので、
  `install_plan` が失敗しても orphan plan は残らない。
- **`bind` の u64 変換ガード**: `ParameterTargetId::raw()` (u128)
  → `TargetId` (u64) は無条件 truncate だった。範囲外は
  `param_target_out_of_range` で拒否。
- **`bind` の共有 param guard**: ledger は `(node, param)` 軸で
  帰属するため、同一 param への別 target bind は誤帰属を
  引き起こす。`param_in_use` で拒否し、同一 target の re-bind は
  旧所有エントリを掃除する。
- **`observe` の plan 引数廃止**: 呼び出し側が任意の plan を渡せる
  と陳腐/異系 plan で誤帰属し得た。`ParamDispatch` が
  `last_plan` を保持し常にそれで投影する。
- **`RemoveNote` doc の誤記**: 「note-scoped target は残る」は
  post-apply oracle と矛盾 — 正しくは参照残存時に reject。
- **`SplitClip` が note を無視**: 分割点を跨ぐ note を残すと
  domain の尾が失われ silent corruption。跨ぐ note は
  `Validation` で拒否、分割点以降の note は clip B へ
  re-parent + re-base (評価 domain の絶対位置を保存、
  `pc_b_02_split_clip_*` で検証)。
- **`AddNote` の domain 検証不足**: `key > 127`、musically
  anchored でない clip、`start+len > clip extent` を `Validation`
  で拒否 (`pc_b_02_add_note_validates_domain`)。
- `project.rs` の serde コメント誤記修正 (v3 body は deserialize
  できるが `load()` は v4 を要求)。

レビューで発見し、PC-B の原因でないため変更しなかったもの:

- `gesture_end` Off-mode の空 gesture で `BaseEdit{value:last}`
  を commit する (PC-A 既存動作、`last` は gesture_begin 時点の
  評価値) — 別 issue 候補として報告。
- `dispatch(&[])` は空 plan + Play を staging する — 有害では
  ないが無駄。呼び出し側契約として記録のみ。
- `ParamDispatch` は `HeadlessDriver` 配線外 (spec どおり
  caller が attach から直接組み立てる) — 統合は follow-up。

## Related Experiments

### Adopted

なし — 承認済み設計どおりに実装。

### Rejected

- upstream `PluginNode` を直接改変して `offline_support` を
  Verified にする案 — ruviform 側の未検証宣言を Synvelis 側で
  捏造することになり、multi-repo 契約に反する。
- note-scoped target 残存時の `RemoveClip` を dangling 許容で
  通す案 — post-apply oracle の「loader が reject する snapshot を
  commit しない」不変条件を壊す。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt` / `fmt-check` | PASS |
| `mise run check` | PASS (workspace --all-targets) |
| `mise run lint` | PASS (`cargo clippy --workspace --all-targets -- -D warnings`) |
| `mise run test` | PASS (`cargo test --workspace --locked` — pc_b 系 10 test + rt_ 1 test 含む全 crate green) |
| `git merge origin/main` | Already up to date (88026ed) |

受入対応: PC-B-01 `pc_b_01_*` (実 bridge 経路で endpoint が
(param,abs frame,value) を受信、bind `param_unknown`/dispatch
`param_unresolved`/`param_mismatch`/queue-full が typed token で
伝播、rejection で staging なし)、PC-B-02 `pc_b_02_*`
(note-relative 評価・記録・clip move 追従・domain 外
`ScopeUnavailable`・pin/cascade)、PC-B-03
`rt_control_eval_and_lower_alloc_free` (eval_rt+lower_event 成功
パス alloc/dealloc==0 + evaluate_control 等価)、PC-B-04
`pc_b_04_offline_render_matches_playback` (playback drive と
`render()` で endpoint 観測列完全一致)。

## Follow-ups

- ruviform 側で bridge node の `offline_support` が検証・宣言
  されたら `VerifiedOfflineNode` wrapper を撤去し裸の
  `PluginNode` で `render()` を通す (ruviform issue 起票候補 —
  coordinator 経由で確認)。
- `ParamWriteMeans::PerNote` の真の note 寿命配送 (中立契約
  未対応のまま — PC-A 引継ぎ残件)。
- 実 VST3 plugin 統合 (rf#253 gate、本 issue スコープ外)。
- 実 GUI / automation lane 表示 / Control Mapping (同じく
  スコープ外)。
