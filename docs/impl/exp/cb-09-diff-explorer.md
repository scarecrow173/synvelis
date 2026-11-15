---
id: "EXP-cb09-diff-explorer"
title: "CB-09 Difference Explorer headless view-model 設計実験メモ (synvelis#286)"
created: "2026-11-15"
updated: "2026-11-15"
---

# 実験記録: CB-09 Difference Explorer headless スライス（synvelis#286）

## E1 Semantic Diff の行集合から `Variants` domain の bookkeeping をどう扱うか

**仮説**: live project と candidate payload を `compare_snapshots`
で比較すると、scoped content の差分に加えて `VariantSet`/
`VariantBranch` entity の差分（fork/adopt/edit が live 側の
variant bookkeeping を更新した結果）が `ChangeSet` に混入する。
これをそのまま content row として表示すると、ユーザーには
「variant 機構の内部更新」が scoped diff に見えてしまう。

- 調査: `compare_snapshots` は project 全体を比較する純粋関数で、
  `ComparisonDomain::Variants` を持つ `SemanticChange` も正当に
  生成する。Explorer の責務は「scoped content の差分を見せる」
  ことなので、Variants domain の change は content row から
  除外しつつ件数は正直に報告すべきか検討した。
- 案 A（Variants 変更も行として表示）: fork 直後の比較では
  `VariantSet`/`VariantBranch` の `Added`/`Modified` が大量に
  並び、実際の clip/param 差分が埋もれる。却下。
- 案 B（Variants domain を content row から除外し
  `variants_bookkeeping` 件数として別計上）: 行は scoped
  content のみ、`ChangeSetSummary` と `unresolved` は
  フィルタ前の全量を保持するため情報損失なし。比較 context
  自体の bookkeeping であることが型で区別できる
  （`ObjectKind::domain() == ComparisonDomain::Variants`）。

**観察**: 案 B を採用。`cb09_real_changeset_projects_domain_rows`
で mute 済み candidate の `clip/modified` 行が content row に
現れ、`variant-branch` 行が content に出ないこと・
`variants_bookkeeping >= 1` であることを検証。

## E2 Adopt 系 action の scope digest を client 側で組み立てるか

**仮説**: adopt は CAS/composite 操作で、`scope_digest` が
stale なら authority 側が拒否する。view-model が intent に
載せる digest をどこから取るか — view state・input context・
authority のどれを正本にするか。

- 調査: `CompareContext.scope_digest` は投影時の pin 済み
  context、`CompareAuthority.scope_digest` は apply 時の
  権威側再計算値。両者が一致しない場合 context は既に
  `Stale{StaleScope}` として action は全て blocked になる。
- 案 A（context の digest を intent に転記）: client が
  「見えていた context」を主張する形になり、authority と
  乖離した場合に intent 自体が嘘をつく。却下。
- 案 B（apply 時に authority の `scope_digest` を intent に
  同梱）: intent は常に「権威が今持っている digest」を運び、
  CAS 照合は authority 側の責務のまま。view-model は digest
  を計算も保持もしない — 境界が明確。

**観察**: 案 B を採用。`cb09_compare_and_adopt_require_explicit_
pins` で `Adopt` intent の `scope_digest` が
`authority.scope_digest` と一致することを検証。view-model は
digest の生成・検証を行わず、pin（branch+revision）のみを
運ぶ。

## E3 driver-neutral ViewSpec に evidence の「測定値」をどう載せるか

**仮説**: signal/perceptual layer は本 slice では実測定経路を
持たない（CB-08 orchestrator・real plugin・PCM/RT は別 gate）。
`ViewSpec` の Label/Table に何を出すか — 「測定したかのような」
数値を出すと headless 成功が real 受入に見える危険がある。

- 調査: `AuxLayerEvidence` は `origin: Option<EvidenceOrigin>`
  （`Pipeline`/`Mock`）と `alignment` を持つ。
  `EvidenceOrigin::Mock` のとき spec は provenance label
  （"mock"）を必ず含める設計にした。`Unavailable`/`Unknown` は
  行を生成せず state label のみ — 「差分なし」に見せない。
- 案 A（mock でも数値行を出す）: 表示上は本物と区別がつかず、
  Fake driver 並行開始の文脈で real 測定と誤認される。却下。
- 案 B（origin を label に必ず露出・unavailable は行なし）:
  mock seam の利用が spec テキストから監査可能。headless
  成功 ≠ real 受入の原則を UI contract 側でも維持する。

**観察**: 案 B を採用。`cb09_mock_evidence_is_labelled_not_
measured` で spec 内に "mock" provenance label が含まれること、
`cb09_unavailable_and_unknown_layers_are_not_zero_diff` で
unavailable/unknown が equality `None`・行空であることを検証。
