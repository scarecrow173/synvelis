---
id: "IMPL-034XKYCcvVVYhxGwGvv8JV"
type: "impl"
status: "completed"
title: "AUTX-04-F02 GW-04 bindings scratch-ID/rewrite seam"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/tests/gw04_bindings.rs"
      - type: "file"
        path: "docs/impl/exp/autx-04-f02-gw04-bindings.md"
      - type: "file"
        path: "docs/test-specs/autx-04-f02-gw04-bindings.md"
      - type: "file"
        path: "evidence/sv263/gw04-bindings-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "docs/automation/design-atomic-macro.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/test-specs/autx-04-f02-gw04-bindings.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-atomic-macro.md"
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-04-f02-gw04-bindings.md"
    - "docs/test-specs/autx-04-f02-gw04-bindings.md"
    - "evidence/sv263/gw04-bindings-ledger.md"
    - "docs/impl/exp/autx-04-atomic-macro.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-atomic-macro.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-04-f02-gw04-bindings.md"
  verified-by: []
  references:
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/id.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-04-f02-gw04-bindings.md"
    rejected: []
---

# AUTX-04-F02 GW-04 bindings scratch-ID/rewrite seam

## Summary

Issue #263（AUTX-04）の gateway 側 slice として、
`synvelis.operation.batch` の非空 `bindings` に対する
typed fallback を、prepare 時の scratch-ID/rewrite seam
へ置き換えた。`bindings` 宣言（`{name, member, kind}`）
を strict parse し、member を pinned base の clone 上で
`CounterIdGenerator::new(base.id_watermark)` シードの
member 順 `validate`+`apply`（trial-clone）で処理し、
`resource_map` diff で exactly-one created を確認した
binding だけを placeholder rewrite の解決先とする。
rewrite は lowering 前の member wire JSON へ再帰適用し、
retained op は flatten された単一
`ProjectOp::Composite` のまま — proposal/commit/undo
契約は不変。provisional gate は bindings batch に限り
seam の scratch post-state を `BaseFirstOracle` の
candidate として受け取り、agent caller の
transaction-created resource read を `Known` に解決する。

## Context

AUTX-04 本体（PR #429 系）で `bindings` schema 受理と
非空時の `PreparationRejected` fallback、F01（#432 /
commit `737b0f2`）で `Composite::validate` の逐次
dry-run が入ったが、placeholder → 実 ID の rewrite と
provisional oracle への created resource 供給は未実装
だった（`docs/automation/design-atomic-macro.md` §3、
`docs/impl/exp/autx-04-atomic-macro.md` E3/E4、
`docs/impl/ir/autx-04-f01-composite-forward-ref.md`
Follow-ups）。本 slice がその gateway 側実装。

## Implementation

### Changes

- `crates/synvelis-gateway/src/prepare_engine.rs` —
  - `OUTPUT_BINDING_ID_BASE`/`MAX_BINDING_SLOTS` を
    automation 側 wire 定数の mirror として追加
    （`synvelis-automation` は dev-dependency のみ）。
  - `BindingDecl`/`parse_placeholder`/`rewrite_member` を
    追加 — 宣言 strict parse（未知 field・欠落・非
    object・範囲外 member・未知 kind・重複
    `(member,kind)` を typed 拒否）、reserved-range
    placeholder の slot/kind-tag/producer 順序検査、
    object key placeholder の拒否、解決済み placeholder
    の `prepare::ref_tagged` canonical id への再帰
    rewrite。
  - `lower_named` が `base: &Project` を受け取り
    `(commands, op, Option<Arc<Project>>)` を返す。
    非空 `bindings` は新設 `lower_bound_batch` へ、
    それ以外は従来通り `None` scratch。
  - `lower_bound_batch` — `bindings` 宣言 parse、
    producer→slot の `by_member` 索引、base clone +
    watermark seed の scratch/`CounterIdGenerator`、
    member 毎の rewrite→parse→lower→trial
    validate/apply（clone した trial に適用し成功時
    のみ差し替え、scratch は常に「base + 成功 member
    全て（member 順・失敗 member は skip）」）、
    `resource_map` 前後 diff で declared
    kind 一致の created がちょうど 1 つの binding のみ
    `resolved` へ記録。0/複数は typed
    `PreparationRejected`、未解決 placeholder は
    typed 拒否。member の domain 失敗は raise せず
    emitted `Composite` の `op.validate` が gate 後で
    再現する（SEC-03 ordering）。
  - `run`/`provisional_gate` — scratch を配線し、
    bindings batch の provisional oracle を
    `BaseFirstOracle::new(&pin.state, scratch)` に、
    それ以外は従来の pinned-base `ProjectOracle` に
    維持。
  - in-file unit tests を fallback 検証から seam 契約
    （happy path・wrong-kind arity・malformed 宣言・
    premature/undeclared placeholder）へ更新。
- `crates/synvelis-gateway/tests/gw04_bindings.rs`（新規）
  — 12 integration tests。agent caller の
  create-then-reference が deterministic id
  （track=1000/lane=1001）で prepare→`accept_apply`→
  `ProjectController` 1 apply/1 undo まで通ること、
  3 member 多段 binding、human caller、create axis /
  `include_future` の authorization 非迂回、宣言・
  placeholder・参照方向・producer 失敗/arity・object
  key/nested batch の fail-closed 行列、legacy
  lowering の非回帰を固定。
