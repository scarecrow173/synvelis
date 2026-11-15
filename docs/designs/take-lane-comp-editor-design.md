---
id: "DESIGN-clIJhvdpK3FeleehsQ9lfb"
type: "design"
status: "approved"
title: "Take Lane / Comp Editor — UI 詳細設計（synvelis#12 §14/§15 後続 slice — docs-only 設計 slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  implements:
    - "docs/specs/take-lane-comp-editor.md"
  implemented-by: []
  depends-on:
    - "docs/specs/take-lane-comp-editor.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/timeline-editing-design.md"
    - "docs/designs/ed-a-editing-model-design.md"
    - "docs/designs/ed-b-media-domain-editing-design.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
    - "docs/designs/mda-comp-effective-projection-placement-design.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/designs/daw-workflow-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/take-lane-comp-editor.md"
    - "docs/tasks/take-lane-comp-editor-task.md"
    - "docs/test-specs/take-lane-comp-editor.md"
    - "evidence/sv12/ui-contract-ledger.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/take-lane-comp-editor.md"
  derived-by: []
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

# Take Lane / Comp Editor — UI 詳細設計（synvelis#12 §14/§15 後続 slice — docs-only 設計 slice）

## 1. 概要

本設計は Issue #12 §14/§15 で合意済みの Take Lane / Comp Editor /
Audio・MIDI Editor 統合 UX を、実装 Owner が消費できる UI 契約へ固定する。
対象は **文書契約のみ**であり、`crates/**`・実 GUI・数値プリセット・
OS 別既定キーバインドは対象外。

設計の中心原則（Issue §14.6 / upstream 契約の継承）:

- **View State / Runtime State / Project State / Undo の所有境界を
  UI 語彙で分離する**。Take Lane の表示・選択・Audition・Preview は
  Project musical state を変更しない。
- **Comp を変更する経路は明示 gesture のみ**。preview → 全対象
  validation → atomic commit → 1 undo unit。暗黙 Adopt を一切
  割当てない。
- **機構は upstream 正本を継承する**。独自の undo モデル・preview/
  audition 経路・hit-test 実装を新設しない。本設計は「どの UI 操作が
  どの状態を触るか」の責務分界だけを固定する。

## 2. 上流契約（正本の所在）

| 領域 | 正本 | 本設計の関係 |
| --- | --- | --- |
| Recording/Take/Comping 基本契約・C6（Comp・Take Group・Editor）・C7（Transaction/Session） | `docs/designs/recording-take-comping-design.md`（spec: `docs/specs/recording-take-comping.md`） | 概念契約の上位正本。display mode 分離・gesture fixation・audition 非破壊・view/focus 非干渉はここが正本。本設計は UI 操作への写像のみ行い、重複規定しない |
| Timeline Editing / Selection / Tool / Snap / Gesture / Undo | `docs/designs/timeline-editing-design.md`（spec: `docs/specs/timeline-editing.md`） | gesture target fixation・Preview 不変性・atomic commit・1 undo・hit-test の概念正本。Take Lane/Comp Editor はこの契約を継承する |
| Editing Model（選択・Tool・Snap・Transaction・LinkedContext） | `docs/designs/ed-a-editing-model-design.md` | gesture 開始時の Target ID / Revision 固定・stale 拒否・Ambiguous の候補提示先例 |
| Media Domain 編集意味（move/slip/commit） | `docs/designs/ed-b-media-domain-editing-design.md` | raw source 不変・non-destructive 編集の正本 |
| Workspace Foundation（view-state/selection/linkgroup/command/dock） | `docs/designs/ws-a-workspace-foundation-design.md` | View Instance 所有 view-state・per-view 独立性の正本 |
| TakeGroup / Comp / Effective Comp / Placement | `docs/designs/mda-comp-effective-projection-placement-design.md` | Comp Region/Transition schema・Effective Comp 投影・Source Handle 検証・half-open range の正本 |
| Comp Transition / MIDI Phrase fixture / stable ID | `docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md` | transition 境界・fail-closed 検証・raw media 不変の fixture 契約 |
| Keyboard / Gesture dispatch・tool 語彙 | `docs/designs/sv-gui-03-06-keyboard-gesture-design.md` | `tool.activate` の tool 名語彙・View-context-only 発火・Mod 正規化の正本 |
| DAW workflow・Undo lane | `docs/designs/daw-workflow-design.md` | Undo lane の配送・Tool 切替の Undo 対象外等 |

