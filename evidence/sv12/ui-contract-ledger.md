# sv12 UI Contract Ledger — Take Lane / Comp Editor（synvelis#12 §14/§15 後続 slice）

synvelis#12 の UI 関連受入条件（owner-matrix の AC-28/29/30/31/32/36/
38/39/41/47）と、本 slice の canonical doc chain
（`docs/specs/take-lane-comp-editor.md` →
`docs/designs/take-lane-comp-editor-design.md` →
`docs/test-specs/take-lane-comp-editor.md`）の対応台帳。

`evidence/sv12/owner-matrix.md` は READ-ONLY の既存台帳であり本書は
それを改変しない。owner-matrix の状態（AC-28/29=`Deferred`、
AC-30/31/32/36/38/39/41=`Partial`、AC-47=`Pass (headless)`）は
実機・機構面の評価であり、本書は **UI 契約面** の固定状況を別途記録
する。両者を混同しない。

状態語彙:

- `Contract fixed (design)`: 本 slice の design/test-spec が UI 契約と
  して固定した（文書契約の存在を意味し、実機 Pass ではない）。
- `Deferred`: 実 GUI/実 OS/実 Device/実 Plugin での受入は #114/EXT-L3
  の別 Gate。契約固定を実機 Pass に換算しない（POLICIES.md 準拠）。

## AC 別台帳

| AC | 要求（要約） | 契約領域（design） | test-spec case | upstream 証拠参照 | 契約状態 | 実機状態 |
| --- | --- | --- | --- | --- | --- | --- |
| AC-28 | Collapsed/Expanded Take Lane 切替で Active Comp と Undo 履歴が不変 | C1（lane view-state vs project provenance） | TS-LANE-01, TS-LANE-02 | `docs/designs/ws-a-workspace-foundation-design.md`（view-state 分離）・`docs/designs/ed-a-editing-model-design.md`・`docs/designs/recording-take-comping-design.md` C6/C7 | **Contract fixed (design)** | **Deferred**（#114 実 lane UI） |
| AC-29 | Take Click/Time Selection/Audition は Comp 採用を変更せず Audition 終了で元状態復帰 | C2（non-mutation + restore） | TS-SELECT-01, TS-SELECT-02, TS-AUDITION-01, TS-AUDITION-02 | `docs/designs/ed-a-editing-model-design.md`・`docs/designs/ws-a-workspace-foundation-design.md`（selection/view-state）・recording-take-comping C6 | **Contract fixed (design)** | **Deferred**（#114 実 audition/take click UI） |
| AC-30 | 明示 Adopt/Comp Paint のみ Comp Region 変更・Preview 中 Project 不変・Esc/Cancel で変更なし・Commit 全件 Validation 単一 Undo | C3（gesture lifecycle）+ C5/C6 | TS-GESTURE-01, TS-GESTURE-02, TS-GESTURE-03 | `docs/designs/mda-comp-effective-projection-placement-design.md`・`docs/designs/ed-a-editing-model-design.md`（preview/cancel/atomic undo）・`docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md` | **Contract fixed (design)** | **Deferred**（#114 comp paint tool UI。機構は headless 実証済み） |
| AC-31 | Region Boundary/Transition Handle 操作・Target/Revision 分離・Source Handle 不足/欠落/古い Revision を正しく拒否・表示 | C4（handle fixation & stale reject）+ C5 | TS-HANDLE-01, TS-HANDLE-02, TS-HANDLE-03 | `docs/designs/mda-comp-effective-projection-placement-design.md`（handle 検証）・`docs/designs/ed-a-editing-model-design.md`（fixation）・`docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md` | **Contract fixed (design)** | **Deferred**（#114 実 handle UI） |
| AC-32 | Grouped Comp 一部 Track 不整合で成功対象だけ無言変更せず複数 Track 同時単一 Undo | C3.3（Grouped Comp all-or-nothing） | TS-GESTURE-04 | `docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md`（group CAS・partial 分離）・`docs/designs/mda-comp-effective-projection-placement-design.md` | **Contract fixed (design)** | **Deferred**（grouped comp 一括 command は #118 後続。機構 headless Partial） |
| AC-36 | Window/Dock/Inspector 移動・リンク/フォーカス変更だけで Record Target/Audition/Active Comp/Project Undo が誤変更されない | §10 所有境界表 + C2 | TS-EDITOR-01 | `docs/designs/ws-a-workspace-foundation-design.md`（dock/focus）・`docs/designs/recording-take-comping-design.md` C7（view/focus 非干渉）・`docs/designs/ed-a-editing-model-design.md` | **Contract fixed (design)** | **Deferred**（#114 実 window 操作） |
| AC-38 | Comp Clip を MIDI Editor で開き複数 Take の Note/CC 表示でも Source 正本不変 | §11 Editor Binding 分界 | TS-EDITOR-02, TS-EDITOR-04 | `docs/designs/mda-comp-effective-projection-placement-design.md`（editor targets・Effective Comp 投影）・`docs/designs/ed-b-media-domain-editing-design.md`（raw source 不変）・`docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md` | **Contract fixed (design)** | **Deferred**（#114 実 MIDI editor） |
| AC-39 | Effective Comp/Take Source/Compare 切替・Take Click・Audition で Project Revision/Undo が増えない | C2 + §11（Display Mode = view-state） | TS-AUDITION-01, TS-EDITOR-02 | `docs/designs/mda-comp-effective-projection-placement-design.md`（md_a_06 editor targets）・`docs/designs/ed-a-editing-model-design.md`・`docs/designs/ws-a-workspace-foundation-design.md` | **Contract fixed (design)** | **Deferred**（#114 実 compare/ghost UI） |
| AC-41 | Source-local Audio 編集と Comp Region/Transition 編集・Arrangement Clip Move/Trim が Command/Target/Undo で区別 | §10 所有境界表 + §11（command/target/undo 分離） | TS-EDITOR-02, TS-EDITOR-03 | `docs/designs/ed-b-media-domain-editing-design.md`（media domain）・`docs/designs/ed-a-editing-model-design.md`（commands）・`docs/designs/mda-comp-effective-projection-placement-design.md`・`docs/designs/daw-workflow-design.md`（undo lane） | **Contract fixed (design)** | **Deferred**（#114 実 editor 区別 UI） |
| AC-47 | 複数 Window による Source/Comp 編集競合を Revision で検出・入力 Gesture の Target を無断切替しない | C4.2（fixation & stale reject）+ C6 | TS-HANDLE-01 | `docs/designs/ed-a-editing-model-design.md`（gesture fixation・stale 拒否）・`docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md`（CAS stale reject）・`docs/designs/ws-a-workspace-foundation-design.md` | **Contract fixed (design)** | **Deferred**（実 2 window 同時編集は #114。機構 headless Pass） |

## サマリ

- 対象 AC: 10 件（AC-28/29/30/31/32/36/38/39/41/47）。
- `Contract fixed (design)`: 10 件 — 本 slice の design/test-spec が
  UI 契約として固定。
- 実機状態: 全件 `Deferred`（#114/EXT-L3 の別受入 Gate）。AC-32 は
  機構面も `Partial`（grouped comp 一括 command は #118 後続）。
- owner-matrix の既存 status は本書では変更しない。実機受入時に
  owner-matrix 側を更新するのは #114/EXT-L3 slice の責務。
