---
id: "DISC-4qXzyQzBwS2yIrMLH0Ra"
type: "discovery"
status: "resolved"
title: "Issue #530 discovery — stateless plugin を PluginStateDecl で表現する契約調査"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "https://github.com/scarecrow173/synvelis/issues/293"
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
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/adr/issue-530-plugin-state-decl.md"
    - "evidence/sv530/decision-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-530-plugin-state-decl.md"
  verifies: []
  verified-by: []
  references:
    - "evidence/sv514/decision-ledger.md"
    - "docs/impl/exp/issue-514-session-engine-selection-exp.jsonl"
---

# Issue #530 discovery — stateless plugin を PluginStateDecl で表現する契約調査

## 背景

Synvelis#514 の decision-ledger AC4 で「stateless `PluginStateDecl`
の型表現には player 契約変更が必要」と確定し、本 Issue #530 に帰属
された。RCP-04 semantic→selection 統合（Synvelis#293）では、plugin
node の live 判定が `verify_plugin` に委ねられるが、state save/restore
を実装しない stateless plugin は現契約では `Live` に到達できない。

## 調査結果（main `07b67f5b` で再確認）

### 現行契約の所在

- `crates/synvelis-player/src/rcp/model.rs`
  - `PluginStateDecl { digest: Option<String>, quality:
    StateCaptureQuality, status: OpaqueStateStatus }` — `digest: None`
    は「state blob が存在しない」を意味するが、stateless by design と
    未 capture を型上区別できない。
  - `StateRestoreSupport { Verified, Unsupported, Unknown }` —
    「適用外（not applicable）」variant がない。
- `crates/synvelis-player/src/rcp/verify.rs` `verify_plugin`
  - `StateRestoreSupport::Verified` の Live 条件は
    `status == OpaqueStateStatus::Known` かつ `digest.is_some()` かつ
    `quality.exact_eligible()` を無条件要求し、失敗時 `StateUnknown`。
  - `Unsupported → StateUnsupported`、`Unknown → StateUnknown`。
- `synvelis-project` 側語彙（read-only consume）:
  - `StateCaptureQuality::{Unsupported, BestEffort, Quiescent,
    Coordinated}` — `Unsupported` は「plugin が state
    save/restore を実装しない」TIM-04 語彙を既に保有。
    `exact_eligible()` は `Quiescent`/`Coordinated` のみ true。
  - `OpaqueStateStatus::{Known, Unknown, Missing}` — 検証状態を表し、
    statelessness は表現しない。
- env 側の stateless 事実の供給経路は実在:
  `crates/synvelis-player/src/plugin_state.rs` が
  `HostError::Unsupported → PluginStateCaptureError::Unsupported`
  に写像し、stateless mock fixture の先例
  （`tim04_plugin_state.rs`）がある。

### sv#514 EXP-4 の棄却理由の再確認

`evidence` 相当の
`docs/impl/exp/issue-514-session-engine-selection-exp.jsonl` EXP-4 は
「`StateCaptureQuality::Unsupported` + `digest: None` で workspace
側から stateless を表現確定できる」仮説を棄却した。理由は
`verify_plugin` が `digest.is_some()` を無条件要求し、
`StateRestoreSupport` に適用外 variant がなく、verdict 判定は
player 内にあり workspace 側に lever がないため。本 Issue はまさに
この player 契約変更を行う。

### 設計空間

Coordinator 確定の 2 系統:

- (a) `PluginStateDecl` に field/enum を追加 — 構造体 literal 3 点
  （`rcp_04.rs`・`rcp_selection_integration.rs` は lease 内、
  `crates/synvelis-render/tests/rcp07_gate.rs`:947-951 は lease 外）
  が compile 破損 → scope 拡張 ask が必須。
- (b) 既存語彙の意味論確定 — `quality ==
  StateCaptureQuality::Unsupported` を stateless marker とし、
  `StateRestoreSupport` に適用外 variant を追加 or 既存
  `Unsupported` を dual-use。literal 非破壊。

`PluginVerdict` に新 variant を加える設計は
`crates/synvelis-workspace/src/render_ui.rs`:310-321 の exhaustive
match（`_` arm なし、lease 外）を compile 破損させるため不採用方向。

## 結論

- 契約変更の所有境界は `synvelis-player::rcp`（model + verify）。
  workspace 側は証拠構築の doc 追従のみ。
- `digest: None` の「stateless by design / 未 capture」区別は
  `PluginStateDecl` の型付き predicate として表現し、env/decl の
  整合を `verify_plugin` の `Unsupported` arm で判定する設計を
  design/ADR に記録する。
