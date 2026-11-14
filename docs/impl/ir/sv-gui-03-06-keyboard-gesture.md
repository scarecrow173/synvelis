---
id: "IMPL-rJLOVIiQN2lzaXHo3tegtw"
type: "impl"
status: "completed"
title: "SV-GUI-03 slice6: Keyboard/Gesture key-assignment contract(docs-only)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sv-gui-03-06-keyboard-gesture.md"
      - type: "file"
        path: "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
      - type: "file"
        path: "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
      - type: "file"
        path: "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
      - type: "file"
        path: "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
      - type: "file"
        path: "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
      - type: "file"
        path: "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
      - type: "file"
        path: "evidence/sv112/case-ledger.md"
      - type: "file"
        path: "evidence/sv112/verification.md"
    modified:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
    - "evidence/sv112/case-ledger.md"
    - "evidence/sv112/verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-03 slice6: Keyboard/Gesture key-assignment contract(docs-only)

## Summary

Issue #112 の本スライスとして、daw-workflow(Issue #8)が
#112 へ委譲したキーボード/ジェスチャ項目を canonical
契約文書チェーンで回収した。成果物は spec/design/plan/
task/test-spec/impl(ir/exp)の 7 文書と `evidence/sv112/`
の case ledger・検証記録、および doc-maintenance による
生成 index。**コード変更は一切行っていない** —
keymap dispatch 実装は実機 PoC へ Deferred。実 OS/実 GUI/
実機の検証は実施しておらず、各レーンは NotRun/Deferred
として明示した(POLICIES.md §2/§3、KG-AC-07)。

## Context

- Task: `docs/tasks/sv-gui-03-06-keyboard-gesture-task.md`
- Plan: `docs/plans/sv-gui-03-06-keyboard-gesture-plan.md`
- Spec: `docs/specs/sv-gui-03-06-keyboard-gesture.md`
  (KG-R-01〜07・KG-AC-01〜07)
- Design: `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`
  (§3.1 contract command catalog・§3.2 KG-KEY-01〜15 割当表・
  §3.3 OS 正規化・§4.1 発火行列・§5 安全契約・§6 メニュー
  網羅・§8 KG-D-1〜8)
- Issue: https://github.com/scarecrow173/synvelis/issues/112
  (docs-only スライス。keymap dispatch 実装は実機 PoC へ
  Deferred、コード変更禁止)

## Implementation

### Changes

#### Added

- `docs/specs/sv-gui-03-06-keyboard-gesture.md` — KG-R-01〜07・
  KG-AC-01〜07。daw-workflow design §7.2・§12 D-3・§13.2・
  spec「Deferred Design Concerns」の委譲項目を本スライスの
  受入条件として正規化。
- `docs/designs/sv-gui-03-06-keyboard-gesture-design.md` —
  契約本文: contract command catalog(`[contract]` id を
  `spec_for` 登録済みと明示分離)・既定キー割当表・
  `Mod` OS 正規化・InputContext×lane 発火行列・K-2/K-4
  安全契約・auto-repeat・KG-C-1〜6 競合ポリシー・
  CM-1〜6+§6.2 メニュー網羅対応・KG-D-1〜8。
- `docs/plans/sv-gui-03-06-keyboard-gesture-plan.md` —
  docs-only 単一 stream の plan(File Map・Sequence・
  Dependencies/Risks・Verification)。
- `docs/tasks/sv-gui-03-06-keyboard-gesture-task.md` —
  作業チェック・Verification コマンド・Done Criteria。
- `docs/test-specs/sv-gui-03-06-keyboard-gesture.md` —
  TS-DOC(TS-R-01〜07 文書契約検証)・TS-EX(TS-EX-01〜11
  headless 契約試験仕様)・TS-GUI(TS-GUI-01〜03 実機
  NotRun/Deferred)。
- `docs/impl/exp/sv-gui-03-06-keyboard-gesture.md` —
  設計判断ログ(既存契約再利用・契約先行アプローチ・
  KG-D-1〜8 の採否)。
- `evidence/sv112/case-ledger.md` — daw-workflow 委譲項目 →
  契約セクション → KG-AC → 受入状況の対応表と検証レーン別
  受入状況。
- `evidence/sv112/verification.md` — 実行した検証コマンド・
  exit code・NotRun/Deferred 記録・既知の範囲外 blocker。

#### Modified(生成物・doc-maintenance `index apply`)

