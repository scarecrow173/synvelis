---
id: "DESIGN-TR6hK0f243aEk3CmFQlfp"
type: "design"
status: "approved"
title: "VCS-06 evidence completion — RESTORE-08 / COMPAT-03/04 設計"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  implemented-by: []
  depends-on:
    - "DESIGN-lzcGjnJjPNFKiNNUpE2gE3"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "SPEC-pkRnrZ2RKJfPW1ymwMSENS"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
    - "DESIGN-034VG5a2ikhPyz9RCyjDDT"
  refines: []
  refined-by: []
  derives-from:
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
  defers: []
  deferred-by: []
---

# VCS-06 evidence completion — RESTORE-08 / COMPAT-03/04 設計

## Overview

本 slice は 2 つの独立した evidence 補完からなる。いずれも
`src/` 変更を伴わない test/report/docs のみの変更であり、
既存の VCS-06 fixture matrix の oracle（公開 API の外部観測
契約）をそのまま再利用する。

1. **RESTORE-08**: `restore_as_copy` の `fork_project` が
   `Project` を clone して `project_id` のみ再採番する実装に
   対し、`plugin_instances`（schema v13 persisted
   `StablePluginInstanceId` store）が verbatim 保持されることを
   envelope readback で pin する fixture を追加する。
2. **COMPAT-03/04**: PR #268/#306/#323 をカバーする ci.yml
   verify leg の実走 run を `gh` で取得し、required/observed
   区別つきで集約 evidence レポートに記録する。

## RESTORE-08 fixture 設計

### Oracle の選択

先例 `restore_as_copy_preserves_monitoring_verbatim`
（`vcs06_monitoring_matrix.rs`）と同じく、assertion の oracle は
publish 済み bundle の `SnapshotEnvelope` readback とする。
`fork_project` は `snapshot.projection.project().clone()` に
`project_id` を代入するだけなので、保持されるべきなのは
`Project::plugin_instances` 全体 — `watermark`・各 entry の
`PluginInstanceId`・pin された `PluginRef` — である。
`PluginInstanceIdentities` は `PartialEq` を持つため、
`assert_eq!(envelope.project.plugin_instances,
c.live().plugin_instances)` で verbatim 一致を直接 pin できる。

### Fixture 構成

- `ProcessorSpec` 2 件（`proc-a`/`proc-b`、異なる `PluginRef`
  pin）を resolved processor として project に載せ、各々に
  `PluginInstanceIdentities::resolve` で identity を発行する。
  これにより watermark=3・id=1,2 の非 default persisted 値が
  得られる（watermark は「次に発行する id」の semantics）。
- unresolved `ProcessorSpec` 1 件（`proc-unresolved`）を載せ、
  identity entry が発行されないこと（negative control）を
  assert する。
- processor 保持 project の `restore_as_copy` は plugin-state
  source を要求するため、実 `DirectoryPluginStateStore` に
  manifest + `Captured` blob を書き込み、`StoreBackedPluginState`
  port 経由で供給する（`plugin_state_manifest_and_blobs_are_
  copied` と同型の helper `plugin_state_source` に集約）。

### Assert 一覧

- pre-condition: `watermark == 3`、resolved id が `1..watermark`
  範囲、unresolved proc に identity 無し。
- post-restore: fork `project_id` が source と異なり新規採番
  される、`project.plugin_instances` が source と `PartialEq`
  一致、保持された identity が fork の `PluginRef` に対して
  `resolve()` で機能する（id 再利用なしの確認）。
- `src/` 契約は不変 — test は公開 API の外部観測のみを oracle
  にする。

## COMPAT-03/04 証拠設計

### 錨の明確化

synvelis#83（独立 CI gate）は Closed（2026-11-04）。COMPAT-03
の現行の錨は `ci.yml` の verify/full lane であり、
`tools/ci-summary.mjs` が required leg（`verify (ubuntu-
latest)`・`verify (windows-latest)`）と observed leg
（`*-macos`、pr-mode では observed-only、full-mode では
required）を定義する。この区別を evidence に明記する。

### 取得方法

- `gh pr checks <PR>` — ci.yml が `workflow_dispatch` 専用
  （#145 で PR trigger 撤去済み）のため PR head に check は
  無いことを実測として記録する。
- `gh run list --workflow ci.yml` + `gh run view` で、PR merge
  commit を含む main 向け run（および daily-ci 対応 run）の
  URL・head_sha・leg outcome を取得する。
- run 42938180676（head `13e65189`）は `verify (windows-
  latest)` が webdav MockServer の Windows TransientIo
  （socket 10054/10053）で failure。VCS-06 fixture test は当該
  job 内で pass しており、failure は VCS-06 非関連として区別
  して記録する。overall outcome は正直に `failure` と記す。

### 記録の境界

CI leg の pass は workflow 実行の証拠であり、実 OS durability・
実 plugin host・consumer・RT の受入ではない。ledger verdict
への換算（Pass* 等）は Coordinator の後続 slice に委ね、本
レポートは生の run URL/outcome と required/observed 分類のみを
提供する。

## 変更ファイル

| ファイル | 種別 | 内容 |
| --- | --- | --- |
| `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs` | new | RESTORE-08 fixture（1 test） |
| `evidence/reports/vcs-06-compat-evidence.md` | new | COMPAT-03/04 run URL/outcome 記録 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/vcs-06-evidence-completion*.md` | new | doc chain |

## リスクと対処

- **ledger 占有**: `vcs-06-fixture-matrix-ledger.md` は別 Worker
  が占有中のため編集しない。RESTORE-08・COMPAT-03/04 の行追加は
  PR 本文で統合依頼として明示する。
- **clippy `too_many_lines`**: test 本体が 100 行を超えるため
  plugin-state store 構築を `plugin_state_source` helper に
  集約して回避（挙動不変の純粋な構造整理）。
- **CI run の再現性**: `gh` の実測値のみ記録し、将来の run で
  結果が変わっても本レポートは取得時点の snapshot として扱う。
