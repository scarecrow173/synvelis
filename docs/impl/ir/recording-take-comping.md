---
id: "IMPL-C1ZTg7tLJ3bWNVepfbzpSs"
type: "impl"
status: "completed"
title: "Recording / Take / Comping — 文書チェーン・受入 Owner Matrix 実装記録（synvelis#12）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/12"
  changes:
    added:
      - type: "file"
        path: "docs/specs/recording-take-comping.md"
      - type: "file"
        path: "docs/designs/recording-take-comping-design.md"
      - type: "file"
        path: "docs/plans/recording-take-comping.md"
      - type: "file"
        path: "docs/tasks/recording-take-comping-task.md"
      - type: "file"
        path: "docs/test-specs/recording-take-comping.md"
      - type: "file"
        path: "docs/impl/ir/recording-take-comping.md"
      - type: "file"
        path: "docs/impl/exp/recording-take-comping.md"
      - type: "file"
        path: "evidence/sv12/owner-matrix.md"
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
  implements:
    - "docs/tasks/recording-take-comping-task.md"
    - "docs/plans/recording-take-comping.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/recording-take-comping-task.md"
    - "docs/test-specs/recording-take-comping.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/recording-take-comping.md"
    - "evidence/sv12/owner-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/recording-take-comping-task.md"
  derived-by: []
  verifies:
    - "docs/specs/recording-take-comping.md"
    - "docs/designs/recording-take-comping-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/12"
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Implementation Record: Recording / Take / Comping — 文書チェーン・Owner Matrix（synvelis#12）

## Summary

Issue #12 の基本アーキテクチャ合意を canonical 文書チェーン
（spec → design → plan → task → test-spec → impl/ir + impl/exp）と、
69 受入条件の Owner Matrix（`evidence/sv12/owner-matrix.md`）として
固定した **docs-only slice**。実装・実 OS・実 Device・実 Plugin の
受入は主張しない。実装 Owner は #115〜#118。

## Context

- 対象 Issue: synvelis#12（Recording/Take/Comping 基本契約・受入 Owner Matrix）。
- 前提: sv#117（Monitor/Punch headless fixture・PR#387）統合済み。
  実 Device Capture/同 Clock・Raw Fidelity は #115 が Owner。
- 本 slice の範囲: `docs/` の契約文書 + `evidence/sv12/` のみ。
  `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate は対象外。
- 先行 slice（sv#5/#6/#11）と同型の「設計定義のみ・実装は後続」
  docs slice として、headless fixture 成功と実機受入を分離して記録。

## Implementation

### Changes

- `docs/specs/recording-take-comping.md`: What/Why・AC-01〜69・
  Non-goals・Sources。69 受入条件を Issue #12 §13 から転記・固定。
- `docs/designs/recording-take-comping-design.md`: Owner 境界表・
  C1〜C7 基本契約（identity/lifecycle・frame/time・RT/handoff・
  writer/journal/recovery・monitor/latency・comp/editor・transaction）・
  概念状態モデル・Scenario/Oracle・Decision Log D-01〜D-08・検証戦略。
- `docs/plans/recording-take-comping.md`: docs-only scope・file map・
  dependencies・risks・verification matrix・downstream handoff・rollback。
- `docs/tasks/recording-take-comping-task.md`: T1〜T11 task 分解・
  完了条件・実装 Owner 表・ブランチ。
- `docs/test-specs/recording-take-comping.md`: F-CAPTURE/F-WRITER/
  F-MONITOR/F-RT/F-SESSION/F-COMP/F-RECOVERY-UI の Feature/Rule/
  Example/Guarantee・状態語彙・non-goals。
- `evidence/sv12/owner-matrix.md`: 69 AC × Owner Issue × fixture ×
  独立 oracle × OS Gate × 状態（Pass (headless) 44・Partial 22・
  Deferred 2・Pass (design) 1・NotRun 0・Unsupported 0）。
- `docs/impl/ir/recording-take-comping.md`（本書）・
  `docs/impl/exp/recording-take-comping.md`: 実施記録・graph route。
- 触れた dir の管理 index（`docs/*/README.md`）を doc-maintenance
  `index apply` で再生成。

### Decisions During Implementation

- AC-01〜69 を Issue #12 §13 の受入条件順に採番し、spec・owner-matrix・
  test-spec で同一 ID を維持（追跡性のため）。
- 状態分類は `Pass (headless)`/`Partial`/`NotRun`/`Deferred`/
  `Unsupported`/`Pass (design)` を使用。headless/unit 成功を実機・
  GUI・実 Device・実 Plugin Pass に換算しない（POLICIES.md 準拠）。
- AC-28/AC-29（Take Lane/Comp Editor UI）は UI 未実装のため `Deferred`
  （#114/#118 GUI Gate）。AC-69（API 適合チェック process AC）は本
  chain が証拠のため `Pass (design)`。
- graph route は `blocked`（既存 repository-wide broken relation、
  本 slice 起因でない）— sv#5/#6/#11 先例と同型。exp に記録。

## Related Experiments

- `docs/impl/exp/recording-take-comping.md`（graph route・ID 生成・
  判断記録）。adopted/rejected は空 — exp は narrative 記録であり
  採用/却下の分岐は発生しなかった。

## Validation

- `mise run fmt-check`: docs slice の format 検査（実施結果は PR 本文に記録）。
- doc-maintenance `index plan`/`index apply --dir <touched>`: 触れた
  dir（specs/designs/plans/tasks/test-specs/impl-ir/impl-exp）の管理
  index を再生成し `in-sync` を確認。
- `sync_impl_index.js --kind ir|exp`: impl index の preview→apply。
- doc-status audit: 新規 broken relation がないことを確認（既存の
  repository-wide broken relation は slice 起因でなく範囲外）。
- Rust build/test/lint: docs-only のため `NotRun`/`NotApplicable`。

## Risks

- headless 成功を実機 Pass と誤記するリスク → owner-matrix で状態を
  厳密分離し、`Pass (headless)`/`Deferred` を明示。
- 69 AC の Owner 対応付け漏れ → spec・matrix・test-spec で同一 ID・
  集計件数照合（69 件一致）。
- 既存 broken relation で graph route blocked → slice 起因でないことを
  exp/PR に記録し、無関係な broken relation は修復しない（範囲外）。

## Follow-ups

- 実装 Owner（#115〜#118）が `evidence/sv12/owner-matrix.md` の各 AC
  から自分の Issue・fixture・OS Gate を参照して実装・検証する。
- 実機・実 OS GUI・実 Device・実 Plugin・実 loopback・実 power-loss
  の受入 → #114・EXT-L3・ruviform capability Issues。
- literal 8 track・literal tempo fixture・Grouped Comp 一括 command・
  Take Lane/Comp Editor UI → #115〜#118・#114・#12 §14/§15 後続。
- Independent Device Clock → #119（RCD-F01）。Creative Branch →
  #278/#279/#288。
- Issue #12 の close は Coordinator 判断（設計合意の記録であり実装
  完了ではない）。
