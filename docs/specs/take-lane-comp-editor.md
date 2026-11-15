---
id: "SPEC-xOTmUaWxKVIkrr8hUBfugh"
type: "spec"
status: "approved"
title: "Take Lane / Comp Editor — UI 詳細契約（synvelis#12 §14/§15 後続 slice — docs-only 設計 slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
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
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/timeline-editing.md"
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/specs/daw-workflow.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/take-lane-comp-editor-design.md"
    - "docs/plans/take-lane-comp-editor.md"
    - "docs/tasks/take-lane-comp-editor-task.md"
    - "docs/test-specs/take-lane-comp-editor.md"
    - "docs/impl/ir/take-lane-comp-editor.md"
    - "docs/impl/exp/take-lane-comp-editor.md"
    - "evidence/sv12/ui-contract-ledger.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  derived-by:
    - "docs/designs/take-lane-comp-editor-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/take-lane-comp-editor.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# Take Lane / Comp Editor — UI 詳細契約（synvelis#12 §14/§15 後続 slice — docs-only 設計 slice）

## What Should Be Built

Issue #12 §14（Take Lane / Audition / Comp Tool / Recovery Review UX）と
§15（Audio / MIDI Editor と Take / Comp の統合）で合意済みの UX 方針を、
実装 Owner が消費できる canonical UI 契約として固定する **docs-only slice**。
本 spec は「何を・なぜ」だけを定義し、機構契約（Preview→検証→Atomic
Commit→1 Undo、gesture target fixation、stale revision 拒否、view-state /
project provenance 分離）の正本は upstream canonical docs に残す。本 slice
はそれらを継承する **UI 側の責務分界** を定義する。

固定対象は次の 7 領域（Coordinator 確定の契約範囲。数値プリセット・OS 別
既定キーバインドは固定しない）:

1. **Take Lane Collapsed / Expanded**: view-state と project provenance の分離
2. **Take Click / Time Selection / Audition**: non-mutation + restore semantics
3. **Comp Paint / Adopt gesture**: preview → validate → atomic commit → 1 undo
4. **Region Boundary / Transition Handle**: fixation & stale-revision rejection
5. **HitTest priority ordering**: 契約としての優先順位定義
6. **Tool / Modifier semantics**: semantics matrix（OS 別既定は固定しない）
7. **Fade / audition defaults as contract**: 存在すべき保証の定義（数値は固定しない）

## Why Now

- `evidence/sv12/owner-matrix.md` の AC-28 / AC-29 は唯一の `Deferred` AC、
  AC-30 / 31 / 32 / 36 / 38 / 39 / 41 は `Partial`、AC-47 は
  `Pass (headless)`（機構は headless 実証済み）。いずれも UI 面の実
  GUI/実機受入は #114/EXT-L3 `Deferred` であり、本 slice が固定するのは
  UI 契約のみ。
- 4 つの canonical doc が本 slice へ明示的に defer 済み:
  `docs/designs/recording-take-comping-design.md`（Take Lane/Comp Editor UI
  詳細 → #12 §14/§15 後続）、`docs/specs/daw-workflow.md`、
  `docs/specs/timeline-editing.md`、`docs/specs/sv-gui-03-06-keyboard-gesture.md`。
- 実 GUI 実装 Owner（#112/#114 系 slice）が、合意済み UX をそのまま実装
  できる契約を必要とする。契約が無いまま実装すると Take Click が暗黙
  Adopt 化する、Audition が Project を汚す等の解釈分裂が起きる。

## Users and Value

- **実装 Owner（#112/#114 系 GUI slice、#115〜#118 実装 Owner）**:
  gesture → View/Runtime/Project 状態の責務分界を第二正本なしに消費できる。
- **Reviewer / 受入担当**: `evidence/sv12/ui-contract-ledger.md` 経由で
  Issue #12 §14.7 の受入条件と test-spec case を追跡できる。
- **将来の Editor 統合実装（§15）**: Display Target / Editing Target 分離・
  Effective Comp 非破壊表示の UI 側契約を参照できる。

## Scope

### In Scope

- 上記 7 領域の UI 契約（design contracts のみ。数値プリセット禁止）。
- §14.6 の所有境界を UI 語彙で正本化した表（どの gesture が View State /
  Runtime State / Project State / Undo のどれを触るか）。
- §15.2〜§15.5 の Editor 統合について、UI 側の責務分界
  （Display Mode = view-state、Editing Target = 明示 binding）。
