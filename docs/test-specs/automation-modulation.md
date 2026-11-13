---
id: "TSPEC-5CuediZdfo8z7uI9MvTAHB"
type: "test-spec"
status: "approved"
title: "Automation / Modulation 統合 Parameter Control — テスト仕様・受入 Fixture（synvelis#6）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/automation-modulation.md"
    - "docs/designs/automation-modulation-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/automation-modulation.md"
    - "docs/designs/automation-modulation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "evidence/sv6/coverage.md"
    - "docs/plans/automation-modulation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies:
    - "docs/specs/automation-modulation.md"
    - "docs/designs/automation-modulation-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
  defers:
    - "実 Window/OS GUI・実 device・実 plugin を要する product E2E（#102/#114/#103 の別受入 Gate）"
  deferred-by: []
---

# Automation / Modulation 統合 Parameter Control — テスト仕様・受入 Fixture

## 目的

spec の AM-01〜13 と design の画面操作 Fixture（F-UX-01〜05）を、
検証可能な **Feature / Rule / Example / Guarantee** 構造で固定する。
本 slice は docs-only のため、各 Fixture は「既存 headless 証拠の
写像」または「後続実装/Gate が満たすべき契約」として定義し、
未実装の設計動作を `Pass` として主張しない。被覆の現在値は
`evidence/sv6/coverage.md` を正本とする。

## 検証レベルと状態語彙

POLICIES.md の規範に従う:

- `Pass (headless)`: 既存 headless/unit テストが契約を実証済み。
- `Partial`: 一部経路・一部状態のみ実証済み。
- `NotRun`: 設計契約はあるが本 slice で未検証。
- `Deferred`: GUI/実機/実 Plugin 等の後続 Gate に委譲。
- `Unsupported`: 当該 capability を提供しないことを明示。

Requirement 状態（`Required`/`Deferred`/`NotApplicable`）と
実行結果（`Pass`/`Fail`/`NotRun`/`Unsupported`）は混同しない。

## Features

### Feature AM-F1 Scope・Target・Time 分離（AM-01）

Track Automation・Clip Envelope・Note Expression が
`ControlOwner × AutomationTimeScope` の 2 軸で区別され、
作用先 `ParameterTargetId` と時間座標が混同されない。

- **Rule R1**: 各 Automation 種別は一意の
  `(ControlOwner, AutomationTimeScope)` を持ち、旧 Scope 表の
  表示名で代用しない。
- **Rule R2**: Clip Envelope と Track Automation は別 Target・
  別処理段階であり、暗黙の上書きをしない。
- **Rule R3**: Clip Envelope を他 Parameter へ適用するには
  明示 `ModulationAssignment` が必要。
- **Example E1**: Clip を移動しても Clip-local Envelope は Clip
  に追従し、Track Volume Automation は Project 座標で不変。
- **Guarantee G1**: `pc_a_02_scope_separation_clip_follows_move`・
  `pc_b_02_*`・`ctl_01.rs` の headless 契約を満たす
  （`Pass (headless)`）。製品 Arrangement での 3 種別同時表示は
  `Deferred`（#102/#114）。

### Feature AM-F2 競合 Automation の明示解決（AM-02）

同一 Parameter・同一評価時刻の競合を暗黙優先順位で潰さない。

- **Rule R4**: 重複する主要系列は `AutomationConflict` として
  検出し、`ConflictPolicy::{Reject, Replace}` の明示解決を要求。
- **Rule R5**: last-edit-wins・View 順序を暗黙の優先順位にしない。
- **Example E2**: 同一 Target に 2 系列を書き込もうとすると
  conflict が返り、明示 Replace でのみ解決される。
- **Guarantee G2**: `pc_a_03_conflict_reject_and_replace`・
  `aut_02.rs`・`aut_03_replace_collision_preserves_outside_curve`
  の headless 契約（`Pass (headless)`）。UI での conflict 解決
  操作は `Deferred`。

### Feature AM-F3 Off/Base/Modulation/Lane 可視性（AM-03）

- **Rule R6**: Automation Off は Base 復帰を意味し、Modulation
  を停止しない。Off≠Modulation Off。
- **Rule R7**: Lane の表示/非表示は View 状態であり Playback/
  Automation を変更しない。
- **Example E3**: Automation Off + 有効 LFO Assignment →
  Host-resolved は Base+Modulation、Automation=0。
- **Guarantee G3**: `pc_a_01`/`pc_a_05_mode_select_never_mutates_
  and_off_read_behave`・`automation_view_is_read_only_and_never_
  mutates_source`（aut_04）で `Pass (headless)`。Lane hide の
  製品 GUI 操作は `Deferred`。

### Feature AM-F4 Modulation 合成・Clamp・Capability（AM-04/05）

- **Rule R8**: `raw(t)=a(t)+Σ[d_i×f_i(s_i(t))]`。複数 Assignment
  は順序・DAG 位置と独立に加算。集約順序は固定。
- **Rule R9**: Depth/Polarity/Transform/Enabled は Assignment
  単位。合計後 Clamp、`Clamped/Saturated` を説明。
- **Rule R10**: Boolean/Choice/Stepped/非対応は明示 capability/
  変換/量子化がある場合のみ。無ければ `Unsupported`/`Unavailable`。
