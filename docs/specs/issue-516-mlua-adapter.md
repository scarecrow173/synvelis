---
id: "SPEC-Nc4BLBrVpI3wgQ9T7hI5SE"
type: "spec"
status: "approved"
title: "Issue #516 spec — interim mlua adapter → ruviform 製品 port 交換の受入仕様"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/discovery/issue-516-mlua-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/adr/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-script-03-luau-backend.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-516-mlua-adapter.md"
  derived-by:
    - "docs/adr/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-516-mlua-adapter.md"
  references:
    - "evidence/sv516/binding-fit.md"
    - "docs/adr/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #516 spec — 製品 port 交換の受入仕様

## 目的

interim `synvelis-script-luau`（mlua 直実装）を ruviform 製品
backend（`ruviform-script`/`ruviform-script-luau`）への薄い binding
へ置換し、重複 VM 実装を解消する。consumer 公開面
（`synvelis_script::backend` port）は維持する。

## 機能要件

- FR-1: `synvelis-script-luau` は `ruviform_script_luau::LuauBackend`
  を内部に保持し、`synvelis_script::backend` の `ScriptBackend`/
  `ScriptSession` を実装する adapter として動作する。
- FR-2: `open_session` は interim と同一の fail-closed 検証
  （language→size→digest→budgets→artifact_kinds）を先行実施し、
  製品 backend へ委譲する。`SessionError` の variant・優先順位は
  interim と同一に保つ。
- FR-3: `evaluate` は product `evaluate(None)` へ委譲し、
  `ScriptResult` を synvelis 語彙へ変換する。2 回目 evaluate は
  `ContractViolation`、close 後は `Backend`、cancel は `Cancelled`。
- FR-4: artifact は synvelis canonical（`digest_of(to_json(payload))`）
  で digest を再計算し、interim と同一の provenance 形状
  （`ArtifactSource`/`ProducedBy`）を維持する。
- FR-5: `BudgetExceeded.kind` は単一 armed 予算次元のみ `Some`、
  それ以外 `None`（port 語彙を `Option<BudgetKind>` へ拡張）。
- FR-6: `max_instructions`/`max_host_calls` は
  `UnsupportedBudget` で拒否（interim と同一）。
- FR-7: consumer（`synvelis-cli`/`synvelis-automation`）のコード・
  公開 API は変更しない。
- FR-8: mlua は `ruviform-script-luau` 経由のみ — synvelis 側
  crate は mlua を直接依存しない。

## 非機能要件

- NFR-1: headless 境界維持 — `check-headless-deps` で GUI/GPU/
  WebView が headless crate に混入しないこと。
- NFR-2: `unsafe_code = forbid` — synvelis 側コードに unsafe を
  書かない（ruviform 側 ffi module は上流の局所許可）。
- NFR-3: fail-closed — unsupported budget・digest drift・言語
  不一致・artifact 不一致・単一 evaluate は全て typed error。

## 受入条件（Issue #516 AC 対応）

- AC1: 交換実施の決定・記録 → `docs/adr/issue-516-mlua-adapter.md`
  （Option C 採用）。
- AC2: consumer が port trait 経由で backend を消費し既存契約に
  回帰なし → `mise run test synvelis-script-luau`・
  `test synvelis-automation`・`check-cli`/`test-cli` green。
- AC3: 実 Luau.Analysis native FFI 走行は sv#274 gate として
  NotRun 記録（headless Pass を native Pass に昇格しない）。
- AC4: `check`/`lint`/`test`（変更 crate + consumer）が pass。

## 対象外

- analyzer（`LuauAnalyzer`）切替 — sv#274。
- host binding handler seam・named entry・Resume VM・instruction
  budget — 中立契約に存在しない。
- `synvelis-cli`/`synvelis-automation` src・ルート `Cargo.toml`
  （lease 外）。
- ActionRecipe/workflow/schema/model — port 適合と無関係な変更
  禁止（Issue 指定）。
- 3-OS 実機受入 — NotRun。
