---
id: "PLAN-sv517gH8iJ0kL1mN3oP5qR7"
type: "plan"
status: "approved"
title: "Issue #517 plan — render/branch pipeline 本番配線"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/issue-517-render-branch-wiring.md"
    - "docs/designs/issue-517-render-branch-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-517-render-branch-wiring.md"
  derived-by:
    - "docs/tasks/issue-517-render-branch-wiring.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #517 plan

## スライス分割

| # | スライス | 主要ファイル | 依存 |
| --- | --- | --- | --- |
| S1 | 永続 BranchContentStore | `synvelis-project/src/variant/store.rs`, `variant/mod.rs` | — |
| S2 | 本番 RuntimePublish | `synvelis-project/src/variant/runtime.rs`（新規）, `variant/mod.rs` | S1 と独立 |
| S3 | VariantPorts::standard 配線 + consumer 波及 | `synvelis-gateway/src/variant.rs`, `synvelis-cli/src/host.rs`, `mcp-poc/src/main.rs` | S1, S2 |
| S4 | synvelis.render.* 登録 | `synvelis-gateway/src/variant.rs`, `prepare_engine.rs`, `prepare.rs`, `commit.rs` | S3 |
| S5 | VCS-RESTORE-10 fork-copy | `synvelis-project/src/restore/mod.rs`, `lib.rs` | S1 |
| S6 | テスト | `synvelis-project/tests/sv517_render_wiring.rs`, `synvelis-gateway/tests/sv517_render_actions.rs`, 既存 GateDeps literal 更新 | S1-S5 |
| S7 | doc chain + evidence + 検証 | `docs/**`, `evidence/sv517/`, ledger | 全て |

## リスクと対応

- `VariantPorts::standard()` シグネチャ変更 → 全呼出点を grep で
  網羅更新（fixture 含む）。
- `GateDeps` への `render` field 追加 → 全 literal 更新（production
  2 + test 13+）。
- restore receipt schema 追加（`branch_content` field,
  `#[serde(default)]`）→ 既存 receipt 読み取りとの後方互換を
  default で確保。
- render op が project state を変更しないこと → commit.rs で
  candidate 不変を pin、provenance に dispatch evidence。

## 検証マトリクス

| 変更 crate | 必須タスク |
| --- | --- |
| synvelis-project | check/lint/test synvelis-project |
| synvelis-gateway | check/lint/test synvelis-gateway |
| synvelis-cli | check/lint/test synvelis-cli |
| mcp-poc | check/lint/test mcp-poc |
| synvelis-workspace | check/lint/test（render_ui 語彙照合 — 変更なし想定） |
| 全体 | fmt-check, prek run --all-files |
