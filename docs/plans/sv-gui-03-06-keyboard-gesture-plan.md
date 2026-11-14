---
id: "PLAN-La8HY0jgJiSxqNij6Hj6OQ"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract 文書チェーン（Issue #112 docs-only slice）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract 文書チェーン Implementation Plan

## Goal

Issue #112 の本スライスとして、daw-workflow(Issue #8)が #112
へ委譲したキーボード/ジェスチャ項目(§7.2 キー割当・§12 D-3・
§13.2・コンテキストメニュー網羅)を canonical 契約として
spec → design → plan → task → test-spec → impl(ir/exp)の
文書チェーンと `evidence/sv112/` case ledger・検証記録で
確定する。keymap dispatch の実装は実機 PoC へ Deferred、
コード変更は一切行わない。

## Scope

本 plan は **文書成果物の作成のみ**を対象とする単一の
implementation stream である。`crates/` 以下のコード実装、
実 OS/実 GUI/実機検証、`Cargo.toml`/`Cargo.lock`/`mise.toml`/
CI の変更はスコープ外。`focus.rs`/`command.rs`/
`workspace.rs` は READ-ONLY の契約根拠として参照のみ。

## Upstream Documents

- `docs/specs/sv-gui-03-06-keyboard-gesture.md` — KG-R-01〜07・
  KG-AC-01〜07。
- `docs/designs/sv-gui-03-06-keyboard-gesture-design.md` —
  §3.1 contract command catalog・§3.2 KG-KEY-01〜15 割当表・
  §3.3 OS 正規化・§4.1 発火行列・§5 安全契約・§6 メニュー
  網羅・§8 Decision Log KG-D-1〜8。
