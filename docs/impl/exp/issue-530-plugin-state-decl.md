---
id: "EXP-nKUfkEsrlJqncQtrrZ5j"
type: "experiment"
status: "completed"
title: "Issue #530 experiment log — stateless 契約表現の検討記録"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/exp/issue-530-plugin-state-decl-exp.jsonl"
  implements:
    - "docs/plans/issue-530-plugin-state-decl.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/issue-530-plugin-state-decl.md"
    - "docs/impl/exp/issue-530-plugin-state-decl-exp.jsonl"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #530 experiment log

契約表現の検討過程を記録する。詳細な試行データは
`issue-530-plugin-state-decl-exp.jsonl` を参照。

## EXP-1: `PluginStateDecl` への capture-applicability field 追加

- 仮説: `capture_applicability: StateCaptureApplicability` のような
  新 field で stateless/未 capture を明示すれば AC2 を最も直接に
  満たせる。
- 結果: **棄却**。構造体 literal 3 点が compile 破損し、うち
  `crates/synvelis-render/tests/rcp07_gate.rs`:947-951 は lease 外で
  scope 拡張 ask が必須。得られる情報は
  `quality == StateCaptureQuality::Unsupported` と論理同値で、
  同一ドメイン事実の二重表現にしかならない。

## EXP-2: `StateRestoreSupport::NotApplicable` variant 追加

- 仮説: env 側に「適用外」variant を追加すれば decl/env の対応が
  より明示的になる。
- 結果: **棄却**。既存 `Unsupported`（doc: "The plugin does not
  implement state restore"）が既に env 側 stateless 事実を意味し、
  供給経路 `HostError::Unsupported →
  PluginStateCaptureError::Unsupported` も `Unsupported` に写像済み。
  variant 追加は `render_ui.rs` 等の消費側波及のみ増やす。

## EXP-3: `PluginVerdict` への stateless 専用 variant 追加

- 仮説: `StatelessLive` のような verdict で stateless を区別表示
  できる。
- 結果: **棄却**。`Live` と意味が同一で、`render_ui.rs`:310-321 の
  `_` arm なし exhaustive match（lease 外）を compile 破損させる。

## EXP-4: 既存語彙の意味論確定（採用）

- 仮説: `PluginStateDecl::stateless()` predicate（`digest: None` +
  `quality: Unsupported` + `status != Known`）で decl 側を型付き
  定義し、env 側は既存 `StateRestoreSupport::Unsupported` を
  dual-use、`verify_plugin` の `Unsupported` arm のみで Live 経路を
  開けば、variant/field 追加なしに AC を全て満たせる。
- 結果: **採用**。`digest: None` の「stateless by design / 未
  capture」は閉じた enum `StateCaptureQuality` の variant で型上
  区別され、未 capture は `stateless() == false` で fail-closed を
  維持する。oracle で全真理値表を pin。
