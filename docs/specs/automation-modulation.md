---
id: "SPEC-2i1xSQAXTQNZIpUBQGPt15"
type: "spec"
status: "approved"
title: "Automation / Modulation 統合 Parameter Control — 製品 UX・設計仕様と受入 Fixture・設計文書チェーン（synvelis#6）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/specs/pc-c5-04-recording-status.md"
    - "docs/specs/pc-c6-oracle-rows.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/aut-03-loop-write-one-pass-write-range-rearm.md"
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/specs/context-inspector.md"
    - "docs/specs/svt-product-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/automation-modulation-design.md"
    - "docs/plans/automation-modulation.md"
    - "docs/tasks/automation-modulation-task.md"
    - "docs/test-specs/automation-modulation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/1"
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/40"
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/121"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/134"
    - "https://github.com/scarecrow173/synvelis/issues/135"
    - "https://github.com/scarecrow173/synvelis/issues/136"
    - "https://github.com/scarecrow173/synvelis/issues/137"
    - "https://github.com/scarecrow173/synvelis/issues/138"
    - "https://github.com/scarecrow173/synvelis/issues/139"
    - "https://github.com/scarecrow173/ruviform/issues/186"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  defers: []
  deferred-by: []
---

# Automation / Modulation 統合 Parameter Control — 製品 UX・設計仕様

## What Should Be Built

synvelis#6（設計 Issue）の成果物として、Automation / Modulation /
Parameter Control の **製品 UX・設計仕様を定義する**。Synvelis は
汎用 DAW として Arrangement の Automation Lane（Track Automation・
Clip Envelope・Note Expression）と LFO・Envelope・Macro 等の
Modulation をサポートし、「その Parameter がどの制御によって、
どれだけ、なぜ変化しているか」を追跡可能にする。本 spec が固定する
契約領域:

1. **所有 Scope と時間座標の分離** — Issue #6 の歴史的 Scope 表
   （Track/Project・Clip-local・Note-local・Project-global）は
   CTL-01 #121 の `ControlOwner × AutomationTimeScope` 分離と
   CTL-02 #122 の `ParameterTargetId` 単一正本を優先する。旧 Scope
   表を新型の代用品にしない。所有（owner）と automation 点の時刻
   座標（time_scope）は別軸であり、Clip Envelope が Track Volume
   Automation を暗黙に上書きすることはない（別処理段階・別 Target）。
2. **競合 Automation の明示検出・解決** — 同一 Parameter・同一
   評価時刻の主要 Automation 系列は原則一つ。重複は暗黙の優先順位
   （「最後に編集した方が勝つ」「View 上の順序」）で潰さず、
   `AutomationConflict` / `ConflictPolicy::{Reject, Replace}` で
   明示的に検出・解決する。複数の編集 Region は同じ論理系列へ
   包含できる。
3. **Automation Off / Lane 表示と再生の分離** — Automation 無効化は
   Base Value への復帰を意味し、既存 Modulation は別に維持される。
   Timeline 表示の非表示は再生の停止を意味しない。Off は Modulation
   Off を意味しない。
4. **Modulation 合成規則** — `raw(t) = a(t) + Σ[d_i × f_i(s_i(t))]`
   の加算合成。複数 Assignment は順序と独立に加算され、個別の
   Depth/Polarity/Transform/Enabled は Assignment 単位で操作できる。
   合計後に対象範囲へ Clamp し、`Clamped/Saturated` を説明できる。
   Boolean/Choice/Stepped 等の離散・非線形・非対応 Parameter に
   誤った値・機能保証をしない。制御 DAG の無遅延循環は拒否する。
5. **Parameter Control Breakdown** — Base / Automation / Reference /
   各 Source 寄与 / Host-resolved / Plugin-reported / Probe 実測を
   出所付きで区別する。Host-resolved はホストが適用しようとする
   意図値であり、Plugin 内部の真の最終値と同義にしない。
6. **観測状態の区別** — `At Playhead` / `Live` / `Captured` /
   `Stale` / `Unavailable` を表示でき、時間位置・Latency・Revision・
   世代（Graph/Control/Transport/Clock Epoch）を参照可能にする。
   旧世代の観測を現在値として誤表示しない。
7. **Automation Playback / Manual / Write 状態機械** — UI Mode
   `Off / Read / Touch / Latch / Write`。Read は手動プレビュー後に
   既存 Automation へ復帰、Touch は操作区間のみ記録して復帰、
   Latch は操作後も継続するが終了後の次回再生に記録を持ち越さない。
   Write は明示 Arm + Automation Record + Transport を満たす対象
   のみへ作用し、非対象・未 Arm へ書き込まない。記録するのは
   Modulation 合成前の Manual/Reference 値であり、LFO 動作中の
   Automation 記録で二重 Modulation を発生させない。1 録音
   セッションは適切な Project Undo 単位へまとめる。