- **Rule R11**: 無遅延 Cycle は `ControlGraph` 構築時に拒否。
- **Example E4**: LFO+Macro の 2 Assignment で順序を変えても
  同一結果。Cycle 構成で拒否。
- **Guarantee G4**: `pc_a_06`/`pc_a_07`/`pc_c4_02`/`pc_c4_03` で
  `Pass (headless)`。非線形表示（Depth%≠Hz）の製品 UI は
  `Deferred`。

### Feature AM-F5 Control Breakdown の出所分離（AM-06）

- **Rule R12**: Base/Automation/Reference/各 Source 寄与/
  Host-resolved/Plugin-reported/Observed Output を出所付きで
  区別。
- **Rule R13**: Host-resolved≠Plugin 内部最終値。Plugin-reported/
  Observed が無ければ `Unavailable`（推測値なし）。
- **Example E5**: Probe 未接続で Observed Output=`Unavailable`、
  Plugin report 無しで Plugin-reported=`Unavailable`。
- **Guarantee G5**: `pc_a_04`/`trace_c3_*`/`pc_c4_03_observation`
  の語彙で `Pass (headless)`。製品 Breakdown UI は `Deferred`
  （#102）。

### Feature AM-F6 観測状態・鮮度（AM-07）

- **Rule R14**: `At Playhead`/`Live`/`Captured`/`Stale`/`Unavailable`
  を区別し、評価時刻・Latency・Revision・適用世代を参照可能に。
- **Rule R15**: 旧世代（Graph/Control/Transport/Clock Epoch 不一致）
  を現在値として表示しない。
- **Example E6**: Transport seek/epoch 変更後に旧 trace が
  `Stale` になり現在値と誤表示されない。
- **Guarantee G6**: `trace_c3_02`/`pc_c4_07`/`sf04_*` で
  `Pass (headless)`。3 View での鮮度表示 UI は `Deferred`。

### Feature AM-F7 Mode 状態機械（AM-08/09/10/11）

- **Rule R16**: Off/Read/Touch/Latch/Write を `PlaybackState`・
  `WriteMode`・`WriteArm`・`GestureState`・`RecordingTransaction`
  に分離。Mode 選択のみでは mutate しない。
- **Rule R17**: Read=一時優先→復帰、Touch=操作区間のみ記録→復帰、
  Latch=操作後継続だが次回再生へ持ち越さない、Write=Arm+Record+
  Transport を満たす Target のみ。
- **Rule R18**: 記録は pre-modulation Manual/Reference（double-
  modulation 防止）。1 セッション=1 Undo。Begin 時に Target/
  Revision/Arm を fixation。
- **Example E7**: Latch 終了→Transport 停止→再生開始で記録が
  自動再開しない（明示 rearm が必要）。
- **Guarantee G7**: `aut_01_*`/`aut_03_*`/`pc_c6_r07`/`rec_c1_*`/
  `uic5_02_*`/`pc_c6_ui04`/`pc_c6_r04` で `Pass (headless)`。
  実 Window ジェスチャは `Deferred`（#114）。

### Feature AM-F8 評価安定性・View 整合（AM-12/13）

- **Rule R19**: Loop・Tempo・Offline・再生で Musical 座標の安定
  評価。RT/Offline parity。精度/制限を明示。Loop wrap で seal、
  次 pass は明示 rearm。
- **Rule R20**: Signal Flow/Inspector/Arrangement は同一
  Relation/Trace を投影。調査クリックは Editing Selection を
  変更しない。Follow/Explore/Pinned は非破壊。
- **Example E8**: Loop 再生で wrap 時に transaction が seal し、
  以降の pass が書き込まない。Signal Flow の辺クリックで
  Arrangement の選択が変わらない。
- **Guarantee G8**: `aut_03_*`/`pc_a_08`/`pc_b`/`pc_c4_01`/
  `rel_c5`/`uic5_*`/`pc_c6_ui*`/`sf_05*` で `Pass (headless)`。
  実 Plugin 精度・3 View 同時 GUI は `Deferred`（EXT-L3/#102/#114）。

## Non-goals（テスト対象外）

- 実 Window/OS GUI・実 device・実 plugin を要する product E2E —
  #102/#114/#103 の別受入 Gate（`Deferred`）。
- Multi-pass Takes・Envelope/Random Source・外部 MIDI CC・
  Audio-rate Modulation・Bake — AUT-F01〜F05（#135〜139）。
- 数値閾値（Touch Return・LOD・精度）— PoC 後決定事項であり
  本 test-spec は既存契約の参照のみ。
- 第二 Control Engine・新 schema — `ControlModel` を正本とする。

## Risk

- **被覆の過大主張**: 設計のみの項目を `Pass (headless)` と誤記
  する危険。→ `evidence/sv6/coverage.md` で AC 毎に根拠テストを
  列挙し、無いものは `NotRun`/`Deferred`/`Unsupported`。
- **語彙の分岐**: 新しい観測状態・scope 名を発明すると既存
  `TraceFreshness`/`ObservationState`/`ControlOwner`/
  `AutomationTimeScope` と不整合。→ 既存型名のみ使用。
- **Fixture と実装の乖離**: AM-UX-* が実装と異なる操作を仮定。
  → 各 Fixture に対応する既存テスト名を併記し、差異は coverage
  の `Partial`/`Deferred` で明示。