- `docs/specs/README.md`・`docs/designs/README.md`・
  `docs/plans/README.md`・`docs/tasks/README.md`・
  `docs/test-specs/README.md`・`docs/impl/ir/README.md`
  — いずれも `doc_maintenance.js index apply` による再生成。
  手編集なし。`docs/impl/exp` は index in-sync(変更なし)。

### Decisions During Implementation

- **spec/design/plan/task/test-spec を approved/done 化**:
  review-protocol(協調/敵対の分離 pass + 自己照合)で
  blocker なしを確認した時点を承認代替として扱い、
  plan-doc の approved-design ゲートを通過させた。
  人間承認の捏造ではなく coordinator 契約に基づく手続き。
- **`[contract]` command id の分離**: `transport.toggle`/
  `stop-return`/`record`・`project.save`/`export`・
  `edit.delete`/`copy`/`paste`・`view.zoom-*`/`scroll-*` は
  `spec_for` 未登録の契約先行 id として命名し、登録は
  実装 slice へ Deferred(KG-D-8)。
- **View/Application lane のゲート配置**: 現行 `dispatch` は
  View/Application kind を抑制しないため keymap 層ゲートが
  必要と判明。契約は振る舞い(§4.1 行列)のみ固定し、
  実装配置は Deferred(KG-D-4)。
- **`Ime` Undo 配送の不整合を記録**: workspace-ux §9.1 は
  Ime Undo「発火しない」だが現行 `dispatch_undo` は text
  lane 配送。契約は現行コード挙動に合わせ、実機検証・
  文書照合は #114 へ委譲(design R-3)。
- **メニュー発行 Command の input**: `input=Modal` では
  suppression で何も実行できないため `input=View`+
  `explicit_targets` とする(KG-D-6)。
- **impl exp は repo 慣行の `.md`**: `docs/impl/exp/
  sv-gui-03-06-keyboard-gesture.md`(`type: "experiment"`)。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-gui-03-06-keyboard-gesture.md` —
  「既存 headless 契約(`focus.rs`/`command.rs`/`workspace.rs`)
  を READ-ONLY で照合し、契約文書を先に正本化する」
  アプローチを採用。`spec_for` 登録・View/Application
  suppression 配置は実装判断を含むため、コード変更禁止の
  本スライスでは契約のみを固定した。

### Rejected

- OS 別に 2 表を維持する案(正規化の一意性を損なう)、
  全 binding の repeat 許容案(トグルストロボ・連続削除
  リスク)、メニュー発行を `input=Modal` にする案
  (suppression で実行不能)、Tool 全種へのキー割当案
  (最小集合超過)。

## Validation

`evidence/sv112/verification.md` に全記録。概要:
`doc_maintenance.js index plan`→`apply`(specs/designs/
plans/tasks/test-specs/impl-ir、exp は in-sync)・
`doc_status.js lint`(sv-gui-03-06 由来 finding 0)・
`doc_status.js audit`(664 blocking は全て既存文書由来、
本スライス 0)・`audit_impl_record.js`(section 修正後 error
0)・`mise run fmt-check`(exit 0、no-op)・`git diff --check`・
`git status` 範囲確認。実機/実 GUI/実 OS 検証は NotRun
(#114/#176 lane)。

## Risks

- `[contract]` id が `spec_for` 未登録のまま契約先行 →
  design §3.1 で登録済み/contract を明示分離済み。
- `Ime` Undo 配送の文書不整合 → design R-3・実機検証は
  #114。
- View/Application suppression の実装配置が今後二重化
  するリスク → 契約は振る舞いのみ固定、配置は Deferred。
- 既定セットが workflow で不足 → メニュー面・リマップ・
  §7.2 改訂で吸収、追加 binding は Decision Log 記録を要す。
- doc-driven-dev-graph router の既存 broken relation →
  範囲外、evidence に記録。

## Follow-ups

- #112 実装 slice: keymap dispatch・`spec_for` 登録・
  リマップ UI・TS-EX-01〜11 の headless 契約試験実走。
- #114/#176: 実 OS/実 GUI/実機キー・IME/Plugin GUI 検証
  (TS-GUI-01〜03)。
- workspace-ux §9.1 vs `dispatch_undo` の `Ime` Undo
  文書照合 → #114 実機結果を踏まえて判断。
- doc-driven-dev-graph の既存 broken relation(repo 全体の
  graph 債務)→ coordinator 判断で別途対応。