- `evidence/sv12/ui-contract-ledger.md`: owner-matrix の UI 関連 AC
  （AC-28/29/30/31/32/36/38/39/41/47）と本 chain の test-spec case の対応表。
- canonical doc chain（spec → design → plan → task → test-spec → impl/ir
  → impl/exp）と触れた dir の管理 index 再生成。

### Out of Scope

- `crates/**` の Rust コード・テスト・fixture の変更（一切行わない）。
- 実 GUI 実装・実 OS/実 Device/実 Plugin 検証（#112/#114・EXT-L3 の責務。
  `Deferred`/`NotRun` として正直に記録）。
- 数値プリセット: fade 既定長・曲線・60ms/Equal-power 等の例示値、
  hit-test の px 閾値・Snap Hysteresis（#11 §9 PoC 後続決定）。
- OS 別既定キーバインド・修飾キー配置・Touch ジェスチャ（§14.8 未確定）。
- Creative Branch Take/Comp 候補（#278/#279/#288 の別作業）。
- MIDI comp の note-level schema・chase アルゴリズム詳細・serialization
  （§15.6〜§15.9 の未確定事項）。
- `evidence/sv12/owner-matrix.md` の改変（READ-ONLY の既存台帳）。
- ADR の新規作成（Issue §14/§15 合意の正本化であり新規の大きな分岐選択を
  想定しない）。

## Requirements

| ID | 要件 | 対応 AC |
| --- | --- | --- |
| REQ-TLC-01 | Take Lane の Collapsed / Expanded 切替は View Instance 所有の view-state であり、Active Comp・Project Revision・Undo 履歴を変更しない | TL-AC-01 |
| REQ-TLC-02 | Take Click・Time Selection・Audition は Comp 採用・Revision・Undo を変更しない。Audition は `AUDITION ONLY` 表示・対象固定・明示解除経路を持ち、終了・別候補切替・録音開始・View 破棄で元再生状態へ復帰する | TL-AC-02〜04 |
| REQ-TLC-03 | Comp Region を変更するのは明示 Adopt / Comp Paint 操作のみ。Drag Preview 中は Project 不変、Esc/PointerCancel で変更なし、Commit は全対象 Validation して単一 Undo。Smart Tool の Take Click に暗黙 Adopt を割当てない | TL-AC-05 |
| REQ-TLC-04 | Grouped Comp は全 Track の Capture Integrity / Source Handle / Revision を検証し、部分適用なし・複数 Track 同時の単一 Undo | TL-AC-06 |
| REQ-TLC-05 | Region Boundary Handle と CompTransition Handle は別 Target。Gesture 開始時に Target ID / Revision を固定し、stale 変更を retarget せず拒否。Source Handle 不足/欠落/Invalid Segment/古い Revision を拒否・表示する | TL-AC-07 |
| REQ-TLC-06 | HitTest 優先順位・Tool/Modifier semantics・Fade/Audition の契約保証を upstream 契約と整合する形で定義する（数値・OS 別既定は固定しない） | TL-AC-08〜10 |
| REQ-TLC-07 | Window / Dock / Inspector の移動・リンク/フォーカス変更だけで Record Target・Audition・Active Comp・Project Undo が誤変更されない。Editor Display Mode 切替は view-state、Editing Target は明示 binding で、Display/Editing/Command/Undo の責務を分離する | TL-AC-11〜12 |

## Acceptance Criteria

