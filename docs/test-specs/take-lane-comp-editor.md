---
id: "TSPEC-UB4fnjlz7vYEkTNeKiDU3G"
type: "test-spec"
status: "approved"
title: "Take Lane / Comp Editor — UI 契約・受入検証仕様（synvelis#12 §14/§15 後続 slice — docs-only）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/take-lane-comp-editor.md"
    - "docs/designs/take-lane-comp-editor-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/take-lane-comp-editor.md"
    - "docs/tasks/take-lane-comp-editor-task.md"
    - "evidence/sv12/ui-contract-ledger.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/take-lane-comp-editor-design.md"
  derived-by: []
  verifies:
    - "docs/specs/take-lane-comp-editor.md"
    - "docs/designs/take-lane-comp-editor-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  defers: []
  deferred-by: []
---

# Take Lane / Comp Editor — UI 契約・受入検証仕様（synvelis#12 §14/§15 後続 slice — docs-only）

## Purpose

Issue #12 §14/§15 の UI 契約（spec TL-AC-01〜12 / design 契約 C1〜C7）
が将来の実装で保持されるべき振る舞いを固定する。本 test-spec は
「どの UI 操作が View/Runtime/Project/Undo のどの状態を触ってよいか」
という保証を記録し、実装 slice が契約から逸脱した場合に検出できる
ようにする。実機・実 GUI での受入は #114/EXT-L3 の別 Gate であり、
本書の case は契約面の検証意図を定義する（実行結果は impl/exp に
記録しない — docs-only slice）。

## 状態語彙

case の status は `Contract fixed (design)`（本 chain が契約として
固定）と `Deferred`（実機/実装 slice へ委譲）を区別する。契約固定を
実機 Pass に換算しない（POLICIES 準拠）。

## Feature

### F-LANE — Take Lane Collapsed/Expanded（TL-AC-01, REQ-TLC-01, design C1）

Rule: lane 展開/折畳みは View Instance 所有の view-state であり、
Active Comp・Project Revision・Undo 履歴・他 View 状態を変更しない。

- Example TS-LANE-01: Expanded → Collapsed → Expanded を往復しても
  Active Comp・Revision・Undo が不変（design Scenario S1）。
- Example TS-LANE-02: lane 上の provenance 表示（TakeId・integrity・
  commit 状態）は read-only 投影であり、表示自体が Project を変更
  しない（design D-TL-5）。

### F-SELECT — Take Click / Time Selection（TL-AC-02/03, REQ-TLC-02, design C2.1）

Rule: Take Click は View-local Selection のみ。Time Selection は
Preview/Scope 保持のみ。いずれも Arm・Record Target・Active Comp・
Revision・Undo を変更しない。

- Example TS-SELECT-01: Take lane で Take Click → selection が更新
  されるが Comp 採用・Revision・Undo は不変。
- Example TS-SELECT-02: Time Selection の範囲設定は scope のみを
  保持し、Source/Comp を変更しない。

### F-AUDITION — Audition override と復帰（TL-AC-04, REQ-TLC-02, design C2.2）

Rule: Audition は固定対象（TakeId+Time Range）の Runtime Playback
Override。`AUDITION ONLY` 表示必須。解除経路（明示終了・別候補切替・
録音開始・View 破棄）で元再生状態へ復帰。Project/Source/Undo 不変。

- Example TS-AUDITION-01: Audition 開始で `AUDITION ONLY` が表示され
  Active Comp が一時 override されるが Project Revision は不変。
- Example TS-AUDITION-02: 録音開始で Audition が解除され元 Active
  Comp 再生へ復帰する（design Scenario S2）。

### F-GESTURE — Adopt / Comp Paint（TL-AC-05/06, REQ-TLC-03/04, design C3）

Rule: Comp Region を変更するのは明示 Adopt/Comp Paint の commit のみ。
preview 中 Project 不変、Esc/PointerCancel で変更なし、commit は全対象
validation + 単一 Undo。Smart Tool の Take Click に暗黙 Adopt なし。
Grouped Comp は all-or-nothing・単一 Undo。

- Example TS-GESTURE-01: Dedicated Comp Tool drag → preview 中 Project
  不変 → pointer up で全対象 validation 成功時のみ Comp Region 更新 +
  1 undo（S3/S4）。
- Example TS-GESTURE-02: 同 drag を Esc で cancel → Project・Undo
  不変（S3）。
- Example TS-GESTURE-03: Smart Tool で Take Click → selection のみ、
  preview/adopt なし（S5）。
- Example TS-GESTURE-04: Grouped Comp で一部 Track が Source Handle
  不足 → 全 Track 不変・失敗表示・部分適用なし・Undo なし（S7）。

### F-HANDLE — Boundary / Transition Handle（TL-AC-07, REQ-TLC-05, design C4）

Rule: Region Boundary Handle と CompTransition Handle は別 Target。
Gesture 開始時に Target ID/Revision を固定し stale は retarget せず
reject。Source Handle 不足/欠落/Invalid Segment/古い Revision を
拒否・表示。Hard Cut/Transition 無効化は明示操作のみ。

