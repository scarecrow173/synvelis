---
id: "IMPL-mFCaw7vxrI5DaWd03Ze4Il"
type: "impl"
status: "completed"
title: "issue-516-mlua-adapter"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
      - type: "file"
        path: "docs/adr/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/designs/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/discovery/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/impl/exp/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/impl/exp/issue-516-mlua-adapter-exp.jsonl"
      - type: "file"
        path: "docs/impl/ir/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/plans/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/specs/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/tasks/issue-516-mlua-adapter.md"
      - type: "file"
        path: "docs/test-specs/issue-516-mlua-adapter.md"
      - type: "file"
        path: "evidence/sv516/binding-fit.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-script-luau/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script-luau/tests/backend.rs"
      - type: "file"
        path: "crates/synvelis-script/src/backend.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/issue-516-mlua-adapter.md"
    - "docs/designs/issue-516-mlua-adapter.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-516-mlua-adapter.md"
    - "docs/impl/ir/sv-script-03-luau-backend.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/issue-516-mlua-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv516/binding-fit.md"
    - "docs/adr/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-516-mlua-adapter-exp.jsonl"
    rejected: []
---

# issue-516-mlua-adapter

## Summary

`synvelis-script-luau` を interim mlua 直実装から anti-corruption
adapter へ全面書換し、`ruviform_script_luau::LuauBackend`
（RF-SCRIPT-00 製品 backend・ruviform rev `84a68f25`）へ委譲する
構成へ置換した。synvelis port 公開面
（`synvelis_script::backend`）は維持し、consumer
（`synvelis-cli`/`synvelis-automation`）は無変更で動作する。
決定は `docs/adr/issue-516-mlua-adapter.md`（Option C 採用）、
語彙差分の網羅は `evidence/sv516/binding-fit.md` を正本とする。

## Context

`docs/tasks/issue-516-mlua-adapter.md` / plan /
`docs/designs/issue-516-mlua-adapter.md` に従う。
spec `docs/specs/issue-516-mlua-adapter.md` の FR-1〜8 を実装した。
interim 実装（synvelis#273・PR #505）は sandbox globals・lift
規則・budget・digest・artifact provenance を synvelis 側に複写
しており、Ruviform 側に同責務の製品 backend が着地したため
重複を解消する。

## Implementation

- `crates/synvelis-script/src/backend.rs` —
  `ScriptErrorKind::BudgetExceeded.kind` を `Option<BudgetKind>`
  へ拡張。中立 `ScriptError` が次元情報を運ばず複数 budget
  併用時に帰属不可能なため、単一 armed 次元のみ `Some`・
  それ以外は `None` で「不明」を正直に返す。
- `crates/synvelis-script-luau/Cargo.toml` —
  `ruviform-script`/`ruviform-script-luau` を既定ブランチ git
  依存で追加（crate ローカル宣言）。`mlua`/`serde_json`（通常
  依存）を除去し `serde_json` は dev-dependency へ。
- `crates/synvelis-script-luau/src/lib.rs` — adapter 実装
  （約 650 行）。synvelis descriptor 保持・open_session の
  fail-closed 事前検証（language→size→digest→budgets→
  artifact、interim 順序）・`rf::SessionConfig` 変換
  （`host_bindings=empty()`・`placement_hint=None`・
  `resume=None`）・evaluate 委譲（`evaluate(None)`）・
  error/value/artifact/digest の全写像・evaluate-once
  `ContractViolation`・close 後 `Backend`・cancel `Cancelled` の
  interim 分類維持。
- `crates/synvelis-script-luau/tests/backend.rs` — 製品契約挙動へ
  整合（artifact payload は Record 必須・`BudgetExceeded` は
  `Some`/`None` パターン・lifted-shape 違反は `Internal` 分類）。
- `Cargo.lock` — ruviform 解決 rev `cbcc8343` → `84a68f25`
  （`ruviform-script-luau` が旧 pin に存在しないため必須）。

### adapter が引き受けた差分（要点）

- source digest: consumer の `sha256:<hex>` tag を open 時に検証。
  製品側 `sha256:v1:` digest は `rf::ScriptSource::new` が自動計算。
- artifact digest: 製品 `DefinitionDigest` は使わず synvelis
  canonical `digest_of(to_json(payload))` で再計算（interim と
  同一式）。`produced_by.impl_version` は実際に VM を走らせた
  製品 backend の descriptor 値を記録。
- `max_instructions`/`max_host_calls` は `UnsupportedBudget` で
  拒否（製品は host call 経路がなく vacuous 受理するため adapter
  が port 契約「要求次元は enforce 必須」を保持）。
- lifted-shape 違反（sparse/mixed table・depth/node 上限・
  non-record artifact payload）は製品側 `Internal` — interim の
  `ContractViolation` から分類差。`ContractViolation` は binding
  検出違反（domain 修飾値・未知 variant）と port 契約違反に限定。

## Related Experiments

`docs/impl/exp/issue-516-mlua-adapter.md`（event log:
`docs/impl/exp/issue-516-mlua-adapter-exp.jsonl`）。
E1 直接交換不可の確認・E2 ruviform pin 更新の必然性・E3 artifact
Record 必須の分類差・E4 `BudgetExceeded.kind` 帰属・E5 clippy
修正を記録。採用: 薄い binding 分岐（E1 の結論）。棄却: 直接
crate 交換・port 語彙の再 binding。

## Validation

全て実測（2026-11-17・Windows `x86_64`）:

- `mise run fmt-check` — clean。
- `mise run check synvelis-script` / `check synvelis-script-luau` /
  `check synvelis-automation` — pass。
- `mise run lint synvelis-script` / `lint synvelis-script-luau` /
  `lint synvelis-automation` — pass（`-D warnings`）。
- `mise run test synvelis-script` — pass。
- `mise run test synvelis-script-luau` — 30 test pass・0 fail
  （real-VM: descriptor・言語/digest/budget fail-closed・artifact・
  sandbox globals・session isolation・evaluate-once・cancel）。
- `mise run test synvelis-automation` — 全 test pass（`.luau`
  real-VM resolver/eval・mixed-source run 含む）。
- `mise run check-cli` / `lint-cli` / `test-cli` — pass
  （lease 外 consumer 無変更）。
- `mise run check-headless-deps` — pass（12 headless crates・
  GUI/GPU/WebView なし）。
- `mise exec -- prek run --all-files` — pass。
- `cargo tree` — mlua は `ruviform-script-luau` 経由のみ、
  synvelis crate は mlua を直接宣言しない。

## Risks

- `Cargo.lock` の ruviform rev 更新（`cbcc8343` → `84a68f25`）が
  workspace 全 crate に影響 — Daily Full CI の検証に委譲、
  PR 本文に明記済み。
- 製品 backend の error 分類差（`Internal` vs `ContractViolation`）
  — consumer は区別していないことを確認済み、binding-fit 表
  §6 に記録。
- `ruviform-script-luau` の build.rs が vendored Luau.Analysis
  C++ を静的コンパイル — build 時間増は許容（headless 維持）。

## Follow-ups

- 実 Luau.Analysis native FFI bridge（analyzer）切替 — sv#274
  gate。本 slice は runtime port のみ。
- host binding handler seam・named entry・Resume VM・instruction
  budget — 中立契約に存在しない。追加時は binding の写像層を
  再評価（binding-fit 表 §9・§10 に記録）。
- macOS/Linux 実機受入 — sv#267 gate。Windows `x86_64` のみ実測、
  NotRun を維持。
- workspace 全体の ruviform pin 更新影響 — Daily Full CI。
