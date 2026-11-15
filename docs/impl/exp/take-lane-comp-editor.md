---
id: "EXP-take-lane-comp-editor"
type: "experiment"
status: "completed"
title: "Take Lane / Comp Editor — UI 詳細契約 canonical doc chain 実験・判断記録（synvelis#12 §14/§15 後続 slice — docs-only）"
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
  implemented-by:
    - "docs/impl/ir/take-lane-comp-editor.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/take-lane-comp-editor-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/take-lane-comp-editor-task.md"
  defers: []
  deferred-by: []
---

# Take Lane / Comp Editor — UI 詳細契約 doc chain 実験・判断記録（synvelis#12 §14/§15 後続 slice — docs-only）

## E1 doc-driven-dev-graph router の状態

`route_graph.js --cwd . --json` を実行。`status: blocked`、
`current/next: probe`、`edgeId: null`。blocker は **既存の
repository-wide broken relation**（`docs/designs/automation-
modulation-design.md`・`rcd-*`・`tim-02-*`・`vcs-*`・`docs/impl/ir/*`・
`docs/impl/exp/*` の deferred relation が存在しない path を指す）で
あり、本 slice が新たに作ったものではない。`recording-take-comping`
（sv#12）・`rcd-f02-monitoring-overdub`（sv#120）先例と同様、
この blocked は slice 起因でないため、router が返す delegate が無い
まま `spec → design → plan → task → test-spec → impl/ir + impl/exp`
の文書チェーンを規約どおり作成した。無関係な broken relation は
**修復しない**（範囲外）。exp に記録して後続の doc-status 修復
slice に委譲。

## E2 artifact ID の生成

`SPEC-`/`DESIGN-`/`PLAN-`/`TASK-`/`TSPEC-`/`IMPL-` + 22 文字 Base62 の
artifact ID を crypto random で生成（`recording-take-comping` 先例の
手動生成方式に準拠）。生成値:

- `SPEC-xOTmUaWxKVIkrr8hUBfugh`（docs/specs/take-lane-comp-editor.md）
- `DESIGN-clIJhvdpK3FeleehsQ9lfb`（docs/designs/take-lane-comp-editor-design.md）
- `PLAN-iVpbmseH193L48I6MLSzF8`（docs/plans/take-lane-comp-editor.md）
- `TASK-3E7SSn7u5OV3w997JoXm8f`（docs/tasks/take-lane-comp-editor-task.md）
- `TSPEC-UB4fnjlz7vYEkTNeKiDU3G`（docs/test-specs/take-lane-comp-editor.md）
- `IMPL-JB9syTgzH7oggQKcr0ocJE`（docs/impl/ir/take-lane-comp-editor.md）
- `EXP-take-lane-comp-editor`（本書。exp memo は rcd-f02 先例の簡易
  schema に準拠）

## E3 7 契約領域の構成と選択肢の採否

dispatch spec が確定した 7 領域を design の契約 C1〜C7 + 補遺で固定:

- **C1 lane view-state / C2 non-mutation+restore / C3 gesture
  lifecycle / C4 handle fixation / C5 hit-test ordering / C6
  tool+modifier / C7 fade+audition guarantees** を採用。
- 補遺として §14.6 所有境界表（UI 語彙）と §15.2〜15.5 Editor Binding
  分界を追加 — 7 領域の列挙に Editor Display/Editing 分離が明示的に
  含まれないため、AC-38/39/41 をカバーする責務分界として独立節に配置。
- 却下: 独自 undo/preview/audition モデルの新設（upstream 正本との
  第二正本化・矛盾リスク）。機構契約は depends-on 参照で継承し、本
  slice は「どの UI 操作がどの状態を触るか」の責務分界に限定。
- 数値プリセット（fade 既定・px 閾値・Snap Hysteresis）と OS 別既定
  キーバインド/Touch は §14.8/§15.9 未確定として明示 deferred。
  modifier は役割名で定義しキー割当を固定しない（D-TL-3）。
- ADR は作成しない（dispatch spec 指定 — 合意済み事項の正本化であり
  新規の大きな分岐選択なし）。

