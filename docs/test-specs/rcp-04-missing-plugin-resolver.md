---
id: "TSPEC-034Xf6cGmdSv6ezz0A7JNC"
type: "test-spec"
status: "approved"
title: "RCP-04 Missing Plugin Resolver・Execution Intent・Safe Plan Reconciliation 検証仕様 (synvelis#293)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
  depends-on:
    - "docs/specs/render-checkpoint.md"
    - "docs/designs/rcp-render-recipe-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/playback-graph.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcp-render-recipe-design.md"
  derived-by: []
  verifies:
    - "docs/specs/render-checkpoint.md"
  verified-by: []
  references:
    - "docs/impl/ir/rcp-04-missing-plugin-resolver.md"
    - "docs/impl/exp/rcp-04-missing-plugin-resolver.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# RCP-04 Missing Plugin Resolver・Execution Intent・Safe Plan Reconciliation 検証仕様 (synvelis#293)

## Purpose

Issue #293（RCP-04）の headless slice が保証すべき検証意図を固定する。
正本は `docs/specs/render-checkpoint.md` §Subgraph
Substitution / Execution Intent・§Eligibility と
`docs/designs/rcp-render-recipe-design.md` の
Plugin-Missing Execution Modes and Last-good Safety。実装は
`crates/synvelis-player` 内の typed contract（Fake/Domain
Oracle）であり、workspace semantic→selection integration・
実 Plugin/実 OS/実 RT/PCM 実測は本 test-spec の対象外
（#296 RCP-GATE・別 Worker lease）。本書は実装・fixture の
変更で保証が崩れた場合に退役・改訂の判断材料とする。

## Feature

欠落 Plugin を含む ResolvedGraphSnapshot を、検証済み Render
Substitution と実行可能 Live Region へ安全に分解し、
Execution Intent（MainPlayback / EditingPreview /
BranchAudition / OfflineRender / Export）ごとに
Plan / Blocked / Preview を返す Resolver、および edit 分類
（NoRebuild / ControlOnly / Rebuild）と stale publish 拒否・
Last-good 保持を行う Safe Plan Reconciliation。

## Rules

- Rule: Intent ごとの等価性境界 — MainPlayback は全 required
  output を project-authoritative に正確再生する Plan のみを
  返し、非等価音・未解決を Exact と偽装しない。EditingPreview /
  BranchAudition は選択 Live Region の編集を優先し、結果に
  非等価を明示する。OfflineRender / Export は recipe 駆動で
  fail-closed。
- Rule: Plugin 検証の fail-closed — identity/availability/
  state restore/port layout/host capability のいずれかが
  判定不能・不一致なら typed verdict（Missing /
  StateUnknown / PortLayoutMismatch / Unsupported / Denied）
  を返し、推測 byte/parameter/port を生成しない。
- Rule: Subgraph cutset の境界完全性 — 置換 region は境界上の
  必要な全 output port を artifact が domain・format・
  channel map・PDC/latency・sidechain context 一致で賄い、
  境界 input は artifact の pinned input として会計される。
  未収録の必須 MIDI/Event/Control output、二重加算、
  dangling edge、Control 誤配送はいずれも Rejected。
- Rule: Artifact 適格性 — MainPlayback/OfflineRender/Export
  は EligibleExact のみ、preview intent は明示 opt-in 時のみ
  EligiblePreview/PreviewOnly を許し、結果に非等価を表示する。
  Unsupported/Corrupt/Stale/ACL Denied/Unknown artifact は
  候補から除外し理由を receipt に残す。
- Rule: Edit 分類 — graph structure digest・capability
  generation・format generation・clock epoch・runtime
  instance が全て同値なら NoRebuild。Bound fast
  Parameter/Automation event は ControlOnly（下流 checkpoint
  validity を再評価）。構造/Port/Plugin capability/Render
  substitute 変化は Rebuild。render stale 判定とは独立。
- Rule: Stale publish 拒否 — revision/capability/clock
  epoch/runtime instance のいずれかが prepare 時 pin と
  乖離した plan は publish しない。generation は単調増加で
  古い prepare の完了が新しい plan を押しのけない
  （Superseded）。
- Rule: Last-good 安全性 — reconcile/publish 失敗で project
  を auto rollback せず、last-good が生存すれば保持して
  current revision mismatch を表示し、旧 generation への
  新規 param/automation 配送を遮断する。QueueFull /
  Retire 失敗 / Plugin restore failure / Clock epoch 変更は
  typed fault として last-good を残す。

