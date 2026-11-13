---
id: "PLAN-034X3PYp3yD3aBha3pGepA"
type: "plan"
status: "in-progress"
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
    - "docs/specs/workspace-ux-scenarios.md"
  implemented-by:
    - "docs/tasks/workspace-ux-scenarios-task.md"
  depends-on:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/mockups/synvelis-daw.html"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/workspace-ux-scenarios.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/182"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/388"
  defers: []
  deferred-by: []
---

# Workspace UX 操作仕様の正本化（Issue #3 文書チェーン） Implementation Plan

## Goal

Issue #3 の Workspace 共通インタラクション仕様を、spec → design →
plan → task → test-spec → impl 記録の canonical 文書チェーンと
`evidence/sv3/` の追跡・検証記録として確定し、後続の実装 Issue
（#112/#113/#114/#5/#11/#12）と Ruviform（#181/#182/#386/#388）が
参照できる UX 正本を供給する。

## Scope

単一の文書ストリーム。`crates/`・`mise.toml`・CI・Ruviform・
`docs/mockups/` には触れない。成果物は `docs/` 配下の 7 文書と
`evidence/sv3/` の新規ファイル、および `doc-maintenance` が再生成
する各 `docs/<type>/README.md` index のみ。製品コードの実装・
実 OS/実 Plugin GUI 検証は本 plan の対象外（各 Owner Issue の
lane に委譲）。

## Upstream Documents

- `docs/specs/workspace-ux-scenarios.md`（要件 R1〜R8・受入
  WUX-AC-01〜10、本 plan の `implements` 対象）
- `docs/designs/workspace-ux-scenarios-design.md`（UX 正本:
  状態図・操作表・Decision Log・シナリオ A〜F・Owner 境界）
- `docs/designs/workspace-docking-state-architecture.md`（統合設計
  正本: 論理構造・状態正本・Command 契約）
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  / `docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md`
  （headless 固定済み契約）

## Design Inputs

- `docs/designs/workspace-ux-scenarios-design.md`（status:
  approved。review-protocol 実施済み・CONFIRMED P0/P1 なし）
- `docs/designs/overview.md`（design gate 要件として存在確認済み）

## File Map

- `docs/specs/workspace-ux-scenarios.md` — 要件・受入条件の正本
  （作成済み）
- `docs/designs/workspace-ux-scenarios-design.md` — UX 規範本文の
  正本（作成済み）
- `docs/plans/workspace-ux-scenarios.md` — 本 plan
- `docs/tasks/workspace-ux-scenarios-task.md` — 実施単位の task
  記録（本 plan の `implemented-by`）
- `docs/test-specs/workspace-ux-scenarios.md` — 仕様が保証すべき
  性質の検証仕様（`verifies` → spec、`derives-from` → design）
- `docs/impl/ir/workspace-ux-scenarios.md` — 実施記録（製品
  実装なし・文書作成のみであることを明記）
- `docs/impl/exp/workspace-ux-scenarios.md` — 文書生成過程の
  experiment log（impl-doc CLI 経由で JSONL 管理）
- `evidence/sv3/workspace-ux-scenarios-traceability.md` —
  WUX-AC-* → WUX-SCN-* → 設計節 → Issue § の追跡表と既存 docs
  整合確認
- `evidence/sv3/workspace-ux-scenarios-verification.md` —
  実行した検証コマンド・結果・NotRun/Deferred レーンの記録
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md`
  — `doc-maintenance index apply` による再生成 index（手編集
  しない）

## Implementation Sequence

- [x] spec `docs/specs/workspace-ux-scenarios.md` を作成し、
  Issue #3 §1〜§10・§9 受入条件を R1〜R8 / WUX-AC-01〜10 へ
  写像する
- [x] design `docs/designs/workspace-ux-scenarios-design.md` を
  作成し、状態図（View/Window/Inspector/Dock Drag/Layout 復元）・
  操作表・Decision Log・シナリオ A〜F・Owner 境界を規定する
- [x] review-protocol を spec+design に独立実施し、CONFIRMED
  P0/P1 がないことを確認する（P2/P3 指摘 C1〜C6 を解消済み）
- [x] task `docs/tasks/workspace-ux-scenarios-task.md` を作成し、
  機械検査可能な `## Verification` を記す
