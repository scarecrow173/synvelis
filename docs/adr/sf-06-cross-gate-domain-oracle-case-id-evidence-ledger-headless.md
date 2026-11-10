---
id: "ADR-034V2wtOG15uuPCjruYt6i"
type: "adr"
status: "accepted"
title: "SF-06 cross-gate 受入 — 独立 Domain Oracle + Case ID + evidence ledger（headless 昇格禁止）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/adr/pc-c6-oracle-rel-tr-headless-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  derived-by:
    - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
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

# SF-06 cross-gate 受入 — 独立 Domain Oracle + Case ID + evidence ledger（headless 昇格禁止）

## Context and Problem Statement

SF-06（Issue #109）は SF-01〜SF-05 の Routing/Plan/Probe/GUI を
OS/RT/Device/Consumer 別に受け入れる cross-gate である。
個別 slice の受入 test は揃いつつあるが、(a) 実装と同じ内部を
読むだけの検証は oracle の盲点を共有し、(b) 「どの lane が
未計測か」を示す証跡がなく、headless Pass が実機受入に
見える危険がある。

## Considered Options

* 既存 slice test の集約のみ — 追加 oracle なし。実装内部を
  共有するため oracle 盲点を検出できず、Case ID/ledger もない。
* `validate_references`/`audible_strips` を oracle として再利用
  — SUT と oracle が同一実装を共有し、トートロジーになる。
* **独立再導出 oracle + Case ID registry + lane 別 ledger**
  （採用）— persisted 公開フィールドと documented policy だけから
  期待値を導出し、未計測 lane を `NotRun` として明示する。

## Decision Outcome

Chosen option: "独立再導出 oracle + Case ID registry + lane 別
ledger", because Issue #109 が要求するのは実装の再検証ではなく
cross-gate の**証跡**であり、oracle の独立性と lane の正直な
区別が受入の信頼性そのものだから。PC-C6（`pc_c6_oracle_rel.rs`/
`pc_c6_oracle_tr.rs`）の declarative fixture + 独立導出パターンを
SF-06 に適用する。

## Implementation Plan

* Affected paths: `crates/synvelis-project/tests/sf06_oracle.rs`
  （新規 test のみ）、`evidence/reports/sf-06-cross-gate-ledger.md`
  （新規）、`docs/` チェーン。
* Constraints:
  - #106/#107/#108 の実装本体・Ruviform generic primitive は
    変更しない（契約の利用のみ）。
  - Oracle は `RoutingModel`/`ControlModel` の公開フィールドと
    documented policy のみを読み、private 実装・内部状態に
    依存しない。
  - headless Pass を実 OS GUI/device/plugin/consumer 受入へ
    昇格しない。未計測 lane は `NotRun`、非対応は
    `Unsupported`。
  - Synvelis の時間演算は checked のみ（本 slice は時間演算を
    追加しない）。
  - 別 Clock/Probe 正本を作らない（#107 epoch・RF#309 Align を
    再利用）。

## Verification

* [x] `cargo test -p synvelis-project --test sf06_oracle` exit 0
  （audibility sweep・invariant oracle・fault matrix・v5
  migration の 6 test）。
* [x] `evidence/reports/sf-06-cross-gate-ledger.md` が schema 全
  column と全 Case family を持ち、実機/consumer 行が `NotRun`。
* [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` exit 0。
