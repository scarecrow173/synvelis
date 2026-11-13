---
id: "IMPL-034X3VOP02J6rtCa1QMhIJ"
type: "impl"
status: "completed"
title: "Workspace UX 操作仕様の正本化（Issue #3 文書チェーン）"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/3"
  changes:
    added:
      - type: "file"
        path: "docs/specs/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/designs/workspace-ux-scenarios-design.md"
      - type: "file"
        path: "docs/plans/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/tasks/workspace-ux-scenarios-task.md"
      - type: "file"
        path: "docs/test-specs/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/impl/ir/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/impl/exp/workspace-ux-scenarios.md"
      - type: "file"
        path: "evidence/sv3/workspace-ux-scenarios-traceability.md"
      - type: "file"
        path: "evidence/sv3/workspace-ux-scenarios-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
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
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/workspace-ux-scenarios-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/workspace-ux-scenarios.md"
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/test-specs/workspace-ux-scenarios.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-ux-scenarios-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/workspace-ux-scenarios.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/workspace-ux-scenarios.md"
    rejected: []
---

# Workspace UX 操作仕様の正本化（Issue #3 文書チェーン）

## Summary

Issue #3 の Workspace 共通インタラクション仕様を canonical 文書
チェーン（spec → design → plan → task → test-spec → impl ir/exp）
と `evidence/sv3/` の追跡・検証記録として確定した。**製品コードの
実装は一切行っていない** — `crates/`・`Cargo.toml`/`Cargo.lock`・
`mise.toml`・CI・Ruviform・`docs/mockups/` への変更はゼロであり、
成果物は文書のみ。実 OS Window・実 Plugin GUI・実機モニター/DPI/
IME の検証は実施しておらず、各レーンは NotRun/Deferred として
明示した（POLICIES.md §2/§3、Issue #3 明示禁止に準拠）。

## Context

- Task: `docs/tasks/workspace-ux-scenarios-task.md`
- Plan: `docs/plans/workspace-ux-scenarios.md`
- Spec: `docs/specs/workspace-ux-scenarios.md`（R1〜R8・
  WUX-AC-01〜10）
- Design: `docs/designs/workspace-ux-scenarios-design.md`
  （UX 正本・状態図・Decision Log・WUX-SCN-A〜F）
- Issue: https://github.com/scarecrow173/synvelis/issues/3
  （2026-11-10 実行判断: Worker が設計ドラフト・契約/状態図・
  Case 表・review-protocol・PR まで担当。新規トレードオフのみ
  「提案/決定待ち」に分離）

## Implementation

### Changes

- `docs/specs/workspace-ux-scenarios.md` — Issue #3 §1〜§10・
  §9 受入条件を R1〜R8 / WUX-AC-01〜10 へ写像した要件正本。
- `docs/designs/workspace-ux-scenarios-design.md` — UX 規範本文。
  4 操作分類（Navigate/Inspect/Edit/Arrange）、5 つの mermaid
  状態図（View/Window/Inspector/Dock Drag/Layout 復元）、
  Docking 操作表（WUX-DK-01〜10）、閉鎖意味論、Inspector
  Follow/Explore/Pinned 遷移表、Link Group/Linked Context 分離、
  入力境界状態表、Layout 3+1 概念、シナリオ A〜F（Expected
  Oracle 付き）、Decision Log D1〜D24、Owner 境界と検証レーン。
- `docs/plans/workspace-ux-scenarios.md` — 単一文書ストリームの
  実施計画（file map・sequence・verification matrix）。
- `docs/tasks/workspace-ux-scenarios-task.md` — 実施単位と機械
  検査可能な `## Verification`。
- `docs/test-specs/workspace-ux-scenarios.md` — 文書契約の
  検証仕様（WUX-R-01〜11、Purpose/Feature/Rules/Examples/
  Guarantees/Non-goals/Risk）。
- `docs/impl/exp/workspace-ux-scenarios.md` — 文書生成過程の
  experiment 記録。
- `evidence/sv3/workspace-ux-scenarios-traceability.md` —
  WUX-AC-* → WUX-SCN-* → 設計節 → Issue § の追跡表と既存
  docs 整合確認。
