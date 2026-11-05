---
id: "SPEC-034AUT03AW1Qq4l4VCpTSK"
type: "spec"
status: "approved"
title: "AUT-03 Loop 録音 1 Pass・Write Range 半開区間閉鎖・明示 Rearm"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/133"
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
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/aut-01-automation-mode-boundary-touch-return-writer-state.md"
    - "docs/specs/aut-02-region-replace-boundary-curve-preservation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/aut-03-loop-write-one-pass-design.md"
    - "docs/adr/aut-03-write-range-boundary-flush-pass-fixation.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-03-e2e-loop-write-one-pass.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  defers: []
  deferred-by: []
---

# AUT-03 Loop 録音 1 Pass・Write Range 半開区間閉鎖・明示 Rearm

## What Should Be Built

Issue #133「2026-11-04 合意・初期製品」を実装する。Automation
Write は**一度の Loop Pass で終了**し、Transport 自体は続行可能。
Multi-pass Take は AUT-F01(#6)に分離済みで本件の Blocking にし
ない。本 PR の範囲は **headless session 状態機械の Pass 境界・
Write Range 閉鎖・Rearm** — PC-C1(#98)の Recording Transaction
と AUT-02(#132)の Region Replace 機構を前提に、Loop Pass の
意味論を session 層で完成させる。

### 契約(Issue #133 が正本)

- Touch/Latch/Write とも明示 `WriteArm && AutomationRecord &&
  TransportActive` が開始条件(既存 `record_start` ゲート)。
  Record 開始時に `ParameterTargetId + ProjectRevision +
  TimeScope + LoopPassId + ClockEpoch` を固定し、Mode/Follow/
  Inspector/Window が変わっても別 Target へ書込まない。
- Loop Wrap 時、最初の Pass の**記録区間(Write Range)を正確な
  半開 Project Beat 区間で閉じ**、Pending/Prepared Commit へ
  遷移させる。以後の Loop では自動書込みせず、新 Pass には
  ユーザーの Explicit Rearm が必要。Playback/Monitor は継続
  してよい。
- `RecordingTransaction::prepare -> ProjectController::
  apply(expected_revision) -> success_finalize`(PC-C1)。
  Commit 失敗で点列を `take` しない。Conflict は
  Pending/RequiresUserRebase で、勝手な最新 Rev 採用禁止。
- 1 Session/Pass は 1 Project Undo。Recording の Stored Value は
  Modulation 前 Manual/Reference(Host-resolved 値を Bake しない)。
  Region 衝突は AUT-02 の Explicit Replace。
- Stop/Seek/Device Epoch 変更/Graph Stale/WriteArm 解除で
  Write Range を閉じ、Plugin Dispatch/Observed/Project Saved は
  別 Receipt にする。

### 本件で具体化する部分(Issue の session 層解釈)

- **`LoopPassId`**: transport run 内の Loop Pass 識別子。
  非 `Playing` → `Playing` の新規 run で 1 に初期化し、wrap ごとに
  +1(`resume` は同一 run で reset しない)。`record_start` が
  現在の pass を transaction へ固定する。
- **Write Range(記録区間)**: lane ごとの半開 coverage
  `[最初の書込 pos, coverage_end)`。境界イベントで close する際、
  **その瞬間に write gate が開いていて writer(active gesture /
  LatchHeld / WriteHeld)が存在した lane のみ** coverage_end を
  境界位置へ延長する — held write は境界まで連続して書いていた
  ため、`last point + ε` では区間末端を正確に表せない。
  - LoopWrap: coverage_end = loop `end`(`end` は次 Pass 所属で
    半開区間から除外)。
  - Stop/Seek/Epoch/GraphStale/Arm/Mode/LoopChanged/
    ProjectChange: coverage_end = 現在 position(その position
    自体の書込みを含む)。
- **境界 flush**: Write Range close で gesture 中の
  write-gated target の**配送済み point は transaction へ記録**
  する — これらは pass の記録データであり、preview 破棄の対象
  ではない。clip 規則は `pos <= bound && pos < pass_end`
  (LoopWrap は `pos < end` と等価)。write gate が閉じていた
  gesture(Read/Unarmed/disabled record)は従来どおり破棄。
- **Pass 境界の誤所属禁止**: loop 設定中、書込位置 `pos >=
  loop_end` は当該 Pass に帰属し得ない — `record_write` は
  Validation で拒否、gesture batch は境界で clip。同時刻
  event は既存の last-wins dedup で 1 点に正規化される。
- **追加の閉鎖イベント**(`SealReason`):
  - `EpochChange`: `install_clock_anchor` で ClockEpoch が open
    transaction の固定 epoch と変わる場合。
  - `GraphStale`: host が LastGood Graph / ControlPlan mismatch
    を検知して明示通知(`note_graph_stale`)した場合。
  - `LoopChanged`: 録音中に loop 範囲が変わる場合(Pass 境界の
    座標基準が失効するため)。
  - `NewTransportPlay`: `Paused`/`Stopped` からの新規 play edge
    (AUT-01 の「Latch/Write は次の再生で暗黙再開しない」契約を
    transaction gate へ適用)。
- **Explicit Rearm = `record_start`**: 既存契約どおり
  `record_start` は常に明示呼出で、新 Pass の記録はこれのみが
  入口。sealed/open transaction が残る間は "recording already
  open" で拒否され、commit/abort で処分してから次 Pass に進む
  (pass 固定値は新 run の pass ではなく呼出時点の pass)。
- **first-seal-wins**: Write Range は一度だけ閉じる — 2 回目の
  境界イベントは coverage/flush を再実行しない(既存の
  `sealed.is_none()` ガードを全経路で共有)。

## Why

- #6 の AUT-03 行で合意された初期製品 scope。PC-C1 は失敗時の
  点列保全を、AUT-02 は Replace の区間外曲線保存を担保したが、
  **Loop Pass の境界そのもの**(いつ記録を閉じるか、何を Pass へ
  帰属させるか、次 Pass の開始要件)は session 層で未規定だった。
- 現行実装は wrap で gate を seal するが、Replace 書き戻し範囲は
  `first..last point + ε` 由来のため、held write が `end` まで
  書き続けた区間の末端を正確に表せず、`[last+ε, end)` に旧曲線
  が復帰してしまう。Write Range を契約概念として持つことが本件の
  核心。
- in-flight gesture を境界で破棄する AUT-01 既定は「preview」
  として正しいが、write-gated gesture の配送済み point は
  recording の一部 — Pass 境界誤所属 0 を満たすには配送済み分を
  transaction へ帰属させる必要がある。

## Acceptance Criteria

(Issue #133 §Acceptance が正本。headless で pin する範囲を明記)

- Loop 途中開始 / loop 終端 / Tempo 変化 / Block 内 wrap /
  同一 Frame Event で **Pass 境界誤所属 0**、2 周目の記録が自動
  発生しない(全て `cargo test` の決定的 headless 検証)。
- Stale Project CAS / Plugin Gone / 2 Window / Stop 直前 Failure
  で Recorded Points 保全、1 Undo/Redo、Save/Reload(永続化は
  project のみ — session transaction の in-memory 契約は PC-C1
  のまま変えない)。
- LastGood Graph/ControlPlan mismatch で別 Parameter への誤配送
  0 — `note_graph_stale` 閉鎖 + fixed lane 写像で headless pin。
  GUI & PC-C6 同一 Scenario の別 Oracle 判定は PC-C6(#103)/
  SV-GUI-05(#114) 側の責務で、headless 相当を本件で pin し
  実 GUI/実 Plugin は NotRun/Unsupported として記録する。
- Commit 成功後の 2 周目再録音には明示 `record_start` が必要で、
  pass は呼出時点の `LoopPassId` を固定する。sealed transaction
  を残したままの `record_start` は拒否される。
- AUT-01(writer closure/boundary)・AUT-02(境界曲線保存・
  Replace)・PC-C1(prepare/apply/finalize・点列保持・exactly
  once)の全既存テストが無退行。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: `ControlSession` の Pass 計数・`LoopPassId`/
  `TimeScope` fixation・Write Range coverage 追跡と閉鎖・境界
  flush・追加 seal 契約・`note_graph_stale`・prepare の
  coverage 反映・呼出 signature 更新(`Project` 依存の導入)・
  headless acceptance test。
- **Out of scope**: Multi-pass Take / Punch 履歴(AUT-F01 #6)、
  実 Plugin binding(PC-C4 #101 / TIM-03 #127)、GUI 接続
  (PC-C5 #102 / SV-GUI-05 #114)、PC-C6 oracle(#103)、PPQN
  変更コマンド(TIM-01 #125)、RT 経路の型・契約変更、
  transport overshoot の位置再計算(wrap は `pos = start` の
  現行意味論を維持)。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。時刻演算は
  `checked_add`/`checked_sub`/`convert_exact` 等の checked API
  のみ(ruviform `Tick`/`TickDuration` unchecked `Add`/`Sub`
  禁止)。
- Automation 点列は `Project::control` のみが正本 — session/
  transaction は写込み前の収集列と coverage を持つだけで、別の
  永続系列を複製しない。session transaction は非永続のまま
  (PC-C1 の receipt/registry 契約と同じ保証水準を超えない)。
- `eval_rt`/dispatch/RT publish 経路は変更しない。RT publish
  成功を Project mutation と混同しない(SF-03 以降の責務)。
- 互換層・重複実装・原因不明の警告抑制・`#[allow]` による lint
  隠蔽を行わない。