8. **Gesture の固定** — Gesture 中の Inspector Follow 更新や
   Window 移動が書き込み Target をすり替えない。編集 Command は
   Begin 時に Target/Revision/Arm を fixation として確定する。
9. **評価の安定性と精度の明示** — Loop・テンポ変更・オフライン
   レンダリング・再生で評価が安定し、control-rate 基準・Plugin
   規格ごとの精度・対応制限を明示できる。無断の Approximation や
   全 Plugin の Offline Exact を保証しない。
10. **View 間の整合** — Signal Flow / Inspector / Arrangement の
    同一制御関係表示が整合し、調査クリックは他 View の Editing
    Selection を変更しない。View の Explore/Follow/Pinned は
    Project を mutate しない。

## Why

Issue #6 は 2026-11-04〜10 の横断レビューで基本モデル・4 決定・
AUT-01〜04 / PC-C1〜06 / CTL-01/02 の実装 Issue 分解まで合意済みだが、
製品 UX・設計仕様として **Owner 境界・状態遷移・Scope/Target/Time
Mapping・画面操作 Fixture・受入条件の被覆分類**が文書として固定
されていなかった。本 spec と対応する design/plan/task/test-spec は、
その合意を **実装 Owner（#98〜103・#121/#122・#131〜134）が消費
できる契約と証拠分類**へ落とす。設計レビュー済みの契約と、実装・
実 OS・実 Plugin でしか証明できない Gate を分離し、headless 成功を
GUI Pass として主張しない。

## Users and Value

- DAW ユーザー: Arrangement の Automation Lane・Signal Flow の
  Control Layer・Inspector の Control Breakdown が同一 Project
  State の異なる投影として矛盾なく表示・編集でき、「どの制御が
  なぜ効いているか」を追跡できる。Touch/Latch/Write の記録が
  予期せぬ上書き・二重適用・対象すり替えを起こさない。
- 後続実装 slice（#98〜103・#121/#122・#131〜134・#102/#114）:
  Scope/Target/Time Mapping・状態機械・Fixture 定義から実装・検証
  できる契約を得る。
- レビュアー: 「所有」と「時間座標」が別軸であること、観測不能を
  観測値に見せないこと、headless 証拠と製品受入の区別が文書上で
  検証できる。

## Acceptance Criteria

Issue #6 §8 の受入条件を AM-01〜AM-13 として固定する。各行の現在の
被覆状況は `evidence/sv6/coverage.md` の台帳を正本とし、本 spec は
要求のみを定義する（設計のみの項目は `NotRun`）。

- [ ] **AM-01**: Arrangement の Track Automation・Clip Envelope・
  Note Expression の所有 Scope と作用先/時間座標が混同されない。
- [ ] **AM-02**: 一つの Parameter に同時評価される競合 Automation
  が生じたら、暗黙の優先順位で上書きせず明示的に検出・解決できる。
- [ ] **AM-03**: Automation 無効化が Base 復帰を意味し、既存
  Modulation は別に維持される。Lane の非表示が再生停止を意味しない。
- [ ] **AM-04**: LFO・Macro 等の複数 Assignment が順序と独立に
  加算され、個別 Depth/Polarity/Transform を操作でき、Clamp を
  説明できる。
- [ ] **AM-05**: 非線形/離散/非対応 Parameter に対し誤った値・
  機能保証をしない。循環依存の検証が機能する。
- [ ] **AM-06**: Parameter Control Breakdown で Base、Automation、
  Reference、Source 寄与、Host-resolved、Plugin-reported、
  Probe 実測を出所付きで区別する。
- [ ] **AM-07**: At Playhead/Live/Captured/Stale/Unavailable が
  表示でき、時間位置・Latency・Revision を参照可能にする。
- [ ] **AM-08**: Read で手動プレビュー→既存 Automation 復帰。
  Touch は操作区間のみ記録し復帰。Latch は操作後も継続、終了後
  次回再生に記録を持ち越さない。
- [ ] **AM-09**: Write は明示 Arm+Record+Transport を満たす対象
  だけへ作用し、非対象/未 Arm へ書き込まない。
- [ ] **AM-10**: LFO 動作中の Automation 記録で二重 Modulation を
  発生させず、1 セッションを適切な Project Undo 単位へまとめる。