- `evidence/sv3/workspace-ux-scenarios-verification.md` —
  実行した検証コマンド・結果・NotRun/Deferred レーン記録。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md`
  — `doc-maintenance index apply` による再生成 index。

### Decisions During Implementation

- 既存 WS-A（#36）・ED-A（#38）の headless 契約を再利用し、
  UX 規範は新規に重複定義せず「操作面の規範」として分離した
  （統合設計が論理構造・状態正本・Command 契約を担う既存分担を
  維持）。
- 「空になった Sub Window の自動閉鎖」（Issue §3 の「候補」）
  「Window 閉鎖の Layout Undo 粒度」「Follow 元 View 喪失時の
  Inspector 挙動」「ショートカット解決の DPI 非依存」を
  review-protocol の指摘（C1〜C6）を受けて確定化し、Decision
  Log D21〜D24 に記録した。
- review-protocol は spec+design に Reviewer A（協調）/
  Reviewer B（敵対）独立 pass → Validator → Judge で実施。
  CONFIRMED P0/P1 なし。P2/P3 指摘は Worker 共通契約
  「blocker 以外の指摘も必要なものは直す」に基づき全件解消。
  blocker なしをもって spec/design の status を `approved` と
  した（人間承認の捏造ではなく review-protocol 代替）。
- impl exp は impl-doc CLI の JSONL 経路ではなく、本 repo の
  既存慣行（`docs/impl/exp/*.md`、`type: "experiment"`）に
  合わせた Markdown 記録とした。

## Related Experiments

### Adopted

- `docs/impl/exp/workspace-ux-scenarios.md` — 既存 WS-A/ED-A
  契約の再利用方針・design-doc 生成時の構造問題（セクション
  ネスト）の修正判断・review-protocol 指摘への対応を記録。

### Rejected

- なし（単一アプローチで完了。JSONL experiment log は repo
  慣行の Markdown exp 記録を採用したため未使用）。

## Validation

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type spec --json` / `--type design --json`: 新規文書の
  finding は index/orphan 系のみ（index 再生成で解消予定）。
  blocking な front matter/relation エラーなし。
- review-protocol（spec+design）: verdict
  ACCEPT WITH NON-BLOCKING NOTES。CONFIRMED P0/P1 なし、
  指摘 C1〜C6 は修正済み。
- `doc_status lint`（repo 全体）: 既存 baseline に 122 件の
  blocking finding（無関係文書の invalid ID/date/relation）。
  本タスクの新規文書起因の finding とは区別して記録
  （`evidence/sv3/workspace-ux-scenarios-verification.md`）。
- `doc-driven-dev-graph` route: repo 全体の既存 broken
  relation により `blocked`（`focus-required`）を返す
  baseline 状態。新規文書の relation は個別 lint で解決確認済み。
- `mise run fmt-check`: 文書のみの変更のため Markdown/リンクの
  軽量チェックとして実行（結果は verification 記録を参照）。

## Risks

- 本書は UX 正本の**文書契約**であり、#112/#113/#114 の実装が
  全規定を満たす保証はない。乖離は各 Issue の受入で検出し
  本書へ差し戻す。
- 実機依存項目（OS 別 Window ドラッグ・Plugin GUI・異 DPI・
  IME）は real-os lane（#114/#176）依存であり、設計レビュー
  では確定できない。NotRun を Pass にしない運用で管理する。
- Layout Undo 履歴範囲・Explore 履歴深さ・閉鎖 Window 復元
  保持期間は未決定（design §15）であり、実装着手前に
  #112/#113/#5/#177 で確定が必要。

## Follow-ups

- #112: Core Workspace GUI Slice — 本 UX 正本を View/画面実装の
  要件として消費。
- #113（closed）/ #114: Dock 実装経路の実機確認と real-os
  受入（シナリオ A〜F の real-os レーン）。
- #176: 実 Plugin GUI lane（Plugin Focus/IME/Native popup/
  Move rollback の検証）。
- #5: Inspector 詳細設計（Explore 履歴深さ・Provider・
  Probe 連携の確定）。
- #11: Tool/Selection/Gesture 詳細（ED-A headless 契約の
  実装側詳細）。
- #12: Recording 連携（UI 遷移が Capture Target を変えない
  境界の実装確認）。
- Ruviform #181/#182/#386/#388: Window/Dock/NativeSurface
  契約の不足が判明した場合の upstream 参照先。
