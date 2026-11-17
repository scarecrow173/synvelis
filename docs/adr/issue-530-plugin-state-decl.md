---
id: "ADR-TPzhu7QHmjQxoSv91BBZ"
type: "adr"
status: "accepted"
title: "Issue #530 — stateless は PluginStateDecl::stateless() profile + StateRestoreSupport::Unsupported dual-use で表現する"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
    - "https://github.com/scarecrow173/synvelis/issues/514"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "docs/discovery/issue-530-plugin-state-decl.md"
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/specs/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/designs/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/plans/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/tasks/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/test-specs/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/impl/ir/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/impl/exp/issue-530-plugin-state-decl.md"
      - type: "file"
        path: "docs/impl/exp/issue-530-plugin-state-decl-exp.jsonl"
      - type: "dir"
        path: "evidence/sv530"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/rcp/model.rs"
      - type: "file"
        path: "crates/synvelis-player/src/rcp/verify.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/rcp_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/rcp_selection.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rcp_selection_integration.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-530-plugin-state-decl.md"
  implemented-by:
    - "docs/designs/issue-530-plugin-state-decl.md"
    - "docs/plans/issue-530-plugin-state-decl.md"
  depends-on:
    - "docs/specs/issue-530-plugin-state-decl.md"
    - "docs/designs/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "evidence/sv514/decision-ledger.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-530-plugin-state-decl.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv530/decision-ledger.md"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# Issue #530 — stateless は PluginStateDecl::stateless() profile + StateRestoreSupport::Unsupported dual-use で表現する

## Context and Problem Statement

`PluginStateDecl.digest: Option<String>` は「state blob が存在
しない」を表すが、stateless by design（plugin が state
save/restore を実装しない）と未 capture（capture すべきだが無い）
を型上区別できない。`verify_plugin` は `StateRestoreSupport::
Verified` の Live 条件に `digest.is_some()` を無条件要求するため、
stateless plugin は `Live` に到達できず `StateUnknown` に落ちる。
sv#514 EXP-4 で workspace 側 lever がないことが確定し、本 Issue に
帰属された。

## Decision

**`PluginStateDecl` に新 field を追加せず、
`StateRestoreSupport` に variant を追加せず、`PluginVerdict` に
variant を追加しない。** 代わりに:

1. `PluginStateDecl::stateless()` predicate を追加し、
   stateless-by-design profile を `digest: None` +
   `quality: StateCaptureQuality::Unsupported` +
   `status != OpaqueStateStatus::Known` として型付きで定義する。
2. `StateRestoreSupport::Unsupported` を dual-use とし、
   「plugin が state restore を実装しない」環境側事実として扱う
   （doc は既にこの意味を持ち、供給経路
   `HostError::Unsupported → PluginStateCaptureError::Unsupported`
   も実在する）。
3. `verify_plugin` の `Unsupported` arm のみを変更し、
   `decl.state.stateless()` が true のとき `Live`、それ以外は
   従来どおり `StateUnsupported`。

## 根拠

1. **型上区別は既に存在する**: `StateCaptureQuality` は閉じた
   enum で、`Unsupported` は TIM-04 が「plugin は state
   save/restore を実装しない」と宣言する語彙。`digest: None`
   単体は両義的だが `quality` が disambiguate する。並行 field を
   足すと同一ドメイン事実の二重表現になり矛盾し得る。
2. **波及コスト**: field 追加案は構造体 literal 3 点
   （`rcp07_gate.rs`:947-951 は lease 外）を compile 破損させ、
   scope 拡張が必須。variant 追加案は `render_ui.rs` の
   exhaustive match を破損させる。得られる識別力はゼロ。
3. **fail-closed 維持**: 未 capture（capture 可能 quality +
   `digest: None`）は `stateless()` == false で `StateUnsupported`/
   `StateUnknown` に留まる。env=`Verified` + stateless decl の
   drift も `StateUnknown` に fail-closed。env=`Unknown` は
   stateless decl でも `StateUnknown`。
4. **所有境界**: verdict 判定は player 内にあり、workspace 側は
   証拠をそのまま pin するのみ。契約変更は `synvelis-player::rcp`
   に閉じる。

## Consequences

- stateless plugin（state save/restore 非実装）が
  `PluginCapabilitySnapshot` 経由で `Live` 判定可能になる。
- `digest: None` の「stateless by design / 未 capture」は
  `PluginStateDecl::stateless()` で型上区別される。
- 未 capture は引き続き fail-closed。
- `PluginStateDecl`/`StateRestoreSupport`/`PluginVerdict` の
  variant/field 数は不変で、`render_ui.rs` exhaustive match・
  `rcp07_gate.rs` literal・`rcp06_render_ui.rs` は無変更で compile
  維持。
- `ProcessorPluginEvidence.state` の doc に stateless profile の
  宣言方法を明記（コード変更なし）。

## Alternatives Considered

- **`PluginStateDecl` への `capture_applicability` field 追加**:
  `quality == Unsupported` と論理同値の情報を別表現し、literal 3 点
  （うち 1 点は lease 外）の compile 破損と scope 拡張を要求する。
  AC2 は既存型付き enum で満たせるため却下。
- **`StateRestoreSupport::NotApplicable` variant 追加**: env 側の
  stateless 事実は既存 `Unsupported` が意味し、供給経路も
  `Unsupported` に写像済み。消費側波及のみ増えるため却下。
- **`PluginVerdict::StatelessLive` 等の新 variant**: `Live` と意味が
  同一で、`render_ui.rs` exhaustive match を破損させる。却下。
- **workspace 側での verdict 上書き**: verdict 判定は player 内に
  あり lever がない（sv#514 EXP-4 確定済み）。却下。