## 3. 契約 C1: Take Lane Collapsed / Expanded（view-state vs project provenance）

### 3.1 表示モード

- **Collapsed（既定）**: Issue §14 の合意どおり、Arrangement は
  通常 Active Comp を 1 つの編集可能な Clip として表示し、Take 候補
  lane は表示しない（「lanes expand/collapse explicitly」の既定側）。
- **Expanded**: Comp Lane（採用済み Comp Region 列）+ Take 候補
  Lane（TakeGroup の各 Take）+ Transition 表示を展開する。

### 3.2 所有境界

- Collapsed / Expanded の切替状態は **View Instance 所有の
  view-state**（ws-a 正本の per-view 独立状態）である。
- 切替は以下を **一切変更しない**: Active Comp、Project Revision、
  Undo 履歴、Recording Session 状態、他 View の状態。
- Collapsed での Comp Clip は Arrangement 上の通常 Clip として振る舞い、
  通常の clip 編集対象（move/trim 等）を持つ。Expanded での Comp
  Region / Take Lane は C5 の hit-test 契約に従う別対象である。
- Take 候補 lane の各行は provenance 情報（TakeId・Pass/Session 由来・
  integrity/commit 状態の投影）を表示してよいが、表示は read-only で
  あり Project を変更しない（Recovery Review の状態語彙は
  recording-take-comping C6/C7 正本を継承）。

### 3.3 受入との対応

TL-AC-01 / owner-matrix AC-28。

## 4. 契約 C2: Take Click / Time Selection / Audition（non-mutation + restore）

### 4.1 操作の分離

Take の「選択」「試聴」「採用」は **3 つの独立した操作**であり、
互いを暗黙に誘発しない（Issue §14.3）。

| 操作 | 状態への影響 | Undo |
| --- | --- | --- |
| Take Click | View-local Selection のみ（Inspector Follow の起点）。Arm / Record Target / Active Comp / Project Revision を変更しない | なし |
| Time Selection | Preview / Scope の保持のみ。Source / Comp を変更しない | なし |
| Audition | Runtime Playback Override。Project / Source を変更しない | なし |
| Adopt / Comp Paint Commit | Project（Comp Region/Transition）+ publish | 1 undo unit（C3） |

### 4.2 Audition 契約

- Audition は **Runtime Playback Override** であり、固定された対象
  （TakeId + Time Range）を一時再生する。対象は gesture/command 開始時に
  固定し、途中で暗黙 retarget しない。
- Audition 中は **`AUDITION ONLY` 相当の明示表示**を必須とする
  （Active Comp が再生中であるかのような誤表示を禁止）。
- 解除経路（いずれかで確実に解除し元の Active Comp 再生状態へ復帰）:
  - 明示的な Audition 終了操作
  - 別候補への切替（新しい Audition 対象への置換）
  - 録音開始
  - View 破棄
- 復帰先は「Audition 開始前の再生状態」であり、Audition 自体は
  Project Revision・Undo 履歴・Source を変更しない。
- Audition の音声経路（monitor/HPF/latency 等）は RCD-F02・
  recording-take-comping の Runtime 契約の責務であり、本契約は
  「Project を変更しない一時再生 override」という UI 側の性質のみを
  固定する。

### 4.3 受入との対応

TL-AC-02〜04 / owner-matrix AC-29・AC-39 の UI 面。

## 5. 契約 C3: Comp Paint / Adopt gesture（preview → validate → atomic commit → 1 undo）

### 5.1 gesture ライフサイクル

Dedicated Comp Tool（名称は実装 slice が `tool.activate` 語彙へ登録。
sv-gui-03-06 の `Tool` enum 命名規則に従う）による drag は
**Preview → Commit** の 2 相である:

1. **Gesture 開始**: Target ID（CompId / TakeId / region 範囲）と
   Project Revision を固定する（ed-a fixation 正本の継承）。
2. **Preview（drag 中）**: view 側の一時状態のみ。Project・Source・
   Undo 履歴は不変。RT 安全な publish は行わない（non-RT 経路で
   準備し、commit 時にのみ publish — §15.8 の UI/comp 変更 publish
   契約に従う）。
