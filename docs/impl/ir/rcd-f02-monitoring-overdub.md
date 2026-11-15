---
id: "IMPL-034XJDNpQ0olm26HvRkxuW"
type: "impl"
status: "completed"
title: "RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — 実施記録（synvelis#120 docs-only slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/120"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/designs/rcd-f02-monitoring-overdub-design.md"
      - type: "file"
        path: "docs/plans/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/tasks/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/test-specs/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/adr/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/impl/ir/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "docs/impl/exp/rcd-f02-monitoring-overdub.md"
      - type: "file"
        path: "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
      - type: "file"
        path: "evidence/sv120/rcd-f02-monitoring-overdub-verification.md"
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
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
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
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
    - "docs/test-specs/rcd-f02-monitoring-overdub.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/rcd-f02-monitoring-overdub.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md"
    - "evidence/sv120/rcd-f02-monitoring-overdub-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/rcd-f02-monitoring-overdub.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
    - "docs/adr/rcd-f02-monitoring-overdub.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/120"
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Implementation Record: RCD-F02 Advanced Monitoring / Overdub / Pre-Punch Handles — docs-only slice（synvelis#120）

## Summary

Issue #120 が必須化する ADR 合意（UX Matrix・Feedback Risk・Capture Handle Retention・Undo/Recovery・Privacy）を canonical 文書チェーン（spec → design → plan → task → test-spec → adr → impl/ir + impl/exp）と `evidence/sv120/` の決定・検証記録として固定した **docs-only slice**。実装・実 OS・実 Device・実 Plugin の受入は主張しない。実装は ADR merge 後の後続 slice が Owner。

## Context