- 上位正本: `docs/designs/daw-workflow-design.md`(§7.2・
  K-1〜K-5・D-3・§13.2)、`docs/designs/workspace-ux-scenarios-
  design.md`(§9 入力境界)、`docs/designs/timeline-editing-
  design.md`(#11 Tool)、`docs/designs/workspace-model-design.md`。

## Design Inputs

- `docs/designs/sv-gui-03-06-keyboard-gesture-design.md`(approved)
- `docs/designs/overview.md`(design gate 前提、存在確認済み)

## File Map

- `docs/specs/sv-gui-03-06-keyboard-gesture.md` - 要件・KG-AC
  の正本（作成済み）
- `docs/designs/sv-gui-03-06-keyboard-gesture-design.md` -
  契約本文: 割当表・正規化・suppression 行列・安全契約・
  競合ポリシー・メニュー網羅（作成済み）
- `docs/plans/sv-gui-03-06-keyboard-gesture-plan.md` - 本 plan
- `docs/tasks/sv-gui-03-06-keyboard-gesture-task.md` - 実装
  スライスと検証手順
- `docs/test-specs/sv-gui-03-06-keyboard-gesture.md` - 文書
  契約の検証仕様(TS-R/TS-EX)
- `docs/impl/ir/sv-gui-03-06-keyboard-gesture.md` - 実施記録
- `docs/impl/exp/sv-gui-03-06-keyboard-gesture.md` - 設計判断の
  検討記録(repo 慣行の `.md` `type: "experiment"`)
- `evidence/sv112/` - case ledger(委譲項目→契約セクション→
  受入状況)・検証記録
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md`
  - doc-maintenance `index apply` による生成 index(手編集
  しない)。`docs/impl/exp` の index は `.jsonl` のみ対象で
  repo 慣行の `.md` exp は対象外(sv8 先例)

## Implementation Sequence

- [x] spec 作成・review-protocol(協調/敵対の分離 pass)実施・
  blocker 解消・approved 化
- [x] design 作成・review-protocol 実施・blocker 解消・
  approved 化
- [x] plan 作成(approved design を前提)
- [ ] task 作成(approved plan を前提)
- [ ] test-spec 作成・内容記入(verifies=spec)
- [ ] evidence/sv112/ 台帳作成(KG-AC→契約節→受入状況・
  OS-L1/GUI-L2/EXT-L3 マトリクス・NotRun/Deferred 記録)
- [ ] impl ir/exp 作成・内容記入
- [ ] doc-maintenance `index plan`→`index apply` で触れた
  dir の index を再生成
- [ ] `doc_status lint/audit`・`mise run fmt-check`・
  `git diff --check` で検証
- [ ] Conventional Commit → push → PR 作成(日本語本文、
  `Refs #112`、Issue は Close しない)

## Dependencies and Risks

- **依存**: 上位正本 docs(#2/#3/#8/#11)の存在 — 確認済み。
- **依存**: plan-doc ゲート(approved design 必須) — design を
  review-protocol 合格後に approved 化して解消。
- **リスク R-1**: doc-driven-dev-graph router が既存の無関係な
  broken relation で fail-closed する(sv3/sv8 先例、今回も
  `probe` node で blocked を確認)→ 宣言済み文書順で手動継続し、
  router 結果を偽装せず evidence に記録。
- **リスク R-2**: `[contract]` command id が `spec_for` 未登録の
  まま契約だけ先行 → design §3.1 で登録済み/contract を明示
  分離し、登録は実装 slice へ Deferred。
- **リスク R-3**: `Ime` Undo 配送が workspace-ux §9.1 と現行
  `dispatch_undo` で不整合 → design R-3 として記録し、実機
  検証は #114 へ委譲。

## Task Breakdown

- T-1: docs/specs/sv-gui-03-06-keyboard-gesture.md(完了)
- T-2: docs/designs/sv-gui-03-06-keyboard-gesture-design.md
  (完了)
- T-3: docs/plans/sv-gui-03-06-keyboard-gesture-plan.md(本書)
- T-4: docs/tasks/sv-gui-03-06-keyboard-gesture-task.md
- T-5: docs/test-specs/sv-gui-03-06-keyboard-gesture.md
- T-6: evidence/sv112/ 台帳・検証記録
- T-7: docs/impl/ir + docs/impl/exp の sv-gui-03-06 文書
- T-8: index 再生成 + 検証 + commit/push/PR

## Verification

- [x] `node .agents/skills/doc-status/scripts/doc_status.js
  lint --dir docs/specs` / `--dir docs/designs` — 新規文書の
  deterministic finding を確認(未作成 doc への relation・
  index 未反映は後続工程で解消する既知分)
- [ ] `node .agents/skills/doc-maintenance/scripts/
  doc_maintenance.js index plan --dir docs/specs` 等で index
  差分を preview し、`index apply` で適用
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js
  audit` で新規文書由来の blocking finding がないことを確認
- [ ] `node .agents/skills/impl-doc/scripts/
  audit_impl_record.js --json` で IR 監査
- [ ] `mise run fmt-check`(docs-only のため cargo fmt の
  no-op 確認)
- [ ] `git diff --check`(空白エラーなし)
- [ ] `git status` で変更が許可範囲 + 生成 index +
  `evidence/sv112/` のみであることを確認

## Open Questions / Gaps

- keymap dispatch 実装・`spec_for` 登録 → #112 実装 slice。
- リマップ UI/保存形式 → 後続設定 slice。
- 実 OS/実 GUI/実機キー・IME/Plugin GUI 検証 → #114/#176。
- Tool 数値・Recording 詳細 → #11/#12 が正本。

## Self-Review

- [x] Every spec requirement(KG-R-01〜07)maps to a task/文書。
- [x] No placeholders remain.
- [x] File paths、relations、status 値、用語
  (`InputContext`/`CommandKind`/`CommandOutcome`)は既存
  コード・上位文書と一致。

## Implementation Handoff

- Recommended mode: inline execution(文書作業のみ、委譲対象の
  コード実装なし)
- Independent work streams: none
- User approval required before dispatch: yes(review-
  protocol による代替承認済み — coordinator 契約に基づく)

## Review Handoff

- 前提: spec/design は review-protocol(協調/敵対の分離 pass +
  validator/judge 相当の自己照合)で blocker なしを確認済み。
- レビュアー注目点: §3.2 割当表が daw-workflow §7.2 の全操作を
  網羅しているか、§4.1 行列が `captures_input`/`dispatch`/
  `dispatch_undo` の実装と矛盾しないか、§6.2 メニュー網羅が
  「完全網羅」委譲を契約として十分に規定しているか、
  `evidence/sv112/` の NotRun/Deferred 記録が誠実か。