## Examples

- Example RCP-EX-01a: Given instrument node が Missing で
  checkpoint→Live EQ の chain、When MainPlayback で select、
  Then instrument 置換に EligibleExact artifact が必要で、
  Live EQ は live region に残り Plan を返す。artifact が
  EligiblePreview のみなら Blocked（偽 Exact なし）。
- Example RCP-EX-01b: Given 同じ chain で EditingPreview
  （非等価 opt-in）、When select、Then Preview 判定で
  非等価を明示した ExecutionSelection を返す。
- Example RCP-EX-02a: Given multi-out plugin の 2 出力が別
  destination へ fan-out、When 片方のみを含む cutset を
  検証、Then 未カバーの必須 output port で Rejected。
- Example RCP-EX-02b: Given Send/Return + parallel mix +
  sidechain key + fan-in PDC の graph、When missing node を
  含む region を substitute、Then 境界の全 port・multiplicity・
  channel map・PDC・sidechain pin が一致しない候補は
  Rejected、二重加算・dangling・誤 Control 配送 0。
- Example RCP-EX-03: Given Unsupported/Corrupt/Stale/
  ACL Denied artifact と Unknown plugin state、When
  MainPlayback で select、Then いずれも Exact を名乗らず
  Blocked または typed unresolved になる。
- Example RCP-EX-04: Given rename・bound fast parameter・
  structural edit・render artifact 到着・plugin capability
  変更、When classify、Then NoRebuild/ControlOnly/Rebuild が
  evidence（fingerprint/capability/epoch）優先で決まり、
  render stale 判定と独立する。
- Example RCP-EX-05a: Given R1→R2→R3 の連続編集で R2 の
  prepare が R3 commit 後に完了、When publish、Then
  StaleProjectSnapshot/Superseded で拒否し古い plan は
  Activation しない。
- Example RCP-EX-05b: Given active plan があり clock epoch
  が変化、When publish/reconcile、Then DeviceEpochChanged
  fault・last-good 保持・旧 generation 配送遮断。
- Example RCP-EX-05c: Given publish が QueueFull または
  RetireOverflow、When outcome 記録、Then Failed receipt・
  last-good 保持・current mismatch 表示。

## Guarantees

- 各 Intent について Plan/Blocked/Preview の判定が typed で
  一意に決まり、非等価音が Exact と混同されない。
- Cutset validator は boundary port・domain・format・
  channel map・latency/PDC・sidechain pin・multiplicity・
  dangling・double-add・control misdelivery の各違反を
  個別の typed rejection として報告する。
- Fault matrix（Unsupported/Corrupt/Stale/ACL Denied
  artifact・Unknown plugin state・QueueFull・Retire 失敗・
  Clock epoch 変更・Plugin restore failure・R1→R3 連続
  編集）で「古い plan の誤 Activation 0」「偽 Exact 0」
  「last-good 保持 + current mismatch 表示」を oracle
  fixture が検証する。
- 判定は全て非 RT の純粋関数/状態機械であり、RT thread の
  FS/alloc/block/graph search を要求しない。

## Non-goals

- `crates/synvelis-workspace` の semantic→selection
  integration（別 Worker lease・deferred residual）。
- 実 Plugin/実 OS/実 RT/PCM・Seek/Loop/Tail/PDC の実測
  （RCP-EX-06 の実機 Gate 分・#296 RCP-GATE）。
- RenderRecipe/RecipeKey/ArtifactManifest/DependencyIndex/
  Eligibility evaluator の正本実装（#290 RCP-01・#291
  RCP-02・#292 RCP-03 の領域。本 slice は eligibility
  state を入力として消費する typed contract のみ）。
- Ruviform Graph Compiler/Plugin Host/DSP の複製・変更、
  Creative Branch Project writer、AudioLens 解析。
- GUI/CLI/Script への露出（#295 RCP-06）。

## Risk

- cutset 規則の実装が spec の境界条件（multi-out・
  sidechain pin・PDC fan-in）を取りこぼすと、偽 Exact や
  音声破壊（double-add/dangling）の温床になる。
- plan reconciliation が既存 `RoutingEngine` の
  Last-good/CAS 規則と乖離すると、二重の正本が生まれ
  stale publish の隙間ができる。
- Preview の非等価表示を省略すると、ユーザーが preview
  音を Exact と誤認する（RCP-EX-03 の核心）。