- Example TS-HANDLE-01: gesture 開始後に別 View が Project を変更 →
  commit 時に固定 Revision と不一致で reject・stale 表示（S6）。
- Example TS-HANDLE-02: Source Handle 不足の transition commit →
  reject + 理由表示。無効データの読み出し・暗黙の丸めなし。
- Example TS-HANDLE-03: Boundary Handle と Transition Handle が
  空間的に重複 → 推測で選ばず候補提示または明示 target 解決（S10）。

### F-HITTEST — HitTest priority ordering（TL-AC-08, REQ-TLC-06, design C5）

Rule: P0 gesture capture → P1 明示 Handle → P2 Take lane body →
P3 Comp Region/Active Comp clip body → P4 通常 clip 編集対象 →
P5 lane empty → P6 view background。px 閾値は固定しない。

- Example TS-HITTEST-01: 進行中 gesture は hit test を再評価せず
  固定 target を保持する。
- Example TS-HITTEST-02: Expanded で pointer が指す lane 行が優先され、
  行内で handle が body より先に解決される。

### F-TOOL — Tool / Modifier semantics（TL-AC-09, REQ-TLC-06, design C6）

Rule: Smart Tool は hit-test 解決の既定 tool で暗黙 adopt なし。
Dedicated Comp Tool が comp gesture を担う。modifier は役割名
（MOD-AUDITION/MOD-ADD/MOD-CONSTRAIN）で定義し OS 別既定を固定しない。
input context suppression は sv-gui-03-06 正本を継承。

- Example TS-TOOL-01: `tool.activate` で Dedicated Comp Tool へ切替
  → comp gesture が有効化され、Smart Tool では selection のみ。
- Example TS-TOOL-02: TextInput/IME/Modal context では comp gesture
  が発火しない（suppression 規則の継承）。

### F-EDITOR — Display/Editing Target 分離と非干渉（TL-AC-11/12, REQ-TLC-07, design §10/§11）

Rule: Window/Dock/Inspector の移動・focus・link 変更だけで Record
Target・Audition・Active Comp・Undo が誤変更されない。Editor Display
Mode（Effective Comp/Take Source/Compare）切替は view-state。
Editing Target は明示 binding。source-local/comp/arrangement の
command/target/undo は分離。

- Example TS-EDITOR-01: Window/Dock 移動・focus 変更で Record Target・
  Audition・Active Comp・Undo が不変（S8・AC-36）。
- Example TS-EDITOR-02: Display Mode を Effective Comp→Take Source→
  Compare と切替 → view-state のみ、Revision/Undo 不変。Editing
  Target binding は別操作（S9・AC-38/39/41）。
- Example TS-EDITOR-03: source-local edit・comp region/transition edit・
  arrangement clip move/trim の 3 種を個別に commit → それぞれ別の
  command/target/undo 単位として記録され、相互に暗黙 retarget しない
  （AC-41 の区別挙動）。
- Example TS-EDITOR-04: MIDI Editor で Comp Clip を開き複数 Take の
  Note/CC を表示 → Source の bytes/event が不変（AC-38 の区別挙動）。

### F-FADE — Fade/Crossfade/Audition の契約保証（TL-AC-10, REQ-TLC-06, design C7）

Rule: transition の明示表現・fade 非破壊性・既定値の存在・curve 語彙
限定・audition 復帰経路の 5 保証が存在する。数値既定値は固定しない。

- Example TS-FADE-01: 新規 transition が project 既定 fade で初期化
  され、raw Source frame を変更しない（G2/G3）。
- Example TS-FADE-02: audition が必ず C2.2 の解除経路を持ち、解除後
  開始前の再生状態へ決定論的に復帰する（G5）。

## Guarantees

- 実装が上記 Rule に違反した場合（暗黙 adopt・preview 中の Project
  変更・stale retarget・部分適用・audition 復帰不能・view-state の
  Project 昇格）、対応する Example が失敗として検出できる。
- 契約と upstream 正本（timeline-editing/ed-a/mda/sv-gui-03-06）の
  矛盾は test-spec ではなく design review で検出する。本書は
  upstream を再定義しない。

## Non-goals

- 実機・実 GUI/実 OS/実 Device/実 Plugin での受入（#114/EXT-L3）。
- 数値プリセット（fade 既定長・curve 既定・px 閾値・Snap
  Hysteresis）の検証 — #11 §9 PoC 後続。
- OS 別既定キーバインド・Touch の検証 — §14.8 未確定。
- MIDI note-level comp・chase・serialization の検証 — §15.6〜§15.9。
- cargo/fixture の実行 — docs-only slice のため `NotRun`/
  `NotApplicable`。

## Risk

- 契約が実装で形骸化するリスク → ledger で AC↔case を追跡し、実装
  slice の検証時に本書の Rule を oracle として使う。
- 「契約 fixed」を実機 Pass と誤読するリスク → 状態語彙で
  `Contract fixed (design)`/`Deferred` を分離し、ledger・PR に明記。