3. **Commit（pointer up 等の確定操作）**: 全対象を validation
   （Capture Integrity・Source Handle・Revision・range 妥当性）し、
   成功時のみ単一の atomic commit = **1 Project Undo unit**。
   失敗時は Project 不変で reject + 理由表示。
4. **Cancel**: Esc / PointerCancel / window 破棄は Project を
   一切変更しない。

### 5.2 暗黙 Adopt の禁止

- Smart Tool での Take Click は **selection のみ**であり、Comp 採用を
  暗黙に行わない。
- Take lane 上の drag も、Dedicated Comp Tool 以外では Preview を
  開始しない（Time Selection の scope 設定等、view-state のみ）。
- 「録音された新規 Take を既存 Comp へ自動反映」する経路は存在
  しない（Issue §14.4: 新規 Take は candidate 登録のみ）。

### 5.3 Grouped Comp（複数 Track 一括）

- Grouped Comp gesture の commit は **全 Track を一括 validation** し、
  all-or-nothing とする。一部 Track のみ成功した部分適用を無言で
  行わない。
- 複数 Track の変更は **単一 Undo unit** にまとめる
  （track 毎の個別 undo を生成しない）。
- validation 失敗時はどの Track も変更せず、失敗対象を表示する。

### 5.4 受入との対応

TL-AC-05〜06 / owner-matrix AC-30・AC-32。

## 6. 契約 C4: Region Boundary / Transition Handle（fixation & stale-revision rejection）

### 6.1 Handle の分離

- **Region Boundary Handle**: Comp Region の採用境界（どの Take の
  どの区間を採用するか）を編集する対象。
- **CompTransition Handle**: 隣接 Region 間の Overlap 区間・Fade/
  Crossfade 曲線を編集する対象。
- 両者は **別 Target** であり、hit-test・gesture fixation・command・
  undo 単位で区別する。Hard Cut / Transition 無効化は **明示操作**
  でのみ行い、境界ドラッグに暗黙で混ぜない。

### 6.2 Fixation と stale 拒否

- Gesture 開始時に Target ID（CompRegionId / CompTransitionId）と
  Project Revision を固定する。
- commit 時に固定済み Revision と現在 Revision が不一致の場合、
  **retarget せず reject** し、stale であることを表示する
  （ed-a / timeline-editing 正本の継承）。
- Source Handle 不足・Source 欠落・Invalid Segment・古い Revision は
  commit を reject し、理由を UI に表示する。無効データの読み出し・
  暗黙の丸めを行わない（md-a / rcd-04 fail-closed 正本の継承）。
- cross-window / 別 View からの編集は revision validation を経て、
  active gesture の target を暗黙に付け替えない（§15.7）。

### 6.3 受入との対応

TL-AC-07 / owner-matrix AC-31・AC-47。

## 7. 契約 C5: HitTest priority ordering

Take Lane / Comp Editor 面における hit test の優先順位を契約として
定義する。px 閾値・Snap Hysteresis・handle の視覚サイズは #11 §9 PoC
後続決定であり本契約では固定しない。

### 7.1 優先順位（上位が先に解決される）

upstream `timeline-editing-design.md` TE-T-02 の tier
（明示 Handle → 編集対象 → Clip Body → Lane Empty → View Background）
との対応を明示する。P2「編集対象」は upstream の「編集対象」tier、
P4「body」は upstream の「Clip Body」tier に対応し、**編集対象は
body より先に解決される**（upstream と同順）。

1. **P0 — Active gesture capture**: 進行中 gesture は開始時に固定した
   target を保持し、hit test を再評価しない。
2. **P1 — 明示 Handle**: 解決された lane/clip 行の中で、
   CompTransition Handle と Region Boundary Handle が他の全対象より
   先に解決される（upstream「明示 Handle」tier）。両 Handle が空間的に
   重複・曖昧な場合は推測で選ばず候補提示（ed-a Ambiguous 先例）または
   tool+modifier の明示 target 解決に委ねる。
3. **P2 — Tool 別の編集対象**（upstream「編集対象」tier）:
   - Smart Tool → timeline-editing 正本が定義する通常 Clip 編集対象
     （move/trim/slip 等の sub-clip target）
   - Dedicated Comp Tool → Comp Region / Transition の comp gesture
     対象（paint preview・boundary/transition drag）