- [ ] **AM-11**: Gesture 中の Inspector Follow 更新や Window 移動が、
  書き込み Target をすり替えない。
- [ ] **AM-12**: ループ・テンポ変更・オフラインレンダリング・
  再生で評価が安定し、精度/対応制限を明示できる。
- [ ] **AM-13**: Signal Flow/Inspector/Arrangement の同一制御関係
  表示が整合し、調査クリックは他 View の Editing Selection を
  変更しない。

## Non-goals

- Automation/Modulation/Parameter Control の**製品実装** —
  実装 Owner は既存 Issue: PC-C1〜06 #98〜103（Recording
  Transaction・Relation Index・ControlTrace・Plugin Binding・
  GUI 統合・Cross Oracle）、CTL-01/02 #121/#122（ControlOwner ×
  AutomationTimeScope・ParameterTargetId 正本）、AUT-01〜04
  #131〜134（Mode 境界・Region Replace・Loop Write・Curve LOD）。
  本 spec はそれらが消費する製品契約のみを定義する。
- Ruviform 共通 Control/Graph/Plugin 正本への DAW 固有 Scope の
  持ち込み、Synvelis 内の第二の Persisted Parameter/Control
  Engine や RT 上の可変割当。
- `Project::control`（`ControlModel`）の永続 schema 変更 —
  `ParameterTargetId`/`AutomationLaneId`/`ModulationSourceId`/
  `ModulationAssignmentId` の既存 stable ID をそのまま使い、
  新しい domain id を発明しない。
- Multi-pass Automation Takes・Envelope/Random Source・外部 MIDI
  CC Mapping・Audio-rate Modulation・明示 Bake — Issue #6 の
  AUT-F01〜F05（#135〜139）側の後続 Feature Gate。
- 実 GUI/実 OS/実 Plugin/実機の受入 — #102/#114/#103 の別受入
  Gate。Fake/Headless/Offscreen の成功を実機 Pass と換算しない。
- Touch Return Time/Shape の最終既定値・曲線 Decimation 許容誤差・
  TimeBase/Plugin 精度の数値閾値 — Issue #6 §H/§9 の PoC 後決定
  事項（AUT-01 の暫定 150ms Smoothstep は既存契約として参照）。
- CLI/MCP Gateway op の新設 — 既存 `syn.control.*`/`syn.probe.*`
  op 語彙の消費のみ。

## Sources and Relations

- Issue #6 本文（合意済み基本モデル + 4 決定、AUT-01〜04/PC-C1〜06
  索引、受入条件 §8、検証方法、関連 Issue 群）
- `docs/designs/automation-modulation-design.md`（設計正本:
  Owner 境界・Scope/Target/Time Mapping・状態機械・Decision Log・
  Scenario/Oracle）
- `docs/test-specs/automation-modulation.md`（画面操作 Fixture 定義）
- `evidence/sv6/coverage.md`（受入条件 × 既存被覆の台帳）
- 上流契約 spec: CTL-01 #121（`ControlOwner`/`AutomationTimeScope`/
  `ParameterRole`）、CTL-02 #122（`ParameterTargetId` 単一正本・
  canonical SendLevel spec）、PC-A #40（Control Model/Breakdown/
  状態機械）、PC-B #43（Dispatch/Note scope/RT/Offline）、
  PC-C1 #98（Recording Transaction）、PC-C2 #99（Relation Index）、
  PC-C3 #100（ControlTrace）、PC-C4 #101（Plugin Binding）、
  PC-C5 #102（UI-C5 relation/gesture/inspector/recording status）、
  PC-C6 #103（Oracle）、AUT-01〜04 #131〜134、SF-04 #107（Probe
  Adapter）、#5 Context Inspector、#114 SV-GUI-05
- 関連 Issue: #1（製品構想）、#2（Workspace/Focus/Command）、
  #3（共通インタラクション）、#4（Signal Flow/Probe）、#5
  （Context Inspector）、#7（VCS Visual Diff）、#10（Track/Clip
  Model）、#11（Timeline Editing）、#12（Recording/Take/Comping）、
  #18（Generic Execution）、#19（可変 PPQN Musical Time）、#20
  （Time/Automation v0.5）
- Ruviform 側 Owner: #185（Probe/RT Capture）、#186（Param 制御
  イベント精度・配送）、#188（Timed Input Chase）、#189（Musical
  Time）、#190（Transport）、#196（Plugin 精度）、#253（実
  Backend）、#307（Frame Base 検証済み context）