| ID | 受入条件 | 対応 |
| --- | --- | --- |
| TL-AC-01 | Collapsed Active Comp / Expanded Take Lanes を切り替えても Active Comp と Project Undo 履歴が変化しない（owner-matrix AC-28） | REQ-TLC-01 |
| TL-AC-02 | Take Click は View-local Selection のみ。Arm / Record Target / Active Comp を変更しない（AC-29 の一部） | REQ-TLC-02 |
| TL-AC-03 | Time Selection は Preview / Scope の保持のみ。Source / Comp を変更しない（AC-29 の一部） | REQ-TLC-02 |
| TL-AC-04 | Audition は Runtime Playback Override で `AUDITION ONLY` を表示し、対象 Take / Time Range を固定する。Audition 終了・別候補切替・録音開始・View 破棄で解除され元 Active Comp 再生へ復帰する。Project / Source を変更しない（AC-29・AC-39 の UI 面） | REQ-TLC-02 |
| TL-AC-05 | 明示 Adopt / Comp Paint 操作だけが Comp Region を変更する。Drag Preview 中は Project 不変、Esc / PointerCancel で変更なし、Commit は全対象 Validation して単一 Undo。Smart Tool の Take Click に暗黙 Adopt を割当てない（AC-30） | REQ-TLC-03 |
| TL-AC-06 | Grouped Comp の一部 Track に不整合がある場合、成功した対象だけを無言で変更しない。複数 Track 同時の単一 Undo が成立する（AC-32） | REQ-TLC-04 |
| TL-AC-07 | Region Boundary と Transition Handle の操作・Target / Revision が分離され、Source Handle 不足 / 欠落 / Invalid Segment / 古い Revision 時は正しく拒否・表示される（AC-31・AC-47） | REQ-TLC-05 |
| TL-AC-08 | Take Lane 領域・Comp Region・Transition Handle・Region Boundary・通常 Clip 編集対象の hit test 優先順位が契約として定義され、upstream（timeline-editing / sv-gui-03-06）と矛盾しない。px 閾値等の数値は固定しない | REQ-TLC-06 |
| TL-AC-09 | Dedicated Comp Tool・Smart Tool・modifier の semantics matrix が定義される。OS 別既定キーバインド・修飾配置は固定しない | REQ-TLC-06 |
| TL-AC-10 | Fade / Crossfade / Audition の「契約として存在すべき保証」（既定の存在・非破壊・Source 不変・復帰経路）が定義される。数値既定値は固定しない | REQ-TLC-06 |
| TL-AC-11 | Window / Dock / Inspector の移動、リンク/フォーカス変更だけで Record Target、Audition、Active Comp、Project Undo が誤変更されない（AC-36） | REQ-TLC-07 |
| TL-AC-12 | Editor の Display Mode（Effective Comp / Take Source / Compare）切替は view-state で Revision / Undo を変更せず、Editing Target は明示 binding を要する。source-local edit・comp edit・arrangement move/trim の command / target / undo が分離される（AC-38・AC-39・AC-41 の UI 面） | REQ-TLC-07 |

## Deferred Design Concerns

- fade 既定長・曲線・crossfade 形状の数値、hit-test px 閾値・Snap
  Hysteresis → #11 §9 PoC 後続決定。
- OS 別 Keyboard / Modifier / Touch の既定配置 → §14.8 未確定、実装
  slice で PoC/ユーザー設定へ。
- 実 GUI / 実 OS / 実 Device / 実 Plugin での受入 → #114・EXT-L3 の別
  受入 Gate。本 slice の契約 Pass を実機 Pass に換算しない。
- MIDI comp の note-level 編集・chase 詳細・serialization → §15.6〜§15.9
  未確定事項。
- Creative Branch Take/Comp 候補・Fork/Adopt/Audio Diff → #278/#279/#288。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/12（§13 受入基盤、
  §14 Take Lane/Audition/Comp Tool/Recovery Review UX、§15 Editor 統合、
  §14.7 実装優先度と追加受入条件、§14.8/§15.9 未確定事項）。
- upstream 契約（depends-on で参照・第二正本を作らない）:
  - `docs/specs/recording-take-comping.md` +
    `docs/designs/recording-take-comping-design.md`（C6 Comp・Take Group・
    Editor、C7 Transaction・Session 運用 — 概念契約の正本）
  - `docs/specs/timeline-editing.md` + `docs/designs/timeline-editing-design.md`
    （gesture/selection/snap/undo の共通契約）
  - `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
    + `docs/designs/ed-a-editing-model-design.md`（gesture fixation・
    Preview 不変性・atomic commit）
  - `docs/specs/ed-b-media-domain-move-slip-commit-semantics.md` +
    `docs/designs/ed-b-media-domain-editing-design.md`（media-domain 編集意味）
  - `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
    + `docs/designs/ws-a-workspace-foundation-design.md`（view-state /
    selection / command 分離）
  - `docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md`
    + `docs/designs/mda-comp-effective-projection-placement-design.md`
    （TakeGroup/Comp/Effective Comp/placement 契約）
  - `docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md`
    （comp transition / MIDI phrase / stable-ID fixture 契約）
  - `docs/specs/sv-gui-03-06-keyboard-gesture.md` +
    `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`
    （keyboard/gesture dispatch・tool.activate 語彙）
  - `docs/specs/daw-workflow.md` + `docs/designs/daw-workflow-design.md`
    （DAW workflow・Undo lane）
- 台帳: `evidence/sv12/owner-matrix.md`（69 AC の Owner Matrix、READ-ONLY）、
  `evidence/sv12/ui-contract-ledger.md`（本 slice が新規作成する UI 契約
  対応表）。