4. **P3 — Take 候補 lane body**: Take Click（selection）・Audition
   trigger（C6.2 matrix に従う）の対象。Take lane は編集対象を持たない
   ため P2 は適用されず、body が直接解決される。
5. **P4 — Comp Region / Active Comp clip body**（upstream「Clip Body」
   tier）: Smart Tool では selection/scope のみ（編集対象は P2 で先に
   解決済み）。Dedicated Comp Tool では comp gesture 対象として P2 で
   先に解決されるため、ここに到達するのは selection 相当の解決のみ。
6. **P5 — Lane empty 領域**: Time Selection / lane scope の
   preview-only 対象（upstream「Lane Empty」tier）。
7. **P6 — View background**: deselect / focus 戻し（upstream
   「View Background」tier）。

### 7.2 補足

- Collapsed モードでは Active Comp clip は Arrangement の通常 Clip
  であり、Smart Tool では P2 の通常 clip 編集対象 → P4 の body として
  解決される。Comp Region 単位の操作が必要な場合は Expanded モード
  または Dedicated Comp Tool の明示 gesture を使う。
- Expanded モードでは pointer 位置の lane 行を先に解決し、行内で
  P1→P6 を適用する。異なる行の要素が視覚的に重なっても、pointer が
  指す行が優先される。
- Smart Tool における実効順序は upstream TE-T-02 と一致する
  （Handle → 編集対象 → Body → Lane Empty → Background）。
  Dedicated Comp Tool は P2 の編集対象集合を comp gesture へ拡張する
  tool-scoped の追加であり、upstream の tier 順序を反転させない。
- 本順序は timeline-editing / sv-gui-03-06 の hit-test・dispatch
  契約と矛盾しない範囲で定義する。既存契約が handle 優先・gesture
  capture 優先を規定する部分はそのまま継承する。

## 8. 契約 C6: Tool / Modifier semantics

### 8.1 Tool

- **Smart Tool**: hit-test で対象を解決する既定 tool。Take lane 上の
  click/drag は selection / scope のみを生成し、Comp 採用・paint
  preview を暗黙に開始しない。
- **Dedicated Comp Tool**: Comp Region / Take lane / Handle に対する
  comp 編集 gesture（paint preview・boundary/transition 編集）を担う
  専用 tool。Audition trigger は tool に依存せず C6.2 / C2 で定義する
  （Take lane 上の対象指定操作 + `MOD-AUDITION` または明示 command）。
  `tool.activate` の引数として登録されるが、sv-gui-03-06 正本の
  登録語彙は `smart/select/marquee/draw/split/trim/slip/stretch/
  fade/erase` の閉じた集合であり、**新 tool 名の追加は upstream
  契約・`Tool` enum の拡張を要する**（具体名と登録手順は実装
  slice の責務。本契約は「専用 comp tool が存在しなければならない」
  ことのみを固定する）。
- Tool 状態は View ごとに独立し（sv-gui-03-06 正本）、Tool 切替は
  Undo 対象外（daw-workflow 正本）。

### 8.2 Modifier semantics matrix

modifier は **役割名**で定義し、OS 別既定キー・配置は固定しない
（§14.8 未確定）。sv-gui-03-06 の `Mod` 正規化・View-context-only
発火規則を継承する。

| 役割 | semantics | 発火条件の契約 |
| --- | --- | --- |
| `MOD-AUDITION` | Take lane 上の対象を一時 Audition する | Take Click 等の対象指定操作と組合せて Runtime override を開始。tool 非依存（Smart Tool の Take Click + `MOD-AUDITION` でも可。modifier なしの Take Click は selection のみ）。Project 不変（C2） |
| `MOD-ADD` | 既存 selection への追加選択 | Take Click / Time Selection の scope を加算的に拡張。View-state のみ |
| `MOD-CONSTRAIN` | gesture の軸・量子化を制約する | paint/boundary/transition drag の preview 中に適用。commit 値のみに影響し、cancel で消える |

- 上記以外の modifier 組合せは本契約で意味を定義しない。実装 slice が
  追加する場合は本 design の改訂または別途合意を要する。
- TextInput / IME / Modal / Plugin GUI 等の input context では
  sv-gui-03-06 の suppression 規則が優先し、本 matrix の gesture は
  発火しない。

## 9. 契約 C7: Fade / audition defaults as contract

数値プリセットは固定しないが、以下の **保証の存在**を契約化する。

