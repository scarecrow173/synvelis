---
id: "PLAN-034X3jT4T64MOzF6CbB2oy"
type: "plan"
status: "completed"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/product-value-core-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/product-value-core-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/product-value-core-task.md"
    - "docs/test-specs/product-value-core.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/product-value-core-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/product-value-core.md"
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画 Implementation Plan

## Goal

Issue #1 の製品価値・中核体験・Gate A/B/C/AI・UX 評価計画を
`docs/` の canonical 文書チェーンと `evidence/sv1/` の根拠記録
として正本化し、6 項目の Acceptance Matrix を評価可能な形で
確定する。コード変更は行わない。

## Scope

文書・証拠のみの単一ストリーム。`crates/`、`mise.toml`、CI、
`Cargo.toml`/`Cargo.lock` は変更しない。実装・実 OS/RT/実 GUI
受入は #8/#23・#4/#6・#102/#103・#108/#109・#114 等の Owner
Issue/PR に委譲し、本計画はそれらへの相互参照と判定規則を
提供する。

## Upstream Documents

- `docs/specs/product-value-core.md`（製品定義の正本）
- `docs/designs/product-value-core-design.md`（Gate 構造化・
  証拠状態機械・Owner 境界・Decision Log）
- Source: synvelis#1、synvelis#277

## Design Inputs

- `docs/designs/product-value-core-design.md`（approved）

## File Map

- `docs/specs/product-value-core.md` — 製品価値・Gate・Acceptance
  Matrix・UX 比較計画の正本。
- `docs/designs/product-value-core-design.md` — Gate 構造化・
  証拠状態機械・Owner 境界・代表シナリオ Oracle・Decision Log。
- `docs/plans/product-value-core-plan.md` — 本書。
- `docs/tasks/product-value-core-task.md` — 実施タスク。
- `docs/test-specs/product-value-core.md` — Acceptance Matrix と
  証拠正直性の Feature/Rule/Example。
- `docs/impl/ir/product-value-core.md` — 実施記録。
- `docs/impl/exp/product-value-core.md` — 検討・却下案の記録。
- `evidence/sv1/acceptance-matrix.md` — Acceptance Matrix 各行の
  根拠（Issue 節・既存証拠・Owner・NotRun 宣言）。
- `evidence/sv1/issue-1-scope-map.md` — 既存 docs との重複・矛盾
  調査と解消方針。
- `evidence/sv1/verification.md` — 本変更の検証記録。
- `docs/<type>/README.md` — 生成 index の再生成（`doc_maintenance
  index apply`）。

## Implementation Sequence

- [x] Issue #1 本文と関連 Issue（#8/#23/#4/#6/#102/#103/#108/
  #109/#114/#277/#2/#3/#44/#46）の受入条件・Owner を確認。
- [x] 既存 `docs/`（workspace-docking・ws-a・sv-gui・sf・pc-c5/c6）
  との重複・矛盾を調査し `evidence/sv1/issue-1-scope-map.md` に
  記録。
- [x] spec → design → plan → test-spec → task の文書チェーンを
  作成（`doc-driven-dev-graph` の宣言順。route_graph は既存の
  repo 全体 broken relation で fail-closed blocked のため、
  宣言済み delegate 順を手動で踏襲 — `docs/impl/exp/
  svt-product-e2e.md` と同じ既知の制約）。
- [x] `evidence/sv1/` に Acceptance Matrix 根拠・スコープ調査・
  検証記録を作成。
- [x] review-protocol によるレビュー（blocker 解消まで）。
- [x] docs index 再生成・`mise run fmt-check`・doc-status 系の
  軽量チェック。
- [x] impl/ir・impl/exp を完了状態に更新。

## Dependencies and Risks

- **依存:** なし（Blocked by なし）。下流の実装・実機受入は本
  計画の完了を待たず各 Owner Issue で進行する。
- **リスク:** 定義と受入の混同 → 証拠状態機械と NotRun 正直記録
  で対処（design §2・§9）。
- **リスク:** 既存 docs との用語不一致 → design §5 で製品語彙と
  PC-C3/SF-04 の実装語彙の対応を明示。

## Task Breakdown

単一タスク（`docs/tasks/product-value-core-task.md`）。文書
チェーンは直列依存（spec→design→plan→test-spec→task→impl）で、
並列化する独立ストリームはない。

## Verification

- [x] `mise run fmt-check`（docs/監査のみ。コード変更なし）—
  exit 0。結果は `evidence/sv1/verification.md`。
- [x] `node .agents/skills/doc-status/scripts/doc_status.js lint`
  / `audit` — 新規 docs 起因の error なし。spec の
  `traceability-missing-upstream`（warning）は Issue 起点 spec に
  共通の既存パターンとして記録（`evidence/sv1/verification.md`）。
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index plan|apply --dir <touched dir>` で index 再生成 — 全対象
  dir が `in-sync`。
- [x] review-protocol: Reviewer A（協調）/ Reviewer B（敵対）の
  独立パス → Validator → Judge。CONFIRMED P0/P1 なし。
- [x] 実 OS/RT/実 GUI 受入 — **本タスクの範囲外**。Owner Issue
  （#114/#109/#103 等）の証拠で別途判定（NotRun のまま）。

## Open Questions / Gaps

- UX 比較の数値閾値・比較対象 DAW 最終リスト・参加者条件 —
  Baseline 計測後に別途合意（Deferred）。
- 初期リリースでの B/C/AI 必須化 — 製品 Owner の別途決裁。

## Self-Review

- [x] spec の全要求（PV-01〜09）が design/plan/task/test-spec の
  いずれかに対応する。
- [x] プレースホルダなし。file path・relation・status・用語が
  一貫。
- [x] コード・CI・mise への変更を含まない。

## Implementation Handoff

- Recommended mode: inline execution（文書作業のため委譲せず
  本セッションで実施）。
- Independent work streams: none。
- User approval required before dispatch: yes — 本環境では
  review-protocol の blocker なしをもって承認とみなす
  （Worker 共通契約）。

## Review Handoff

- レビュアーは spec §11 Acceptance Matrix と design §6 Owner
  対応が Issue #1 の 6 項目・Owner 指定（#8/#23・#4/#108/#109・
  #6/#102/#103・#2/#3/#114）と一致するかを確認すること。
- 「現状の達成度」列が headless 証拠と NotRun を正直に区別して
  いるかを重点確認。
