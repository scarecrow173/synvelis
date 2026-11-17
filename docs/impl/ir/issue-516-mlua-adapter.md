---
id: "IMPLIR-issue-516-mlua-adapter"
type: "impl-ir"
status: "done"
title: "Issue #516 implementation record — 薄い binding への置換"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/tasks/issue-516-mlua-adapter.md"
    - "docs/plans/issue-516-mlua-adapter.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
      - type: "file"
        path: "docs/adr/issue-516-mlua-adapter.md"
      - type: "file"
        path: "evidence/sv516/binding-fit.md"
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
    - "docs/plans/issue-516-mlua-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-516-mlua-adapter.md"
    - "docs/tasks/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-516-mlua-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-516-mlua-adapter.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/exp/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #516 implementation record

## 実装したもの

`crates/synvelis-script-luau` を interim mlua 直実装から
anti-corruption adapter へ全面書換。内部で
`ruviform_script_luau::LuauBackend`（rev `84a68f25`）へ委譲し、
synvelis port 公開面を維持。

### 変更ファイル

- `crates/synvelis-script/src/backend.rs` —
  `ScriptErrorKind::BudgetExceeded.kind` を `Option<BudgetKind>`
  へ（中立 error が次元を運ばないため）。
- `crates/synvelis-script-luau/Cargo.toml` —
  `ruviform-script`/`ruviform-script-luau` git 依存追加・mlua/
  serde_json(通常)除去・serde_json を dev-dependency。
- `crates/synvelis-script-luau/src/lib.rs` — adapter 実装
  （descriptor 保持・open_session 事前検証・evaluate 委譲・
  全 error/value/artifact/digest 写像）。
- `crates/synvelis-script-luau/tests/backend.rs` — 製品契約挙動へ
  整合（record payload・`Some(kind)`・分類差）。
- `Cargo.lock` — ruviform rev `cbcc8343` → `84a68f25`
  （`ruviform-script-luau` が旧 pin に存在しないため必須）。

### adapter の責務

- descriptor は interim と同一値を保持（id=`luau-mlua`・
  enforced={WallClock,Memory,Output}・max_source=1MiB・
  artifact={WF,AR}・owner_thread_only・Safepoint）。
- open_session: language→size→digest(`sha256:` tag)→budgets
  (requested_kinds ∩ enforced)→artifact_kinds を synvelis 順序で
  先行検証 → `rf::SessionConfig`（host_bindings=empty・
  placement_hint=None・resume=None）へ変換 → 委譲。
- evaluate: closed→cancelled→evaluated の順で interim 分類を
  先行判定 → `inner.evaluate(None)` → 全結果を synvelis 語彙へ。
- artifact digest: 製品 `DefinitionDigest` は使わず synvelis
  canonical（`digest_of(to_json(payload))`）で再計算。
- `BudgetExceeded.kind`: enforced 集合が単一のときのみ `Some`、
  複数併用は `None`（推測しない）。

### 検証結果（実測）

- `mise exec -- cargo fmt --check` — clean。
- `mise exec -- cargo clippy -p synvelis-script-luau --all-targets
  -- -D warnings` — pass（3 件の lint 修正後）。
- `mise run test synvelis-script-luau` — 30 test pass・0 fail
  （real-VM）。
- `mise run test synvelis-automation` — 全 test pass（`.luau`
  real-VM resolver/eval 含む）。
- `mise run check-cli` — pass（lease 外 consumer 無変更）。
- `node tools/check-headless-deps.mjs` — pass（12 headless
  crates・GUI/GPU/WebView なし）。
- `cargo tree` — `synvelis-script-luau` → `ruviform-script`/
  `ruviform-script-luau`・mlua は ruviform-script-luau 経由のみ・
  synvelis crate は mlua を直接宣言しない。

### 記録済みの分類差（consumer 影響なし）

- lifted-shape 違反（sparse/mixed table・depth/node 上限）は
  製品側 `Internal` — interim の `ContractViolation` から分類差。
  `ContractViolation` は binding 検出違反と port 契約違反に限定。
- non-record artifact payload は製品契約で `Internal`（Record
  必須）— interim は scalar を受理していた。consumer は常に
  spec record を期待するため実害なし。

## 残件・NotRun

- 実 Luau.Analysis native FFI bridge（analyzer）— sv#274 gate。
- macOS/Linux 実機受入 — sv#267 gate。Windows `x86_64` のみ実測。
- host binding handler seam・named entry・Resume VM・instruction
  budget — 中立契約に存在しない（binding-fit 表 §9 に記録）。
- workspace 全体の ruviform pin 更新影響 — Daily Full CI へ委譲。
