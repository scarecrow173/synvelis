---
id: "EXP-rcd-f02-monitoring-overdub"
type: "experiment"
status: "completed"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — docs-only 設計 slice 実験・判断記録（synvelis#120）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/rcd-f02-monitoring-overdub.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
  defers: []
  deferred-by: []
---

# RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — docs-only 設計 slice 実験・判断記録（synvelis#120）

## E1 doc-driven-dev-graph router の状態

`route_graph.js --cwd . --json` を実行。`status: blocked`、
`current/next: probe`、`edgeId: null`。blocker は **既存の
repository-wide broken relation**（`docs/designs/automation-
modulation-design.md`・`rcd-*`・`tim-02-*`・`vcs-*`・
`docs/impl/ir/*`・`docs/impl/exp/*` の deferred relation が
存在しない path を指す）であり、本 slice が新たに作ったもの
ではない。`recording-take-comping`（sv#12）先例と同様、
この blocked は slice 起因でないため、router が返す delegate
が無いまま `spec → design → plan → task → test-spec → adr →
impl/ir + impl/exp` の文書チェーンを規約どおり作成した。
無関係な broken relation は **修復しない**（範囲外）。exp に
記録して後続の doc-status 修復 slice に委譲。

## E2 artifact ID の生成

scaffold script（`new_spec.js`/`new_design.js`/`new_plan.js`/
`new_task.js`/`new_test_spec.js`/`new_adr.js`/
`new_impl_record.js`）が生成した ID をそのまま採用:

- `SPEC-034XJxBZyIYAfzQFLeIAWA`（docs/specs/rcd-f02-monitoring-overdub.md）
- `DESIGN-034XJyqSj5ERrc0PHD2tDm`（docs/designs/rcd-f02-monitoring-overdub-design.md）
- `PLAN-*`/`TASK-*`/`TSPEC-*`/`ADR-034XJBlePY8LREvuEvfvkR`/`IMPL-*`（各 file 参照）
- `EXP-rcd-f02-monitoring-overdub`（本書。exp memo は手書き front matter — `recording-take-comping` 先例の簡易 schema に準拠）

## E3 5 領域の決定と選択肢の採否

Issue #120 が必須化する 5 領域について、design/ADR で以下を決定:

- **D1 UX Matrix**: `MonitorRoute::Overdub` variant 追加を採用。
  却下: route+flag 並列表現（`Muted`+overdub 等の不正組合せを
  型で排除できない）、route 集合型再設計（YAGNI・全面書換）。
- **D2 Feedback Risk**: 既存 `MonitorNotices` 3 variant 再利用 +
  `double_monitoring` 発火条件を input-leg 可聴（`Input`|`Overdub`）
  へ拡張を採用。却下: Overdub 専用 notice 追加（増分リスクは
  input leg 可聴一般に内在・既存 latency 報告で表現可能）。
- **D3 Capture Handle**: journal kind=4 `HandleGrant` record +
  既存 `JournalSpan` の effective_ranges 包含分類を採用。
  却下: `JournalSpan` material field（VERSION bump で v1 journal
  全 reject）、別 journal/media file（stream atomicity 破壊）、
  handle capture なし（Issue 要件不充足）。
- **D4 Undo/Recovery**: 既存 project undo + RCD-02 verified
  prefix に整合を採用。却下: handle 専用 undo/recovery 新設、
  take 同一 commit 単位。
- **D5 Privacy**: monitor PCM 非永続化 + session-scoped consent
  grant のみを採用。却下: project-level consent 永続化（privacy
  境界複雑化・YAGNI）、無期限 staging 保持。

## E4 docs-only slice の正直な記録

- 本 slice は **文書のみ**。`crates/**`・`Cargo.toml`/`Cargo.lock`・
  `mise.toml`・CI・`POLICIES.md`・`AGENTS.md` は一切変更していない。
- cargo test/build/lint・実 GUI/実 OS/実 Device/実 Plugin 検証は
  **実行していない**（POLICIES §5 比例原則 — docs-only は軽量
  チェックが原則）。これらは `NotRun`/`NotApplicable` として
  記録し、headless/design 証拠を実機 Pass に換算しない。
- 実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery・RT noalloc）
  は Issue #120 の実装 slice・#114 系受入 Gate の責務であり、
  本 slice では `Deferred`/`NotRun`。
- graph router の `blocked` は pre-existing であり、slice 起因の
  broken relation を新たに増やしていないことを確認した
  （全 local relation path の実在を scaffold/手動で検証）。