## E4 depends-on/related path の実在確認

upstream 参照は `ls docs/specs docs/designs` で実名を確認してから記述
（`ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`・
`ed-b-media-domain-move-slip-commit-semantics.md`・
`ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`・
`mda-takegroup-comp-effective-projection-placement-contract.md`・
`rcd-04-comp-transition-midi-phrase-fixture.md`・
`sv-gui-03-06-keyboard-gesture.md`・`daw-workflow.md`・
`timeline-editing.md`・`recording-take-comping.md` — 全て実在）。
design 側は対応する `-design`  suffix 付き実名を採用
（`ed-b-media-domain-editing-design.md`・
`mda-comp-effective-projection-placement-design.md`・
`ws-a-workspace-foundation-design.md` 等）。修正後、全 local relation
path が実在することを確認した（broken relation を新たに増やさない）。

## E5 docs-only slice の正直な記録

- 本 slice は **文書のみ**。`crates/**`・`Cargo.toml`/`Cargo.lock`・
  `mise.toml`・CI・`POLICIES.md`・`AGENTS.md`・`owner-matrix.md` は
  一切変更していない。
- cargo test/build/lint・実 GUI/実 OS/実 Device/実 Plugin 検証は
  **実行していない**（POLICIES §5 比例原則 — docs-only は軽量
  チェックが原則）。これらは `NotRun`/`NotApplicable` として
  記録し、契約/headless の成功を実機 Pass に換算しない。
- 実 OS GUI での Take Lane/Comp Editor/Editor 統合受入は #114 系別
  受入 Gate の責務であり、本 slice では `Deferred`。
- owner-matrix AC-28/29 の `Deferred`・AC-30/31/32/36/38/39/41 の
  `Partial`・AC-47 の `Pass (headless)` は本 slice では書き換えない。
  ui-contract-ledger は「契約 fixed / 実機 Deferred」を別列で記録し、
  既存 status を変更したと見なされない形にする。

## E6 review-protocol cycle の記録

`review-protocol` Skill に従い Reviewer A（cooperative）/Reviewer B
（adversarial）の 2 独立レビューを実施。Issue Candidate を Validator
（実ファイル・owner-matrix・upstream 設計との照合）で確認し、以下を
CONFIRMED として修正:

- spec/ir が owner-matrix の AC-47 を `Partial` 群に誤記
  （正しくは `Pass (headless)`）→ 修正。
- design C5 hit-test 順序が upstream TE-T-02（Handle → 編集対象 →
  Clip Body → Lane Empty → Background）との tier 対応を未記述で
  読み違い余地 → tier 対応を明示する形に改訂。
- design 内の壊れた内部参照 `(C7.2)` → C6/C5 の正しい参照へ修正
  （改訂に伴い消滅）。
- Audition trigger の tool 依存が C6.1/C6.2/C3.2 間で不整合
  → tool 非依存（Take lane 対象指定 + `MOD-AUDITION` または明示
  command）と明確化。
- ledger の AC-38/AC-41 が TS-EDITOR-02 のみへ写像し区別挙動を
  exercise しない → TS-EDITOR-03（3 種編集の command/target/undo
  分離）・TS-EDITOR-04（MIDI editor 複数 take 表示で source 不変）
  を test-spec に追加し ledger を更新。
- Dedicated Comp Tool の `tool.activate` 登録が upstream の閉じた
  語彙（`smart/select/…/erase`）を前提 → 新 tool 名追加は upstream
  契約・enum 拡張を要する旨を明記。
- ir `changes` が `docs/impl/exp/README.md` を modified/generated に
  誤記（exp index は `.jsonl` のみ管理で `.md` memo は対象外・実 diff
  なし）→ 両リストから除去。
- task `status: done` なのに Verification checkbox が全て未チェック
  → 実行済みコマンドを `[x]` + 結果で記録。
- design の `Collapsed（既定）` が根拠なしに見える → Issue §14 の
  合意（Arrangement は通常 Active Comp 1 clip）を明記。

修正後、P0/P1 の未解決 blocker は 0。P2/P3 も全て対応済み。
