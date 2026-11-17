---
id: "DISC-issue-516-mlua-adapter"
type: "discovery"
status: "complete"
title: "Issue #516 discovery — interim mlua adapter vs ruviform 製品 port の binding-fit 調査"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
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
  related:
    - "docs/adr/issue-516-mlua-adapter.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-516-mlua-adapter.md"
  verifies: []
  verified-by: []
  references:
    - "evidence/sv516/binding-fit.md"
  defers: []
  deferred-by: []
---

# Issue #516 discovery — interim mlua adapter vs ruviform 製品 port

## Issue framing

Synvelis#273 の interim `synvelis-script-luau`（mlua 直実装）が、
Ruviform RF-SCRIPT-00 製品 port（`ruviform-script` 中立契約 +
`ruviform-script-luau` runtime backend、`scarecrow173/ruviform#729`
で main 着地）とどう適合するかを照合し、交換 or 恒久採用を確定
する。本 discovery は照合結果と判定材料を記録する。

## 調査結果（実測）

### 両実装の構造的対応

- 両者とも vendored Luau 0.740 + mlua 0.12.2 safe Rust API。
- session 毎 fresh `Lua`・`shared_vm_state=false`・owner-thread-only
  （mlua `send` 無効）・single-shot evaluate・host binding なし・
  sandbox globals 4 種剥奪（`require`/`loadstring`/`print`/`debug`）・
  lift 上限（depth 64 / nodes 1M）・pcall 飲み込み対策 — 高レベル
  振る舞いはほぼ一致。
- 差異は全て**契約語彙の形状**に集中: trait 署名・error enum・
  digest 形式・SessionConfig 追加 field・artifact payload 規則・
  capabilities 語彙。実行意味論の差は小さい。

### 直接交換の可否

**不可** — `open_session(&SessionConfig)` vs `open_session(SessionConfig)`、
`evaluate()` vs `evaluate(Option<&str>)`、`ScriptErrorKind` の payload
有無、`SessionConfig` の `host_bindings`/`placement_hint`/`resume`、
source digest `sha256:` vs `sha256:v1:`、artifact payload
`ScriptValue` vs `BTreeMap`（Record 必須）等、公開面が全面的に異なり
crate 単位の drop-in は成立しない。

### 薄い binding の成立性

**成立** — synvelis port 公開面を維持する adapter で全差分を吸収
可能。consumer（`synvelis-cli`/`synvelis-automation`）は
`synvelis_script::backend` 経由のみで `LuauBackend` を消費しており、
公開面を壊さなければ変更不要。`BudgetExceeded.kind` のみ
`Option<BudgetKind>` への port 語彙調整が必要（中立 error が次元を
運ばないため）。

### ruviform 側の不足（consumer 要件に対して）

- host binding handler seam なし（記述子のみ）— Synvelis は
  `SessionConfig` に host surface がなく現行要件と一致。
- named entry points 非対応 — Synvelis は未使用。
- `max_instructions` 非対応 — 両者とも拒否で一致。
- `max_host_calls` は vacuous 受理 — synvelis port は「要求次元は
  enforce 必須」なので adapter が事前拒否で保持。
- `BudgetExceeded` に次元 payload なし — `Option<BudgetKind>` で
  「不明」を正直に返す。
- analyzer は `LuauAnalyzer`（native bridge）が存在するが sv#274
  の gate — 本 slice 対象外。

### 依存・build 影響

- `ruviform-script-luau` は build.rs で vendored Luau.Analysis
  C++ を静的コンパイル（cc + find-msvc-tools）— mlua だけの interim
  より build が重いが headless（GUI/GPU/WebView なし）。
- workspace ruviform pin は `cbcc8343` → `84a68f25` へ更新必須
  （旧 pin に `ruviform-script-luau` crate が存在しない）。

## Gap analysis / verdict

- (a) interim 恒久採用: 重複実装を残す — 製品 port 存在下では不適。
- (b) port 語彙の再 binding: consumer 破壊 + 重複残存 — 最悪。
- (c) **薄い binding 交換: 採用** — 重複解消・consumer 無変更・
  fail-closed 維持を全て満たす。

詳細な語彙対応表は `evidence/sv516/binding-fit.md` を正本とする。