| # | 保証 | 内容 |
| --- | --- | --- |
| G1 | Transition の明示表現 | 隣接 Comp Region 間は transition 有無（Hard Cut 含む）を明示的な project 状態として持ち、UI から区別して操作できる |
| G2 | Fade の非破壊性 | fade/crossfade パラメータは commit 時に検証され project data として atomic に保存される。raw Source frame を変更しない（ed-b / md-a 正本の継承） |
| G3 | 既定値の存在 | fade/crossfade には project 既定値が存在し、新規 transition はその既定で初期化される。ただし数値（長さ・曲線の既定選択）は本契約で固定せず #11 §9 PoC 後続決定 |
| G4 | Curve 語彙の限定 | UI が提示する curve 種別は upstream model が定義する範囲に限定し、本契約は新しい curve 種別を追加しない |
| G5 | Audition の復帰経路 | audition は必ず C2.2 の解除経路を持ち、解除後は開始前の再生状態へ決定論的に復帰する。復帰不能な audition 状態を残さない |

## 10. 所有境界表（§14.6 の UI 語彙による正本化）

| UI 操作 | View State | Runtime State | Project State | Undo |
| --- | --- | --- | --- | --- |
| Lane expand/collapse | ✓（per-view） | — | — | — |
| Scroll / Zoom / Inspector 探索 | ✓ | — | — | — |
| Take Click | ✓ selection | — | — | — |
| Time Selection | ✓ scope | — | — | — |
| Audition 開始/終了 | ✓ indicator | ✓ playback override | — | — |
| Comp Paint / Adopt preview | ✓ preview | — | — | — |
| Comp Paint / Adopt commit | ✓ refresh | publish | ✓ Comp Region/Transition | ✓ 1 unit |
| Boundary / Transition edit commit | ✓ refresh | publish | ✓ 同上 | ✓ 1 unit |
| Window / Dock 移動・focus・link 変更 | ✓ | — | — | — |
| Recording finalization | — | ✓ session | ✓ Take/Source/Clip 登録 | ✓ session transaction（独立・通常 1 session = 1 undo） |
| View State / Runtime State のみの変更 | ✓ | ✓ | — | —（Project Undo 対象外） |

原則: **Project Undo は Project State を変更した操作のみを単位とする**。
View/Runtime のみの変更は Undo を生成しない。Recording finalization は
独立 transaction であり Comp 編集 undo と混ぜない（recording-take-comping
C7 正本の継承）。

## 11. Editor Binding 分界（§15.2〜§15.5 の UI 側責務）

- **Display Mode**: Effective Comp / Take Source / Compare の 3 mode は
  Editor の **view-state** であり、切替は Revision・Undo を変更しない
  （TL-AC-12 / AC-38・AC-39 の UI 面）。
- **Display Target ≠ Editing Target**: 表示対象の選択と編集対象の
  binding は別操作。Editing Target は **明示 binding**（対象
  Source Version / Comp を選ぶ command または gesture）を経てのみ
  確定する。
- **Effective Comp は投影**: Effective Comp は複数 Take/Source の
  投影であり、単一 raw Source として表示・編集対象にしない
  （md-a 正本の継承）。Editor で Effective Comp を開いても Source of
  record を変更しない。
- **Raw Capture version 0 は直接編集不可**: Source 編集は明示的に
  Editable Source Version へ binding した場合のみ。Raw Capture の
  version 0 を直接編集対象にしない（md-a 正本の継承）。
- **Audio / MIDI の責務差**: Audio Comp 編集は region/transition/fade
  handle 中心、MIDI Comp 編集は phrase 境界と note/controller 所有権
  中心（§15.4〜§15.5）。本契約は UI 上の対象分離のみを固定し、
  note-level schema・chase アルゴリズムは §15.6〜§15.9 未確定事項として
  後続へ残す。
- **Command / Target / Undo の分離**: source-local edit・comp edit・
  arrangement move/trim は別 command・別 target・別 undo 単位であり、
  相互に暗黙 retarget しない（AC-41 の UI 面）。

## 12. Scenario / Oracle

