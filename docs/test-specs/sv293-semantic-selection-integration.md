---
id: "TSPEC-L2jQuI21cajGQw5SR950IW"
type: "test-spec"
status: "approved"
title: "sv293 semantic→selection integration 検証仕様 (synvelis#293 deferred residual slice)"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/293"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv293-semantic-selection-integration.md"
    - "docs/designs/sv293-semantic-selection-integration.md"
  implemented-by:
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
  depends-on:
    - "docs/specs/sv293-semantic-selection-integration.md"
    - "docs/designs/sv293-semantic-selection-integration.md"
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
    - "docs/test-specs/sf06-cross-gate.md"
    - "evidence/sv293/rcp-04-fault-matrix.md"
  refines:
    - "docs/test-specs/rcp-04-missing-plugin-resolver.md"
  refined-by: []
  derives-from:
    - "docs/designs/sv293-semantic-selection-integration.md"
  derived-by: []
  verifies:
    - "docs/specs/sv293-semantic-selection-integration.md"
  verified-by: []
  references:
    - "crates/synvelis-workspace/tests/rcp_selection_integration.rs"
    - "docs/impl/ir/sv293-semantic-selection-integration.md"
    - "docs/impl/exp/sv293-semantic-selection-integration.md"
    - "evidence/sv293/acceptance-ledger.md"
    - "evidence/sv293/verification.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/296"
  deferred-by: []
---

# sv293 semantic→selection integration 検証仕様

## 検証対象

`crates/synvelis-workspace/tests/rcp_selection_integration.rs`
（新規・公開 API のみ・`rcp_*` prefix で RCP-EX case-ID 対応）。

headless のみ。実 Plugin/OS/PCM/RT は対象外（RCP-EX-06 →
#296 RCP-GATE、NotRun 維持）。

## Feature/Rule/Example

### Feature: semantic→selection producer

| Rule | 内容 | Example |
| --- | --- | --- |
| TS-01 | `structure_digest` は `routing_fingerprint` と一致し、receipt build key が engine `checked_key` と field 一致 | `rcp_ex01_main_playback_all_live_publishes` |
| TS-02 | semantic port/connection が typed node/port/edge へ決定的に写像される | `rcp_ex02_topology_edge_kinds_preserved` |
| TS-03 | plugin evidence 欠落・descriptor 不一致・未知 coverage は typed error で fail closed | `rcp_producer_fails_closed_on_bad_evidence` |

### Feature: Intent 別 verdict（RCP-EX-01/03 統合経路）

| Rule | 内容 | Example |
| --- | --- | --- |
| TS-04 | 全 plugin Live → MainPlayback `Exact` → engine publish | `rcp_ex01_main_playback_all_live_publishes` |
| TS-05 | Missing plugin + render 無し → `Blocked`（偽 Exact 無し） | `rcp_ex01_missing_plugin_blocks_main_playback` |
| TS-06 | Missing + `EligibleExact`/`BitExactCertified` candidate → 置換 `Exact` → publish | `rcp_ex01_missing_plugin_exact_substitution_publishes` |
| TS-07 | OfflineRender は `min_reproducibility` 未満を gate | `rcp_ex01_offline_render_enforces_min_reproducibility` |
| TS-08 | Export/MainPlayback は preview-class artifact を拒否 | `rcp_ex03_export_rejects_preview_only_artifact` |
| TS-09 | EditingPreview は `allow_non_equivalent` opt-in のみ preview-class 置換を `NonEquivalent` で受理 | 同上 |

### Feature: edit 分類・stale（RCP-EX-04/05 統合経路）

| Rule | 内容 | Example |
| --- | --- | --- |
| TS-10 | rename → `NoRebuild`/engine `Equivalent`（publish 無し） | `rcp_ex04_edit_lifecycle_norebuild_controlonly_rebuild` |
| TS-11 | `classify_edit` が BoundParameter→`ControlOnly`、Structural→`Rebuild` を返す | 同上 |
| TS-12 | 構造編集 → fingerprint 変化 → rebuild → 新 publish | 同上 |
| TS-13 | stale selection（revision 乖離）は engine 呼出前に拒否・reconciler `Blocked`・last-good 保持・fresh selection は正常系へ復帰 | `rcp_ex05_stale_selection_never_publishes` |

### Feature: topology（RCP-EX-02 統合経路の構造部）

| Rule | 内容 | Example |
| --- | --- | --- |
| TS-14 | Send/Sidechain/Return/Main の `EdgeKind` が semantic 接続から保持される | `rcp_ex02_topology_edge_kinds_preserved` |
| TS-15 | chain 順が Main edge 列として保存される | 同上 |

## 非対象

- cutset の全 Port 充足・PDC・二重加算 oracle — player slice
  （`rcp_04.rs` 26 oracle）で検証済み。本 slice は接合部の pin。
- `render_ui::project_selection` 投影 — 既 merge・消費側変更不要。
- BranchAudition/provenance — #294。
- RCP-EX-06 実機計測 — #296。