- `docs/automation/design-atomic-macro.md` §3 — 対応形を
  実装済み契約へ更新（placeholder 形式・bindings
  宣言・解決規則・fail-closed 条件・Workflow 分解は
  typed Unsupported 時の正規経路として残存）。
- `docs/impl/exp/autx-04-f02-gw04-bindings.md`（新規）
  — 事前採番・pinned-base validate・taint flag の
  棄却と scratch trial-apply 採用の検討を記録。
- `docs/test-specs/autx-04-f02-gw04-bindings.md`（新規）
  — bindings seam が保証する不変条件を固定。
- `evidence/sv263/gw04-bindings-ledger.md`（新規）
  — Case ID 付き結果台帳。

### Decisions During Implementation

- **scratch 採番は `base.id_watermark` シード**:
  `id_watermark` は「次に発行する raw」で、
  `Composite::apply` が使う controller generator と
  同一 seed・同一 member 順のため、全成功時に
  scratch post-state は実 candidate と一致する。
  F01 IR の `draw_raw_id` 予約窓の fail-closed 既定を
  継承する。
- **created 検出は `resource_map` diff**: op variant
  ごとの mint 規則を仮定せず、apply 前後の observable
  resource map の新規 key を declared kind で filter
  する。variant 追加時に binding 解決側の保守が不要。
- **member apply は trial-clone**: 失敗 apply の
  部分書き込みが scratch を汚染しないよう、member
  毎に `scratch.clone()`+`ids.clone()` へ適用し、
  成功時のみ差し替える。「scratch = base + 成功
  member 全て」の不変条件が常に成立し（失敗 member
  は skip され後続は処理継続 — 失敗時の scratch は
  doomed composite 専用で commit には至らない）、
  provisional oracle へ渡す post-state が一貫する。
  失敗 member は composite を doomed にするが
  lowering では raise せず、後続独立 producer の
  binding 解決を継続する。
- **provisional oracle は `BaseFirstOracle`**: agent
  caller の `check_effects` は read/write/delete の
  `Known` を要求し、`AddLane` の op_facts は create
  parent を `reads` に載せる。pinned-base のみでは
  transaction-created ID が `Unresolved`→`Denied`
  になるため、bindings batch に限り scratch
  post-state を candidate として供給する。scope・
  `create_within`・`include_future`・data-access の
  各判定は変えない（`agent_without_create_axis_
  still_denies`/`agent_scope_without_future_
  membership_denies` で非迂回を固定）。
- **domain 失敗は lowering で raise しない**:
  request-shape 失敗（宣言・placeholder 系）のみ
  lowering で typed 拒否し、member の domain
  validate/apply 失敗は emitted `Composite` の
  `op.validate` が provisional gate 後で再現する。
  SEC-03 existence blinding の ordering を保存する。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-04-f02-gw04-bindings.md` —
  scratch trial-apply + `resource_map` diff +
  `BaseFirstOracle` 供給の採用。事前採番・
  pinned-base validate・taint flag の棄却理由を記録。

### Rejected

- なし（exp 内で棄却した代替案は exp E1/E2/E5 に記録）。

## Validation

- `cargo check -p synvelis-gateway --all-targets` — Pass。
- `cargo test -p synvelis-gateway` — Pass（lib 13 +
  `gw04_bindings` 12 + 既存 26 binaries 全件、0 failure）。
- `cargo test -p synvelis-automation` — Pass（既存
  99 tests、非回帰）。
- `mise run fmt-check` / `mise run lint synvelis-gateway`
  / `mise run test-rt` / `prek` — 実施結果は
  `evidence/sv263/gw04-bindings-ledger.md` に記録。
- 実 OS・実 Plugin・実 RT・`*-msrv` レーンは対象外
  （NotRun）。

## Risks

- member 毎の trial-apply が project clone を伴い、
  member 数（≤64）比例の prepare コストが増える。
  `Composite::validate`/`apply` も同規模の逐次処理を
  行うためオーダーは同等だが、大規模 project では
  prepare が顕在化し得る。
- `OUTPUT_BINDING_ID_BASE`/`MAX_BINDING_SLOTS` の
  mirror が automation 側と乖離すると placeholder が
  解決不能になる。定数変更時は両 crate の同時更新が
  必要（現状は wire 契約として固定）。
- provisional gate への scratch 供給を `bindings`
  以外へ拡張すると pinned-base 前提の SEC-03/SEC-14
  判定が崩れる恐れがある。

## Follow-ups

- Issue #263 の Close 判定は Coordinator が行う
  （`Refs #263`、PR では `Closes` を使わない）。
- `draw_raw_id` 予約窓を跨ぐ batch の end-to-end は
  F01 から継続する既知の制限（fail-closed 既定）。
- automation 側 compile→gateway prepare の結合
  （実 macro 出力をそのまま prepare へ流す e2e）は
  別 slice の検証対象。