- 対象 Issue: synvelis#120（future/recording RCD-F02 tracker — Advanced Monitoring / Overdub Layering / Pre-Punch Handles）。
- 前提: RCD-03（#117）monitor/punch/latency・RCD-02（#116）durability/recovery・take/comping（#12）の baseline 契約は確定済み。`rcd-f0*` docs は未存在であり新規 slug `rcd-f02-monitoring-overdub` を採用。
- 本 slice の範囲: `docs/` の契約文書 + `evidence/sv120/` のみ。`crates/**`・`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/`POLICIES.md`/`AGENTS.md` は対象外。
- 先行 slice（sv#12 recording-take-comping）と同型の「設計定義のみ・実装は後続」docs slice として、headless/design 証拠と実機受入を分離して記録。

## Implementation

### Changes

- `docs/specs/rcd-f02-monitoring-overdub.md`: What/Why・REQ-F02-01〜07・AC-F02-01〜10・Deferred Design Concerns・Sources。
- `docs/designs/rcd-f02-monitoring-overdub-design.md`: 既存契約 inventory（`monitor.rs`/`gate.rs`/`journal.rs`/`monitoring.rs`/`monitor.rs`/`monitor_engine.rs`）・UX Matrix（baseline 転記 + Overdub 次元）・notice semantics・HandleGrant/retention・Undo/Recovery・Privacy・検討して却下した選択肢・Verification Notes。
- `docs/plans/rcd-f02-monitoring-overdub.md`: docs-only scope・file map・dependencies・risks・verification matrix・downstream handoff・rollback。
- `docs/tasks/rcd-f02-monitoring-overdub.md`: T1〜T12 task 分解・完了条件・後続実装 Owner 表・ブランチ。
- `docs/test-specs/rcd-f02-monitoring-overdub.md`: F-ROUTE/F-NOTICE/F-HANDLE/F-RECOVERY/F-PRIVACY の Feature/Rule/Example/Guarantee・状態語彙・non-goals・risk。
- `docs/adr/rcd-f02-monitoring-overdub.md`: **納品本体** — MADR 4.0.0 形式で 5 領域の決定（D1 `MonitorRoute::Overdub`・D2 notice 再利用+発火条件拡張・D3 kind=4 `HandleGrant`+`JournalSpan` 分類・D4 既存 undo/verified prefix・D5 consent なし永続化禁止）・実装計画・検証・pros/cons。
- `docs/impl/ir/rcd-f02-monitoring-overdub.md`（本書）・`docs/impl/exp/rcd-f02-monitoring-overdub.md`: 実施記録・graph route・ID 生成・決定と選択肢。
- `evidence/sv120/rcd-f02-monitoring-overdub-adr-decisions.md`: 5 領域の決定要約 + 採用/却下選択肢。
- `evidence/sv120/rcd-f02-monitoring-overdub-verification.md`: 実行した検証の記録。
- 触れた dir の管理 index（`docs/*/README.md`・`docs/impl/{ir,exp}/README.md`）を doc-maintenance `index apply`・`sync_impl_index` で再生成。

### Decisions During Implementation

- 5 領域の決定は ADR D1〜D5 として固定。既存 `decide`/`GateScheduler`/`evaluate_gate`/`JournalSpan`/`MonitorAdapter`/project undo/RCD-02 recovery の権威を第二正本化せず拡張。
- UX Matrix は `decide` 既存判定表（monitor.rs L177-186）を baseline 転記し、Overdub 次元を `MonitorRoute::Overdub` variant 追加で表現（route+flag 並列表現・集合型再設計は却下）。
- Feedback Risk は既存 `MonitorNotices` 3 variant を正本再利用し、`double_monitoring` 発火条件を input-leg 可聴（`Input`|`Overdub`）へ拡張。新 notice variant は追加しない。
- Capture Handle は journal kind=4 `HandleGrant` record + 既存 `JournalSpan` の effective_ranges 包含分類。`JournalSpan` material field（VERSION bump）・別 journal/media file・handle capture なしは却下。
- Undo/Recovery は既存 project undo + RCD-02 verified prefix に整合。handle 専用 undo/recovery 新設・take 同一 commit 単位は却下。
- Privacy は monitor PCM 非永続化 + session-scoped consent grant のみ。project-level consent 永続化・無期限 staging 保持は却下（YAGNI/privacy 境界）。
- 状態分類は `Pass (design)`/`Pass (headless)`/`NotRun`/`Deferred`/`Unsupported` を使用。headless/design 証拠を実機 Pass に換算しない（POLICIES.md 準拠）。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-f02-monitoring-overdub.md` — graph route blocked（pre-existing）・ID 生成・5 領域の決定と選択肢の記録。

### Rejected

- なし（exp は本 slice の判断記録であり、採用/却下の experiment log 併合は無い）。

## Validation

- `mise run fmt-check`・`mise exec -- prek run --all-files`・doc index `index plan`/`apply`・`sync_impl_index`・doc-status audit・type 別 lint・impl audit・ADR audit を実行（結果は `evidence/sv120/rcd-f02-monitoring-overdub-verification.md` に記録）。
- cargo test/build/lint・実 GUI/実 OS/実 Device/実 Plugin 検証は **実行していない**（docs-only・POLICIES §5 比例原則）。`NotRun`/`NotApplicable`。
- graph router `route_graph.js --cwd . --json` は `status: blocked`（pre-existing broken relation、slice 起因でない）。

## Risks

- `MonitorRoute::Overdub` variant 追加・notice 発火条件拡張・journal kind=4 追加が既存 consumer に影響 → compile error で検出可能・実装 slice で exhaustive match 更新。
- handle/take 分類の曖昧さ → effective ranges の構造的 disjoint を plan 凍結時に保証。
- kind=4 journal の旧 reader 互換 → fail-closed（prefix 切り詰め）を仕様として明記。
- handle capacity の具体的既定値は実装 slice の fixture で確定（本 slice は必須属性契約のみ固定）。

## Follow-ups

- 実装 slice（ADR merge 後・直列 dispatch）: `synvelis-recording` Overdub/Explicit Pre-Punch Handle/Take Layer Policy・`synvelis-player` Monitor Cue・`synvelis-app` Consent/Warning UX・Fixture。
- 実 OS E2E（Loop/Take/Comp/Device 切断/Undo/Recovery・RT noalloc）: Issue #120 実装 slice・#114 系受入 Gate。
- Coordinator: 本 chain merge 後に実装 slice を dispatch。Issue #120 close は Coordinator/Owner 判断。