- [x] test-spec `docs/test-specs/workspace-ux-scenarios.md` を
  作成し、文書契約の保証事項を Feature/Rule/Example で固定する
- [x] impl ir/exp を作成し、「製品実装なし・文書成果物のみ」
  を明記する
- [x] `evidence/sv3/` に追跡表と検証記録を作成する
- [x] `doc-maintenance index apply` で触れた dir の index を
  再生成する
- [x] `doc_status lint`・`mise run fmt-check` を実行し、新規
  finding と既存 baseline を区別して記録する
- [x] `git fetch origin main && git merge origin/main` で最新
  main を取り込み、生成物を再生成してコミットする
- [x] Conventional Commits でコミットし、`docs/3-workspace-ux`
  を push して PR を作成する

## Dependencies and Risks

- **依存**: 統合設計・WS-A・ED-A・SV-GPUI-03・sv-gui-04 slice2
  の既存契約を前提とする。これらは全て repo 内に存在し、
  `doc_status lint` で relation 解決を確認済み。
- **リスク: 仕様と実装の乖離** — #112/#113/#114 の実装が本 UX
  正本を全て満たす保証はない。乖離は各 Issue の受入で検出し、
  本書へ差し戻す（design §14）。
- **リスク: 実機依存項目の滞留** — OS 別 Window ドラッグ・
  Plugin GUI・異 DPI は real-os lane 依存。NotRun を Pass に
  しない運用で管理する。
- **リスク: 文書のみの検証限界** — 本タスクの検証は文書契約の
  静的検査であり、UX 挙動の実測ではない。real-os/headless
  の後続検証は別 lane として明示する。

## Task Breakdown

- 単一 task `docs/tasks/workspace-ux-scenarios-task.md` に集約
  （文書チェーンは相互依存が強く、分割すると relation の
  往復が増えるだけで独立検証可能な stream にならない）。

## Verification

- [x] `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type spec --json` / `--type design --json` で新規文書の
  blocking finding が index/orphan のみ（再生成で解消）である
  ことを確認
- [x] review-protocol（spec+design）: CONFIRMED P0/P1 なし、
  verdict ACCEPT WITH NON-BLOCKING NOTES（指摘は解消済み）
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply` で index を再生成し、lint の index/orphan
  finding が消えることを確認
- [ ] `mise run fmt-check` が exit 0
- [ ] 全変更が `docs/`・`evidence/sv3/`・生成 index の範囲内
  であることを `git status` / `git diff --stat` で確認

## Open Questions / Gaps

- Drop zone 寸法・ドラッグ閾値・Split 最小サイズ、Explore 履歴
  の深さ、閉鎖 Window 復元履歴の保持期間、Layout Undo 履歴範囲
  は Issue #3 §10 の未決定事項として design §15 / spec
  Deferred Design Concerns に記録済み。実機/UI PoC・#5・#177
  運用で確定する。
- 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言は
  「提案/決定待ち」として分離し、人間承認を捏造しない
  （Issue 2026-11-10 実行判断、design D20）。

## Self-Review

- [x] Every spec requirement maps to a task.（R1〜R8 → design
  §5〜§10・WUX-SCN-A〜F → test-spec Feature/Rule で追跡）
- [x] No placeholders remain.（TBD/TODO なし。未決定事項は
  §15/Deferred に明示的 gap として記録）
- [x] File paths, relations, status values, and terminology are
  consistent.（WUX-AC-*/WUX-SCN-*/WUX-DK-* の ID 体系を spec・
  design・test-spec・evidence で統一）

## Implementation Handoff

- Recommended mode: inline execution（文書成果物のため分割
  不要・本 plan が execution stream を兼ねる）
- Independent work streams: none
- User approval required before dispatch: yes（本タスクは Orca
  dispatched worker として Coordinator の task spec に基づき
  実行。review-protocol blocker なしを承認代替とする）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 前提: 統合設計・WS-A・ED-A の既存契約を再利用し、製品固有
  UX 規則を Ruviform へ持ち込まない境界（design §13）を確認
  すること。
- 未解決の依存: 実機 lane（#114/#176）と Inspector 詳細（#5）
  は後続 Issue であり、本 plan の完了条件に含めない。
- reviewer focus: WUX-AC-01〜10 と Issue #3 §9 受入条件の
  写像漏れ、NotRun/Deferred の誤記（Pass 誤記）がないこと。
