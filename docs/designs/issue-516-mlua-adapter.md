---
id: "DESIGN-issue-516-mlua-adapter"
type: "design"
status: "approved"
title: "Issue #516 設計 — synvelis-script-luau を ruviform 製品 port への薄い binding へ置換"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/specs/issue-516-mlua-adapter.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-script/src/backend.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script-luau/tests/backend.rs"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-516-mlua-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-516-mlua-adapter.md"
    - "docs/discovery/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-script-03-luau-backend.md"
  refines: []
  refined-by:
    - "docs/plans/issue-516-mlua-adapter.md"
  derives-from:
    - "docs/specs/issue-516-mlua-adapter.md"
  derived-by:
    - "docs/plans/issue-516-mlua-adapter.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-516-mlua-adapter.md"
  references:
    - "evidence/sv516/binding-fit.md"
    - "docs/adr/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #516 設計 — 薄い binding への置換

## Overview

`synvelis-script-luau` を VM 実装 crate から anti-corruption
adapter へ転換する。公開面（`synvelis_script::backend` port の
`LuauBackend`/`ScriptSession` 実装）は interim と同一を維持し、
内部で `ruviform_script_luau::LuauBackend`（製品 backend）へ
委譲する。consumer（`synvelis-cli`/`synvelis-automation`）は
無変更。

## 構造

```
synvelis-cli / synvelis-automation
        │  synvelis_script::backend traits (不変)
        ▼
synvelis-script-luau  ── adapter ──▶ ruviform-script-luau::LuauBackend
   (語彙写像・fail-closed 事前検証)        │ mlua + vendored Luau 0.740
        │                                  │ (+ vendored Luau.Analysis shim)
        ▼                                  ▼
   synvelis port 型 ←────────── ruviform-script 中立契約型
```

- `LuauBackend { descriptor: synvelis BackendDescriptor, product:
  ruviform_script_luau::LuauBackend }` — descriptor は interim と
  同一値を保持（id=`luau-mlua`、enforced={WallClock,Memory,Output}、
  max_source=1MiB、artifact={WF,AR}、owner_thread_only、Safepoint）。
- `open_session(&SessionConfig)`: language→size→digest(`sha256:`
  tag)→budgets(requested_kinds ∩ enforced)→artifact_kinds の順で
  synvelis `SessionError` を先行検証 → `rf::SessionConfig` へ変換
  （`host_bindings=empty()`・`placement_hint=None`・`resume=None`・
  budget 1:1 転記・artifact kind 翻訳）→ `product.open_session` へ
  委譲 → `session_error_from_rf` で error 写像。
- `LuauSession`: `evaluated`/`closed` flag・enforced budget 集合・
  source provenance・product `impl_version` を保持。
  `evaluate()` は closed→cancelled(token)→evaluated の順で
  interim 分類（`Backend`/`Cancelled`/`ContractViolation`）を先行
  判定してから `inner.evaluate(None)` → `result_from_rf`。
- `BudgetExceeded.kind`: `attribute_budget()` が enforced 集合が
  単一のときのみ `Some(kind)`、それ以外 `None`。

## port 語彙の変更（最小）

- `ScriptErrorKind::BudgetExceeded { kind: Option<BudgetKind> }`
  （`backend.rs`）— 中立 error が次元を運ばないため。consumer は
  `kind` を直接 match していないため影響なし。

## 依存

- `crates/synvelis-script-luau/Cargo.toml`:
  `ruviform-script` + `ruviform-script-luau` を既定ブランチ git
  依存で追加（crate ローカル宣言 — ルート `[workspace.dependencies]`
  は lease 外）。`mlua`/`serde_json`(通常依存)を除去、
  `serde_json` は dev-dependency へ。
- `Cargo.lock`: ruviform 解決 rev が `cbcc8343` → `84a68f25`
  （`ruviform-script-luau` が旧 pin に存在しないため必須）。

## fail-closed 対応表（interim → 製品経路）

| 検証 | interim | adapter + 製品 |
| --- | --- | --- |
| 言語不一致 | `LanguageMismatch` | adapter 先行 → 同 variant |
| source >1MiB | `SourceTooLarge` | adapter 先行 → 同 variant |
| digest drift/未知 tag | `SourceDigestMismatch` | adapter 先行（`sha256:` tag のみ検証）→ 同 variant |
| instruction/host-call budget | `UnsupportedBudget` | adapter 先行（enforced 集合外を拒否）→ 同 variant |
| memory=0/overflow・wall overflow | `UnsupportedBudget` | 製品 `validate_budget` → `session_error_from_rf` で同 variant |
| artifact 未広告/複数 | `UnsupportedArtifact`/`Backend` | adapter 先行 → 同 variant |
| evaluate 2 回目 | `ContractViolation` | adapter 先行 → 同 variant |
| close 後 evaluate | `Backend` | adapter 先行 → 同 variant |
| multi-return/sparse/mixed/depth | `ContractViolation` | 製品 `Internal` → `Internal`（分類差・記録済み） |
| 非有限数（record 内） | `ValueOutOfRange` | adapter 変換時 → 同 variant |
| cancel | `Cancelled` | 製品 token → `Cancelled` |
| wall/memory/output budget 超過 | `BudgetExceeded{Some(kind)}` | 製品 → `BudgetExceeded{kind: attribute()}`（単一時 `Some`、複数時 `None`） |

## 非目標

- analyzer（`LuauAnalyzer` native bridge）切替 — sv#274。
- host binding handler seam・named entry・Resume VM・instruction
  budget — 中立契約に存在しない（binding-fit 表 §9）。
- consumer（cli/automation）・registry・ActionRecipe/workflow
  モデルの変更。
- 3-OS 実機受入 — Windows `x86_64` のみ（NotRun 記録）。
