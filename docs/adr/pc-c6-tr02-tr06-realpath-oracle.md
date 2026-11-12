---
id: "ADR-034WxlsHU6WOjsdvvOogdb"
type: "adr"
status: "accepted"
title: "PC-C6 TR02/TR06 realpath oracle — 既存 dispatch oracle file への test 追加"
created: "2026-11-13"
updated: "2026-11-13"
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
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c6-tr02-tr06-realpath-oracle.md"
    - "docs/specs/pc-c6-tr-ledger-oracle.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c6-tr-ledger-oracle.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
    - "docs/adr/pc-c3-05-trace-adapter-drain-history.md"
    - "docs/designs/pc-c6-tr02-tr06-realpath-oracle-design.md"
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

# PC-C6 TR02/TR06 realpath oracle — 既存 dispatch oracle file への test 追加

## Context and Problem Statement

Issue #103 の固定 Test Matrix の PC-C3-TR02/TR06 行は、project 層
oracle（`pc_c6_oracle_tr.rs`）で分類規則が証拠化済みだが、
`synvelis-player` dispatch-ledger 実経路での再証拠が残件である
（`docs/specs/pc-c6-tr-ledger-oracle.md` が Deferred に記録）。
実経路 rig・fixture・独立 oracle は直前 slice の
`crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs` に確立済み。

争点は TR02/TR06 を実経路で再証拠化する際の配置である:

1. test をどこに置くか — 既存 oracle file への追加か、新規 file の
   重複か、あるいは `trace_c3_05.rs` への追加か。
2. TR02 の 7 次元行列と TR06 の silence/unobserved 区別を、実経路が
   生成し得る証拠の範囲でどう構成するか。

## Decision Drivers

* 既存 rig（`host_plugin`/`ledger_rig`/`fabricated_adapter`）・fixture・
  独立 oracle（`oracle_*`/`assert_trace_agrees`）を再利用し、
  ~700 行の基盤を複製しない。
* 実経路が生成しない状態（`observed` slot の `Expired`/`Dropped`/
  `Unsupported` 等）を捏造しない — 行の要求範囲を実経路証拠の
  境界に正直に合わせる。
* adapter 単体契約・slot variant 写像は `trace_c3_05.rs` と
  project 層 oracle が網羅済み — 再主張しない。

## Considered Options

* Option 1(採用): 既存 `pc_c6_oracle_tr_dispatch.rs` に TR02/TR06 の
  2 test を追加し、file 冒頭 doc comment の対象行を TR01〜TR08 全行に
  更新する。
* Option 2: 新規 test file（例 `pc_c6_oracle_tr02_tr06_dispatch.rs`）
  を作り rig/oracle を複製する — 同一 file 内の基盤を ~700 行
  重複させ、fixture 値の系統分岐を増やすだけで証拠価値を増やさない。
  却下。
* Option 3: `trace_c3_05.rs` に assert を追加する — adapter 単体契約
  （TRACE-C3-05）の証拠 file であり、PC-C6 oracle 行の独立判定という
  別責務を混在させる。直前 slice ADR の Option 2 と同じ理由で却下。
* Option 4: TR02/TR06 を project 層証拠のまま `NotRun` 継続する —
  #103 の TR 行が実経路で閉じない。却下。

## Decision Outcome

Chosen option: "Option 1", because 既存 file はまさに「TR oracle
分類規則の実経路再証拠化」のために作られた file であり、rig・oracle・
fixture の再利用が最も整合的である。file が TR01〜TR08 全行をカバー
する形になり、project 層 oracle file（`pc_c6_oracle_tr.rs`）と
対象行が一致する。

### Consequences

* Good, because 追加は test 2 本と doc comment 更新のみで、基盤の
  重複・保守対象の分岐が発生しない。
* Good, because 実経路が生成しない slot variant（`observed` の
  `Expired`/`Dropped`/`Unsupported`）を要求せず、行の証拠範囲を
  正直に限定できる — それらの写像は `trace_c3_05.rs`・project 層
  oracle の責務のまま。
* Bad, because file が 8 test・約 2000 行に育ち、行単位の失敗切り分けは
  test 名に依存する — 既存の `trNN_*` 命名で識別可能。

### Confirmation

`mise run test` で `pc_c6_oracle_tr_dispatch` の 8 test 全 pass、
および impl record の fixture matrix ledger で TR02/TR06 行が
実測 Pass として記録されること。

## Implementation Plan

* Affected paths:
  `crates/synvelis-player/tests/pc_c6_oracle_tr_dispatch.rs`（test
  2 本追加 + 冒頭 doc comment 更新）、`docs/` の本 chain 7 file。
* Patterns to follow: 直前 slice の `tr03_context_change_stale_
  real_ledger`（live evidence 構成・`live_ctx`・scenario table・
  `assert_trace_agrees`）と `tr04_plugin_unavailable_vs_probe_states`
  （channel state 別分類）。
* Patterns to avoid: `ControlTrace` 手組み、oracle が実装メソッドを
  呼ぶ期待値導出、実経路未生成状態の捏造、headless 結果の実機
  成功への繰り上げ。
* Dependencies or configuration: なし（既存 dev-deps・manifest
  不変）。

## Verification

* [ ] `cargo test -p synvelis-player --test pc_c6_oracle_tr_dispatch`
  で 8 test 全 pass（tr02/tr06 新規を含む）。
* [ ] canonical 9 lane（`mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/`build`）
  全て exit 0。
* [ ] `docs/*/README.md` index を doc-maintenance `index apply` で
  再生成し `index plan` が in-sync を報告する。

## Pros and Cons of the Options

### Option 1

* Good, because rig/oracle/fixture を再利用し diff が最小かつ整合的。
* Good, because file の責務（TR 分類規則の実経路再証拠化）が
  TR01〜TR08 全行で完結する。
* Bad, because 直前 slice の「6 test」記述と file の test 数が変わる
  — impl record で新旧を明記して解消。

### Option 2

* Good, because slice 単位で file が分かれ履歴が追いやすい。
* Bad, because ~700 行の基盤複製と fixture 系統の分岐で保守コストが
  増え、同一 oracle 規則の実装が 2 箇所に分散する（DRY 違反）。

## More Information

* `docs/specs/pc-c6-tr02-tr06-realpath-oracle.md` — 本 slice の spec。
* `docs/specs/pc-c6-tr-ledger-oracle.md` と
  `docs/adr/pc-c6-tr-ledger-oracle.md` — 直前 slice の決定（実経路
  rig・独立 oracle・対象行限定）の正本。本 ADR はその対象行限定を
  TR02/TR06 に適用した残件 slice であり、構成原則を継承する。
* `docs/test-specs/pc-c6-e2e-oracle-rel-tr.md` Rule(TR02)/Rule(TR06)
  — 行が要求する契約の一次情報。
