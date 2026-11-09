---
id: "ADR-YRDVKLkFDWrK8LUdnvCroE"
type: "adr"
status: "accepted"
title: "PC-C6 oracle — test 内独立参照実装による REL/TR fixture 判定"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-oracle-rel-tr.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
    - "docs/designs/pc-c6-oracle-rel-tr-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# PC-C6 oracle — test 内独立参照実装による REL/TR fixture 判定

## Status

accepted

## Context

Issue #103 の固定 Test Matrix は PC-C2-REL01〜07 と PC-C3-TR01〜08 を
「別 Oracle 判定」で検証することを要求する。対象契約本体
（`RelationIndex`・`ControlTrace` projection）は PR #204/#206 で main に
merge 済みであり、既存受入 test（`pc_c2_relation.rs`/`trace_c3.rs`）が
契約の代表的性質を直接 assert している。

争点は oracle の置き場所と独立性の程度である:

1. 期待値をどこで算出するか — 既存 test の期待値の再利用では
   「再主張」に留まり、実装と同じ誤読を共有する危険がある。
2. oracle を test crate 内に置くか、別 crate/ツールに置くか —
   `synvelis-project` の公開 API のみで構成可能な範囲か。
3. 大規模 graph 性能実測（REL-C2-04 からの繰越）をどう記録するか —
   Issue は「Perf thresholds は実 Baselines を測定してから確定」と
   するため、閾値を先に決め打ちできない。

## Considered Options

* Option 1(採用): test ファイル内に独立した参照実装（oracle）を置く。
  fixture が宣言した control model / trace 構成から edge 導出・状態
  分類・有界 BFS・freshness 分類を test 側で再導出し、
  `RelationIndex::query`/`ControlTrace::freshness` の観測と全件照合
  する。契約本体の内部実装には依存しない。
* Option 2: 既存受入 test（`pc_c2_relation.rs`/`trace_c3.rs`）の
  期待値を流用・パラメタ化して再実行する — 実装と同じ前提を共有する
  「再主張」に留まり、Issue の「別 Oracle 判定」要件を満たさない。
  却下。
* Option 3: proptest 等の property-based test で無作為 fixture を
  探索する — 期待値の独立導出は同じ問題を含む上、証拠としての
  再現性・Case ID 対応が弱い。dev-dependency 追加も本 slice の
  編集範囲外。却下。
* Option 4: oracle を別 crate・別バイナリ・CI job として分離する —
  契約本体と同一 workspace の test target で十分に独立性を確保でき、
  分離は証拠連鎖を複雑にするだけ。却下。

## Decision

1. **oracle は test crate 内の独立参照実装。** `tests/` 配下の新規
   ファイルに、fixture 宣言（control map への直接構築 + 宣言的 edge
   モデル）から期待 `RelationReply` / `ObservationFreshness` を導出する
   oracle を実装し、契約本体の観測と照合する。oracle は
   `RelationIndex`/`ControlTrace` の内部関数を呼ばず、公開 API の
   観測結果のみと比較する。
2. **fixture は direct control map 構築を継続使用。** 既存 test と同じ
   `pc_c2_relation.rs` の流儀で `Project::control` を直接構築し、
   dangling 参照・disabled cycle 等 op oracle が拒否する状態にも到達
   する。`ProjectRevision` は任意 tag、時刻系は `Instant`/`MediaTime`
   の合成値を使う。
3. **性能実測は測定値記録 + 保守的 assertion。** REL07 の巨大 graph
   fixture で build/query 時間を `Instant` で計測し `eprintln!` と
   impl record に記録する。fail 条件は「完了しない・結果が oracle と
   一致しない・有界性を逸脱」に限定し、絶対時間閾値は Baseline 未
   確定のため設けない（緩い sanity 上限のみ）。
4. **TR 系は project 層 projection 契約に限定。** `synvelis-player`
   の dispatch ledger 実経路・実 RT 観測・実 plugin report は対象外
   （TRACE-C3-05 は別 dispatch）。TR07 の「2 Inspector 共有 probe」は
   project 層では key 別独立評価として検証し、実 UI 二重起動防止は
   #114/#102 側の責務として記録する。

## Consequences

- `crates/synvelis-project/src/`・他 crate・`Cargo.toml`/`Cargo.lock`
  は無変更。追加は `tests/` の新規ファイルと `docs/` チェーンのみ。
- oracle の独立性は「同じ契約を別実装で導出する」レベルであり、
  契約解釈そのものの誤り（spec 誤読）は検出できない — その場合は
  spec/design/test-spec 側の修正が必要になる。
- 大規模 graph の絶対性能値は本 slice では Baseline として記録する
  に留まり、回帰閾値の確定は実測蓄積後の別判断となる。
- test 実行時間は巨大 graph fixture 分だけ増えるが、headless の
  `cargo test` 範囲に収まる規模（数万 edge、秒未満〜数秒）に留める。
