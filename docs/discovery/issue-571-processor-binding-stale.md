---
id: "DISC-571ProcessorBindingStale01"
type: "discovery"
status: "resolved"
title: "Issue #571 discovery — SetProcessorBinding rebind が SemanticPort.binding を stale に残す UnresolvedPort 拒否迂回"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/571"
    - "https://github.com/scarecrow173/synvelis/issues/565"
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
    - "docs/specs/issue-571-processor-binding-stale.md"
    - "docs/test-specs/issue-571-processor-binding-stale.md"
    - "docs/discovery/issue-565-processor-binding-cascade.md"
    - "docs/specs/issue-565-processor-binding-cascade.md"
---

# Issue #571 discovery — SetProcessorBinding rebind が SemanticPort.binding を stale に残す UnresolvedPort 拒否迂回

## 背景

`synvelis#565` の processor-binding cascade 監査
（`evidence/sv565/audit.md` ADJ-1、`docs/discovery/issue-565-processor-binding-cascade.md`）
で発見された P2 として起票された pre-existing defect —
`synvelis#565` 起因ではなく main 既存。`SetProcessorBinding`
による rebind が `Processor.binding` と persisted
plugin-instance identity を更新する一方、processor が所有する
`SemanticPort.binding` を一切更新しないため、rebind 後の
persisted state が「同じ最終 binding を持つ insert-fresh
processor の state」と一致しない path-equivalence 違反を監査
記録として固定する。

中心となる問い: **port の persisted plugin stamp は binding の
owner である `Processor.binding` と常に一致するか**。一致しない
ならどの消費経路が stale stamp を読み、何が壊れるか。

## 欠陥機構（main `d68f0e54` で再確認）

port の stamp は `InsertProcessor` 時に一度だけ導出されて
persisted される:

- `processor_port_binding`（`crates/synvelis-project/src/ops.rs`）
  が `ProcessorBinding` → `PortBinding` の唯一の authoritative
  な導出実装。`Resolved(reference)` → `PluginDeclared{plugin:
  reference.descriptor, advertised, capability_revision:
  reference.capability_revision}`、`Unresolved{descriptor}` →
  `PortBinding::Unresolved{plugin: descriptor, advertised}`。
- `InsertProcessor` の apply arm はこの helper で
  `in`/`out`/`sidechain`（`sidechain_input` 時）の各 slot を
  初期化し、`SemanticPort` として `routing.ports` に永続化する。

一方 `SetProcessorBinding` の apply arm（`ops.rs:2388-2398`、
修正前）は `proc.binding = binding.clone()` と
`project.plugin_instances.sync_processor(proc)?` のみを実行し、
`proc.port_ids` 経由で辿れる `routing.ports` の各 port の
`binding` を更新しない。port は `Processor.binding` から独立に
persisted されるため、rebind 後も旧 plugin の stamp が残る。

## stale stamp が壊す消費経路

### (a) `UnresolvedPort` 拒否の迂回 — `Resolved(A)→Unresolved`

`routing_tx.rs` の `resolved_endpoints` は endpoint の port
binding が `PortBinding::Unresolved` のとき
`RoutingEditError::UnresolvedPort` で拒否する — 「unresolved
plugin の port は wire 不可能」という設計された fail-closed。
しかし `Resolved→Unresolved` rebind 後の port は
`PluginDeclared` のまま残るため、この拒否を素通りする。
`InsertProcessor`+`Unresolved` path が同一論理状態（unresolved
plugin を載せた processor）で `Unresolved` port を持ち拒否され
るのに対し、rebind path は受理される — 2 経路で発散。

### (b) persisted plugin identity の不整合 — `Resolved(A)→Resolved(B)`

port stamp の `plugin`/`capability_revision` が plugin A のまま
残り、`Processor.binding`（B）と永続化済みの事実が矛盾する。
declared-port を読む消費経路（`rcp_selection` 報告等）が嘘を
返し、`port_capability_generation` の入力にも stale stamp が
混入する（generation は `PluginDeclared.capability_revision`
を合算する fingerprint）。save→load でも oracle は port ↔
processor binding の一致を検査しないため不整合は persisted
のまま残る。

### (c) `Unresolved→Resolved` — 回復でも stamp が残る

`Unresolved` で persisted された port は rebind で
`Resolved(B)` に戻しても `PortBinding::Unresolved` のまま —
逆方向にも insert-fresh shape と発散し、本来 resolvable な
endpoint が `UnresolvedPort` で拒否され続ける。

### (d) `vcs06` fixture の乖離

`vcs06_plugin_instance_matrix.rs`（修正前 :611-632）の
rebind fixture は `capability_revision` のみ手動で stamp し
「the shape a real rebind op would produce」とコメントしていた
が、実 op は port stamp を一切更新しない — fixture の自称する
形状と実 op の産物が乖離していた（`plugin` descriptor は手動
stamp でも更新されず、修正後の実 op が産む stamp とも異なる）。

## 修正方針

`proc.binding` を更新するのと同じ境界（project domain layer、
`SetProcessorBinding` apply arm）で、`proc.port_ids` 経由の
所有 port 各々の `binding` を `processor_port_binding` で再導出
する。port 固有の `advertised` slot 名（`"in"`/`"out"`/
`"sidechain"`）は port 自身の semantic data であり rebind で
保存する。`PortBinding::Internal` は plugin stamp を持たない
ため対象外。`sync_processor` の PC-C4-05 identity  semantics
（新 `PluginRef` → 新 id 発行・旧 id 退役、同一 `PluginRef` →
id 維持、`Unresolved` → entry 削除、単調 watermark）は
`synvelis#565` で不変条件健全と監査済みのため変更しない。

## 近接問題との境界

`synvelis#572`（`routing_tx.rs` の `port_capability_generation`
が同一 `capability_revision` の rebind を fingerprint できない
staleness gap）は同 crate の同 class 問題だが、`routing_tx.rs`
は本 slice の lease 外。parity 検証のため `resolved_endpoints`
/`port_capability_generation` を読み取り・実行するのみで
変更しない。

## 結論

欠陥は `SetProcessorBinding` が `proc.binding` と persisted
port stamp を同期させないことにあり、修正は同じ apply arm で
`processor_port_binding` を再利用して port を再 stamp する
最小の整合的変更で着地。`Resolved→Unresolved`・
`Resolved(A)→Resolved(B)`・`Unresolved→Resolved` の 3 方向
すべてで rebind-path state が insert-fresh state と一致すること
を pin test で固定し、`vcs06` fixture は実 op を通した rebind で
after 状態を作る形に修正した。
