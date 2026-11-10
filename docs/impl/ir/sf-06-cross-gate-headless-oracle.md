---
id: "IMPL-034V2wJTpu3xW419A7YT37"
type: "impl"
status: "draft"
title: "SF-06 cross-gate headless oracle 実装記録"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/109"
  changes:
    added:
      - "crates/synvelis-project/tests/sf06_oracle.rs"
      - "evidence/reports/sf-06-cross-gate-ledger.md"
      - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
      - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
      - "docs/designs/sf-06-cross-gate-oracle-domain-evidence.md"
      - "docs/plans/sf-06-cross-gate-oracle.md"
      - "docs/test-specs/sf-06-cross-gate-oracle.md"
      - "docs/tasks/sf-06-cross-gate-oracle-headless.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sf-06-cross-gate-oracle-headless.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/adr/sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-06 cross-gate headless oracle 実装記録

## Summary

Issue #109 の headless slice として、SF-01〜SF-05 を横断する
独立 Domain Oracle・Case ID registry・fault fixture・
evidence ledger を実装した。`synvelis-project` の persisted
層のみを対象とし、#106/#107/#108 の実装本体・Ruviform
generic primitive には触れていない。

## Context

`docs/plans/sf-06-cross-gate-oracle.md`（spec:
`sf-06-cross-gate-oracle-case-ledger.md`、ADR:
`sf-06-cross-gate-domain-oracle-case-id-evidence-ledger-headless.md`、
design: `sf-06-cross-gate-oracle-domain-evidence.md`）に従う。
PC-C6（`pc_c6_oracle_rel.rs`/`pc_c6_oracle_tr.rs`）の
declarative fixture + 独立導出パターンを適用。

## Implementation

### Changes

- `crates/synvelis-project/tests/sf06_oracle.rs`（新規、6 test）:
  - `AudibilityOracle<'p>` — persisted `RoutingModel`/
    `ControlModel` の生値のみから port→strip 解決・mute/solo
    抽出（`base >= 0.5`）・enabled edge（muted source 除去）・
    upstream/downstream BFS・SF-TX-07/08 policy 合成を独立
    再実装。SUT の `audible_strips`/`enabled_strip_edges`/
    `strip_muted`/`strip_soloed`/`strip_of_port`/
    `strip_of_processor` は呼ばない。
  - `sweep_audibility` — persisted `ControlModel` を clone し
    mute/solo bit を perturb、全 2^(2n) 組合せで SUT/oracle
    の `BTreeSet<StripId>` を比較。mix topology（7 strip、
    2^14=16384 組合せ）・chain topology（4 strip、2^8=256、
    b→master 直接 edge 無効化 variant 含む）。
  - `routing_invariant_violations` — persisted 契約の独立
    再検査（map key==id・master 一意・bus↔strip 1:1・strip
    chain/port back-reference・Main port 1in/1out・processor
    単一 chain 帰属・port owner 解決・sidechain=input のみ・
    connection output→input・send↔SendConfig 1:1・非 send
    edge (src,dst) 一意・gain_target 解決・enabled graph 非
    循環・Mute/Solo 権威単一性）。`validate_references` は
    oracle 内で呼ばない（SUT 側との併用比較のみ）。
  - fault matrix — stale CAS・stale capability generation・
    feedback cycle・duplicate temp ID・Send-kind の
    AddConnection・empty channel map・第二 master・参照中
    delete の 8 系統で reject + Live/Revision/serialized
    project byte 不変を検証。cycle/dup-temp は error 文字列も
    確認。末尾で成功 op + undo が pre-fault 内容へ戻ること
    （revision は apply+undo で +2）を検証。
  - v5→v6 migration oracle — 宣言 fixture（track 数）から
    期待投影を導出し v5 envelope load と比較。再 migration
    の決定性（`c2.live() == c3.live()`）と save の新 snapshot
    head 発行を検証。
  - oracle 自己検証 — corruption fixture（orphaned send・
    第二 master・duplicate edge・duplicate mute 権威・
    dangling endpoint）で非空 violation を確認。
- `evidence/reports/sf-06-cross-gate-ledger.md`（新規）—
  Issue #109 schema 全 column の Case ID registry + lane 別
  結果。headless lane のみ実測値、実 OS GUI・native plugin・
  device・consumer（Synvelis #219/#294）は `NotRun`。
- docs チェーン（spec/ADR/design/plan/test-spec/task/IR）。

### Decisions During Implementation

- Oracle は SUT の公開 persisted フィールドと documented
  policy のみを読み、private 実装・内部キャッシュに依存しない
  — `AudibilityOracle` は `&Project` を保持し全ての解決を
  自前で行う。
- Sweep は persisted `ControlModel` を clone して Boolean
  `base` を直接 perturb — `SetStripMute`/`SetStripSolo` 経由
  ではなく oracle が独立に状態を構成できるようにするため。
- Fault matrix の「状態不変」は `live_revision()` +
  `serde_json::to_value(c.live())` の組で byte 比較 —
  reject が undo stack にも影響しないことを末尾の undo で
  裏付ける。
- v5→v6 migration oracle は `routing_sf01.rs` の envelope 構築
  を再利用し、期待値は宣言 fixture から導出（track 数・
  master 一意・strip/edge 数・processor/send 空）— 具体的な
  ID 値に依存しない。
- `ParameterRole` は `Ord` を実装しないため、bool 権威の
  重複検査は `(StripId, &'static str)` を key にした
  `BTreeMap` で行う。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- [x] `cargo test -p synvelis-project --test sf06_oracle`
  exit 0 — 6 test 全 Pass（audibility sweep mix 2^14 + chain
  2^8×2 variant、invariant oracle、corruption 検出、fault
  matrix 8 系統、v5 migration）。
- [ ] `mise run fmt-check` exit 0。
- [ ] `mise run check` exit 0。
- [ ] `mise run lint` exit 0。
- [ ] `mise run test` exit 0。
- [ ] `mise run test-rt` exit 0。
- [ ] docs 索引再生成（doc-maintenance preview→apply）。

## Risks

- #106/#107/#108 の並行実装で契約が動いた場合、本 test の
  前提（persisted 層の公開フィールド）が陳腐化する可能性 —
  ledger の `fixture & command`/`expected vs observed` 列で
  再評価点を記録済み。
- headless Pass を実機受入と誤認するリスク — ledger で lane
  を列分離し `NotRun` を明示、spec/ADR に昇格禁止を記録。

## Follow-ups

- SF-ACT/PROBE/UX/COMPAT の runtime・GUI・device・consumer
  lane — #106/#107/#108 merge・#114/#176/#219/#294 gate 完了後
  に ledger を更新。
- EX-001〜020・RF#193 G2/G7・RF#308/#309 の evidence link —
  ledger に既存 link を記録済み、再採取は別 gate。
