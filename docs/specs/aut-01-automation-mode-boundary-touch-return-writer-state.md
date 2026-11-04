---
id: "SPEC-034AUT01T7xRw4mNcQ8pZbV2"
type: "spec"
status: "approved"
title: "AUT-01 Automation Mode 境界 State — Read/Touch/Latch/Write・Touch Return 150ms Smoothstep"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/131"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/aut-01-touch-return-writer-state-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-01-e2e-touch-return-writer-state.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/102"
  defers: []
  deferred-by: []
---

# AUT-01 Automation Mode 境界 State・Touch Return 150ms Smoothstep

## What Should Be Built

Issue #131「2026-11-04 合意・初期製品」を実装する。`Off/Read/Touch/
Latch/Write` の既存 Mode 定義(#6 決定④)を維持し、Gesture 終了・
Transport 状態での復帰/境界遷移を具体化する。本 PR の範囲は Issue
横断依存正規化の **Start** 区分どおり **純粋状態遷移/時計制御 PoC**
(状態機械+時計制御のコア)に限定し、Plugin binding(PC-C4 #101)・
GUI 接続(PC-C5 #102)は Integration 対象としてスコープ外とする。

### Touch Return(暫定 150ms Smoothstep)

Touch mode の gesture release 後、Reference 値は Project Automation
Curve を変更せず **runtime 補間**で automation へ復帰する:

```text
v = (1 - w) * v_release + w * v_automation(t)
w = 3x² - 2x³          (smoothstep)
x = clamp(elapsed / return_duration, 0, 1)
```

- `v_release` は release 時点の Manual(合成前 Reference)値。
- `v_automation(t)` は評価位置 `t` における automation 系列値
  (系列未到達なら base — 既存 reference fallback と同じ)。
- 補間は **Modulation 合成前の Reference 領域**で行う。
  Reference/Modulation/Host-resolved/Plugin-reported を混同しない。
- `return_duration` は可変。暫定既定 150ms(PoC で最終既定値を承認)。
  0ms は explicit user value に対し許可するが、click/zipper 影響を
  `instant` として観測可能にする。
- Return は保存点列・`ProjectRevision`・Undo を一切変更しない
  (session runtime state のみ)。

### 境界 State(明示閉鎖)

Automation Playback・Manual Override・WriteArm・AutomationRecord・
Transport・Gesture は独立 State のまま。Mode 選択だけで記録を開始
しない。以下のイベントで Gesture と Writer State を**明示閉鎖**する:

`Transport Stop` / `Seek` / `Loop Wrap` / `New Transport Play` /
`Plugin Crash` / `GUI Focus loss` / `Param Deleted`(target 消去)。

- Mode 別の release 意味:Read = 書込なし・即座に復帰。Touch =
  Gesture 区間のみ Reference 値を書込み、終了後 automation へ復帰。
  Latch = release 後も最後の Manual 値で書込継続、Stop/ModeChange で
  閉鎖。Write = Arm 対象のみ記録開始点から固定 Reference 基準値を
  継続書込み、非対象は不変。
- Latch/Write は次の再生で暗黙再 Arm/再録音しない(再開には常に
  明示 `record_start`)。
- `GestureFixation`: ParameterTarget/ProjectRevision/Arm/Timebase を
  gesture begin(pointer down 相当)で固定し、2Window/Plugin GUI/
  IME Focus や Inspector Follow 変更で別 target へ誤配送しない。

## Why

- #6 の UI Mode 表は「操作中の優先」と「Arm+Record+Transport 成立時の
  書込み」を分離したが、Gesture 解放後の **復帰過程** と、Transport/
  Focus/Plugin 境界での **状態閉鎖** が未規定だった。Touch Return の
  補間形状と境界イベント集合が本件の新規部分。
- PC-C1(#98)の Recording Transaction 契約(prepare/apply/finalize・
  seal・CAS)はそのまま使い、本件はその上流の session 状態機械を
  完成させる。録音 transaction との整合(固定 target/lane/revision・
  seal・点列保持)は PC-C1 の不変条件として維持する。
- RT 規約: session 状態機械自体は non-RT(synvelis-workspace)。
  補間演算は pure f64 で RT 経路にも利用可能な形にするが、
  `eval_rt`/dispatch 経路の契約は変更しない。

## Acceptance Criteria

(Issue #131 §Acceptance が正本。本 PoC で担保する範囲を明記)

- 0ms/150ms/可変 Return 期間で、release 後の復帰値が
  `x = clamp(elapsed/duration)` の smoothstep に従い連続変化する。
  Step/Linear 曲線の変化中でも Return が series 値 `v_automation(t)`
  へ追従する。Return は保存点列/`ProjectRevision` を変更しない。
- `Read`/`Touch`/`Latch`/`Write` × `Stop`/`Seek`/`Loop`/
  `Focus 失効`/`Param Deleted` の State Matrix で、Gesture と
  Writer State が明示閉鎖される。録音済み点列は 1 gesture/1 session =
  1 undo step(PC-C1)のまま。
- `record_start` 時に Arm 済み Write target の Reference 基準値が
  固定され、Arm/Record/Rolling を満たす対象のみへ書込まれる。
  非対象の parameter は不変。
- `record_start` は常に明示呼出で、Latch/Write の次再生における
  暗黙再 Arm/再録音はない。
- gesture fixation が target/revision/arm/clock epoch/frame base を
  begin 時に固定し、Inspector Follow/focus 変更で書込 target が
  すり替わらない。
- headless PoC として全検証が `cargo test` で決定的に再現する。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: `ControlSession` の Touch Return runtime 補間・session
  clock・Writer State  introspection・境界イベントでの明示閉鎖・
  gesture fixation・Write 基準値の record_start 固定・対応する
  headless acceptance test。
- **Out of scope**: Plugin 実機 binding/実 Process Capability(PC-C4
  #101・TIM-03 #127)・GUI Gesture/Mode/Return 接続(PC-C5 #102)・
  PC-C6 oracle(#103)・SV-GUI-05(#114)。Verified Active Graph/
  Plugin Capability generation 一致・実 Plugin Callback・実 RT 精度は
  Acceptance 層で別判定。Region replace/境界値保存は AUT-02 #132、
  Loop write 1-pass 規則は AUT-03 #133。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。時刻演算は
  `checked_add`/`checked_sub`/`convert_exact` 等の checked API のみ
  (ruviform `Tick`/`TickDuration` の unchecked `Add`/`Sub` 禁止)。
- 互換層・重複実装・隠れ fallback を作らない。session state は
  project を変更しない(唯一の mutation 経路は既存どおり
  `control_gesture_end` の `SetParameterBase` と PC-C1 recording
  commit)。
- `EvalPlan::eval_rt` 成功経路の非割当と snapshot 契約を維持する。
  本件は session(non-RT)側のみの変更で、RT 経路の型・契約を
  変更しない。