| # | Scenario | 期待結果 |
| --- | --- | --- |
| S1 | Expanded → Collapsed を往復 | Active Comp・Project Revision・Undo 履歴が不変。view-state のみ変化 |
| S2 | Take lane で Take Click → 別 Take を Audition → Audition 終了 | selection は view-state のみ。Audition は `AUDITION ONLY` 表示で Active Comp を一時 override し、終了で元再生へ復帰。Project/Undo 不変 |
| S3 | Dedicated Comp Tool で drag preview → Esc | Preview 中 Project 不変。Esc で変更なし。Undo 増えない |
| S4 | 同上 → pointer up commit | 全対象 validation 成功時のみ Comp Region 更新 + 単一 Undo。失敗時は Project 不変 + 理由表示 |
| S5 | Smart Tool で Take Click | selection のみ。Comp 採用・preview 開始なし |
| S6 | Gesture 開始後に別 View が Project を変更 → commit | 固定 Revision と不一致で reject。retarget しない。stale 表示 |
| S7 | Grouped Comp で一部 Track が Source Handle 不足 | 全 Track 不変。失敗 Track を表示。部分適用なし。Undo なし |
| S8 | Window/Dock を移動・focus 変更 | Record Target・Audition・Active Comp・Undo が不変 |
| S9 | Editor で Display Mode を Effective Comp → Take Source → Compare と切替 | view-state のみ変化。Revision/Undo 不変。Editing Target binding は別操作 |
| S10 | Region Boundary Handle と Transition Handle が重なる位置をクリック | 推測で選ばず候補提示または tool+modifier の明示 target 解決 |

## 13. Decision Log

| # | 決定 | 理由 | 却下案 |
| --- | --- | --- | --- |
| D-TL-1 | 7 契約領域を C1〜C7 + 所有境界表 + Editor Binding 節で固定 | dispatch spec の契約範囲と一致。upstream 機構契約を再定義せず UI 責務分界に限定 | 独自 undo/preview/audition モデルの新設（第二正本化・upstream 矛盾リスク） |
| D-TL-2 | HitTest 優先順位を契約として定義し px 閾値は PoC へ deferred | 優先順位は解釈分裂を防ぐ契約、数値は実測依存 | px 閾値まで本 slice で固定（根拠データなし・#11 §9 の責務侵害） |
| D-TL-3 | Modifier を役割名で定義し OS 別既定を固定しない | §14.8 が OS 別 Keyboard/Modifier/Touch を未確定と明記 | 既定キー割当の先行固定（未合意事項の勝手な確定） |
| D-TL-4 | ADR を作成しない | Issue §14/§15 合意の正本化であり新規の大きな分岐選択なし（dispatch spec 指定） | 機械的な ADR 追加（合意済み事項の重複記録） |
| D-TL-5 | Take lane provenance 表示は read-only 投影に限定 | lane 上の integrity/commit 表示が誤って編集対象化するリスクを排除 | lane 上で直接 recovery 操作を許可（Recovery Review の責務境界を侵す） |

## 14. Verification Strategy

本 slice は docs-only。検証は文書契約の整合性に限定する:

- docs index 再生成（doc-maintenance `index plan`/`apply`、impl 配下は
  `sync_impl_index` safe sync API）。
- doc-status audit/lint（新規 chain の type 別）と impl audit。
- `evidence/sv12/ui-contract-ledger.md` の AC ↔ test-spec case 対応が
  実在すること。
- **実行しないもの（正直な記録）**: cargo check/lint/test/build・
  `*-full`/`*-msrv`・実 GUI/実 OS/実 Device/実 Plugin（EXT-L3）・
  RT noalloc — 全て `NotRun`/`NotApplicable`。本 slice は `crates/**`
  を変更しない（POLICIES §5 比例原則）。契約・headless の Pass を
  実機 Pass に換算しない。

## 15. Deferred / 未解決（後続へ委譲）

- fade 既定長・曲線数値・crossfade 形状 → #11 §9 PoC 後続決定。
- hit-test px 閾値・Snap Hysteresis・handle 視覚サイズ → 同上。
- OS 別 Keyboard/Modifier/Touch 既定配置 → §14.8 未確定。
- 実 GUI/実 OS 受入 → #114・EXT-L3。
- MIDI note-level comp・chase 詳細・serialization → §15.6〜§15.9。
- Creative Branch Take/Comp 候補 → #278/#279/#288。
- Recovery Review の詳細 UI（Session/Pass/Track/Lane/Stream/
  Segment/integrity/commit/provenance の完全な画面設計）→ recording-
  take-comping C6/C7 正本の実装 slice（#114 系）。
