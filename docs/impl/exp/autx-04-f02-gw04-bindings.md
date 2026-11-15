---
id: "EXP-autx04f02gw04bindings001"
type: "experiment"
status: "completed"
title: "AUTX-04-F02 GW-04 bindings scratch-ID/rewrite — scratch 採番・resource_map diff・provisional oracle の試行"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-04-f02-gw04-bindings.md"
  depends-on:
    - "docs/automation/design-atomic-macro.md"
    - "docs/impl/ir/autx-04-f01-composite-forward-ref.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-04-f02-gw04-bindings.md"
    - "evidence/sv263/gw04-bindings-ledger.md"
    - "docs/impl/exp/autx-04-atomic-macro.md"
    - "docs/impl/exp/autx-04-f01-composite-forward-ref.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/tests/gw04_bindings.rs"
    - "crates/synvelis-project/src/ops.rs"
    - "crates/synvelis-project/src/id.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-04-F02 GW-04 bindings scratch-ID/rewrite — scratch 採番・resource_map diff・provisional oracle の試行

## 問題設定

Issue #263（AUTX-04）の gateway 側 slice。AUTX-04 本体で
`synvelis.operation.batch` の `bindings` schema 受理と非空時の
typed `PreparationRejected` fallback、F01（#432）で
`Composite::validate` の逐次 dry-run が入った。残る未確定点:

1. placeholder（`<kind>-<32hex>` over
   `OUTPUT_BINDING_ID_BASE + slot`）をどの実 ID へ rewrite
   するか — ID を推測せず、commit と同一系列で決定的に
   得る方法。
2. rewrite 済み member をどう lower/検証するか —
   `bind_target`/`lower_*` は実 tagged id を要求し、
   `Composite::validate` は member 順の逐次適用を前提に
   する（F01 で解決済み）。
3. provisional authorization（SEC-03/SEC-14 の
   fail-closed gate）が transaction-created resource を
   解決できるか — agent caller の `check_effects` は
   read/write/delete 全 resource の `Known` を要求し、
   `AddLane` 等の op_facts は create parent を `reads`
   に載せるため、pinned-base のみの oracle では
   placeholder 解決後の ID が `Unresolved` で deny になる。

## 試行と結果

### E1: `peek_raw_id` 相当の事前採番で placeholder を直接実 ID 化 — 棄却

F01 IR が記録した `peek_raw_id()` 予測を使い、scratch
適用なしに「n 番目の create が mint する raw」を計算して
rewrite する案。member op が mint する ID 数は variant
依存（`DetachClipSource` は source + note 複数個）で、
どの member が何個 mint するかを lowering 段階で正確に
知るには結局 apply が必要。かつ `draw_raw_id` 予約窓で
系列がずれ得る（F01 E3）。決定的系列の唯一の権威は
実際の `validate`+`apply` 順序なので棄却。

### E2: pinned base に対する一括 validate のみ（scratch なし）— 棄却

rewrite 後の member を `op.validate(&pin.state)` で
検査する案。forward-reference は pin.state に存在しない
ID を参照するため `NotFound` で必ず失敗する。F01 が
`Composite::validate` 内部に持つ逐次 dry-run は commit
時にしか走らず、prepare 段階の provisional gate
（`op.validate` より前）と provisional oracle には
効かない。placeholder→実 ID の解決自体に逐次適用が
必要なため棄却。

### E3: scratch clone への member 順 trial-apply + `resource_map` diff — 採用

`base.clone()` に `CounterIdGenerator::new(
base.id_watermark)` を seed し、member 毎に
rewrite → `NamedOperation::parse` → 通常 lowering →
`validate`+`apply` を scratch へ行う案を採用。

- **採番系列**: `id_watermark` は「次に発行する raw」
  （`CounterIdGenerator::new(seed)` の `next()` が seed
  を返す）で、`Composite::apply` が使う controller
  generator と同じ seed・同じ member 順のため、全成功
  時に scratch post-state は実 candidate と一致する
  （`agent_create_then_reference_commits_as_one_undo_unit`
  で track=1000/lane=1001 を実測）。
- **created 検出**: op variant を仮定せず、apply 前後の
  `prepare::resource_map` diff で「新規 key かつ宣言
  kind 一致」を数える。`DetachClipSource` のような
  複数 resource 生成 op でも variant 知識なしに
  exactly-one 判定できる（`rejects_wrong_created_arity`
  の created 2 を実測）。
- **失敗時の scratch 不変条件**: member の apply は
  clone した trial に対して行い、成功時のみ
  `scratch`/`ids` を差し替える。失敗 member は
  skip され（scratch = base + 成功 member 全て・
  member 順）、後続の独立 producer は引き続き
  解決できる。失敗自体は emitted `Composite` の
  `op.validate` が provisional gate 後で再現する
  （SEC-03 ordering 保存、
  `rejects_unresolvable_producer_output` で確認）。

### E4: provisional gate への scratch post-state 供給 — 採用

`lower_named` の第 3 戻り値として
`Option<Arc<Project>>`（bindings batch のみ `Some`）を
追加し、`provisional_gate` が `BaseFirstOracle::new(
&pin.state, scratch)` を構成する案を採用。これがないと
agent caller の `check_effects` が `AddLane` の
`reads={Track(1000)}` を `Unresolved` と判定し、
create-then-reference が常に `Denied` になる。
`bindings` 非所持の operation は従来の pinned-base
`ProjectOracle` を維持し、通常経路の oracle 挙動を
変えない。derived-effects 段（candidate 確定後）は
従来通り `BaseFirstOracle::new(&pin.state, &candidate)`
で、candidate は scratch と同一系列で構築されるため
provisional と derived で同じ created resource が
`Known` になる。

### E5: 失敗 member 以降の binding を即時 reject する案 — 棄却

producer の apply 失敗時に `scratch_tainted` flag で
以降の binding 解決を全て止める案。composite は
いずれにせよ失敗するが、失敗が placeholder 依存で
ない独立 producer まで巻き込むと「どの error が先に
出るか」が不必要に変わる。trial-clone（E3）なら
scratch が「base + 成功 member 全て」を保ったまま
後続を解決できるため、
より情報量の多い provisional gate 結果を維持できる
側を採用。

## 結論

採用: member wire JSON への再帰 rewrite +
`bindings` 宣言の strict parse + scratch clone への
member 順 `validate`+`apply`（trial-clone で scratch
不変条件を保持）+ `resource_map` diff による
exactly-one created 判定 + `BaseFirstOracle` への
scratch 供給。棄却: 事前採番（E1）、pinned-base のみ
validate（E2）、taint flag（E5）。
`design-atomic-macro.md` §3 の対応形を実装し、
非空 `bindings` の typed fallback を撤去した。
検証は `tests/gw04_bindings.rs` 12 件 +
`prepare_engine.rs` unit 6 件（`evidence/sv263/
gw04-bindings-ledger.md` 参照）。
