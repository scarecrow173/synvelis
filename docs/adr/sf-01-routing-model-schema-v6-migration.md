---
id: "ADR-034SF01Ln8qRs3tU0vWxYzBcD"
type: "adr"
status: "approved"
title: "SF-01 Routing Model 配置・Schema v6 Migration・Reserved Well-Known ID"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
  implemented-by:
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  depends-on:
    - "docs/adr/vs-a-workspace-ruviform-git-poc-stable-id.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/121"
  defers: []
  deferred-by: []
---

# SF-01 Routing Model 配置・Schema v6 Migration・Reserved Well-Known ID

## Status

approved(2026-11-04、Issue #104 確定詳細契約に基づく設計確定)。

## Context

- SF-01 は `synvelis-project` に Semantic Routing Model を導入する。
  `Project::new` 時点で Empty/New Project にも Master が厳密 1 件必要
  (SF-MODEL-01)。
- `Project::new` は ID generator を受け取らず、watermark 0 で全 Map 空の
  Project を返す。全 Stable ID は `ProjectController` 注入の
  `IdGenerator` が `u128` raw を連番発行する。
- 既存テストは `CounterIdGenerator::new(1)` / `(9_000)` 等の seed を
  広範に使い、`AssetId::new(5)` 等の発行値を直接参照する。
- `check_id_watermark` は「`id_watermark` が全 live entity ID を超える」
  ことを要求し、`max_issued_id_raw` は全 serialized entity Map の最大
  raw を取る(SF-01 契約で新規 Map も追加する)。
- Schema は v5 が現行唯一。`SnapshotEnvelope` は strict
  (`deny_unknown_fields`) で、`Project` の新規フィールドは
  `#[serde(default)]` がなければ v5 body を parse できない。

## Decision

### D1: `RoutingModel` は `synvelis-project::routing` module に配置

- 新規 Stable ID は `id.rs` の `define_id!` で追加:
  `BusId`/`StripId`/`ProcessorId`/`ChainId`/`SemanticPortId`/
  `RoutingConnectionId`/`SendId`。全て tagged serde
  (`"<kind>-<32hex>"`)・`u128`・Map key↔id 一致検証対象。
- `Project.routing: RoutingModel` を末尾フィールドとして追加し
  `#[serde(default)]` を付与(v5 body の parse に必要)。
- Ruviform に Synvelis 固有型を持ち込まない。`GraphSpec`/`NodeId`/
  `PortId` は再生成可能な Derived IR のまま。

### D2: 起動 Master は reserved well-known ID(well-known constants)

- `routing::WELL_KNOWN_BASE: u128 = 1 << 96` 以上を reserved 領域とし、
  `RoutingModel::default()` は Master Bus / Strip / Chain / 主入出力
  Port を `WELL_KNOWN_BASE + 1..=5` の固定値で生成する。
- **理由**: `Project::new` には generator がない。low range
  (1..=N) の bootstrap ID を使うと (a) `id_watermark` を N+1 へ進める
  必要が生じ、(b) `save()` が `generator.peek()` を watermark に記録
  するため、seed が小さい generator で save→load が watermark 検証に
  落ち、(c) generator clamp を `ProjectController::new` に追加すると
  全既存テストの初回発行 ID がずれて広範な回帰になる。reserved
  領域はカウンタ generator が実際に到達不能なため、衝突も発行値の
  ずれも発生しない。
- `max_issued_id_raw` は従来通り全 Map を fold するが、reserved 値
  (`>= WELL_KNOWN_BASE`) は「発行された ID」ではなく定数であり
  watermark 比較対象から除外する。`check_id_watermark` は追加で
  `id_watermark >= WELL_KNOWN_BASE` を拒否し、reserved 領域への
  generator 進入を検出する。
- well-known ID は Project 内で意味が固定されるため、Project 間で同一
  raw を持っても問題ない(ID は Project スコープ)。

### D3: Schema v6 = v5 + `routing`・単一 Versioned Migration Registry

- `SCHEMA_VERSION = 6`。`migration` module が `v → v+1` の順序付き
  step 配列を持つ Registry として `#17/#86/#92` の将来 step が統合
  できる最小構造を作る。SF 専用の並行 Store/Reader は作らない。
- `load()` は envelope を parse 後、`schema_version == 6` はそのまま、
  `== 5` は Registry の `migrate_v5_routing` を適用、その他は
  `UnsupportedSchema` を返す。
- `migrate_v5_routing`:
  - read-only 投影。`project.routing != RoutingModel::default()`
    (master のみ) の v5 body は契約外として `Validation` で拒否
    (v5 に routing field は存在し得ない = 偽造入力を黙って取り込まない)。
  - 既存 Track を `TrackId` 昇順で走査し、各 Track に
    `Strip(owner=Track)` + Chain + Main 入出力 Port +
    `strip.main_out → master.main_in` の `kind=Main` Connection を生成
    (= 旧 Clip→Master 相当経路)。
  - 新規 Stable ID は `base = max(全 issuable live ID, id_watermark)`
    の直上から連番割当(well-known 領域に達する場合は失敗)。同一
    Snapshot の再投影は常に同一結果(決定性)。
  - `id_watermark` を最終発行位置へ進める。外部 `insert_count`
    由来の FX は作成しない。
- Migration 適用 load では `live_revision = envelope.revision + 1` と
  し、`saved` は旧 HEAD(r5.json = v5 bytes)を指す。次の `save()` が
  必ず新しい snapshot 名(r6)を publish し、**旧 v5 snapshot bytes を
  決して上書きしない**(Immutable Snapshot 契約)。
  `Project` 内容が v5 snapshot と同一でないため、migration 自体を
  暗黙の 1 revision として扱う。

### D4: 削除の参照保護は writer 側 oracle(`validate_references`)に一本化

- `RemoveTrack`/`RemoveBus`/`RemoveProcessor` は自 op の validate で
  接続参照を再検査しない。所有物(Track→Strip→Chain→Port、
  Bus→Strip→Chain→Port、Processor→Port)のみ cascade 削除し、
  残存 Connection/Send/Automation target の dangling は
  `ProjectController::apply` 末尾の `validate_references` が検出して
  commit 全体を拒否する。
- **理由**: 現行 `ProjectOp::Composite::validate` は各 child を元
  Project に対して検証するため、op 側で「被参照なら拒否」を実装
  すると `[RemoveConnection, RemoveTrack]` の正当な composite まで
  拒否される。oracle に委ねれば (a) 単独削除 = 拒否、(b) 参照除去を
  含む composite = 受理、が既存セマンティクスのまま成立し、ルールの
  実装が一箇所(「Prefer one authoritative implementation」)に留まる。
- Master Bus の削除は oracle ではなく op validate で明示拒否
  (model rule: Master は厳密 1 件、削除は invalid)。

### D5: SF-01 の op 面は最小・oracle が意味合法性を保持

- 追加 `ProjectOp`: `AddBus`/`RemoveBus`/`AddTrackStrip`/
  `AddConnection`/`RemoveConnection`/`AddSend`/`RemoveSend`/
  `SetSendTap`/`SetConnectionEnabled`/`InsertProcessor`/
  `RemoveProcessor`/`ReorderProcessor`。
- `AddSend` は `kind=Send(SendId)` Connection + `SendConfig` を 1 op で
  同時作成(= 1 CAS + 1 Undo)。`AddConnection` の `kind` は
  `Main`/`Sidechain` のみ受理(`Send` は `AddSend` 専用で 1:1 を保証)。
- `AddTrack` は従来通り Track のみ生成(制作コンテナと DSP を分離)。
  `AddTrackStrip` が track の Strip + Chain + Ports + Master への Main
  接続を生成(Legacy parity の deterministic 既定経路)。
- op 側 validate は存在性・enum ガード等の明確な早期エラーのみ。
  direction/domain/owner/cycle/dup/Send 1:1 等の意味合法性は oracle
  (`routing::validate`) に一本化し、二重実装を避ける。
- **SF-02 接続点**: Temp Ref→Stable ID の `RoutingTransaction` は
  `Composite` ではなく独立 preflight/scratch staging を必要とする
  (元 Project 一括 validate では同一 gesture 内の新 ID 参照を解決
  できない)。SF-01 の op 面はその primitive として機能する。

### D6: `SendConfig.gain_target` は参照のみ・owner mapping は CTL-01 へ

- `gain_target: Option<ParameterTargetId>`。`Some` の場合は
  `control.targets` への存在のみを検証する。新規 Active Send の
  Connection/SendConfig/ControlTarget 同時作成と、
  `ControlOwner::Send` の owner mapping は CTL-01 #121 / SF-02 #105 の
  契約であり、SF-01 は Persisted Gain の第 2 正本を作らない形を
  維持する。

## Consequences

- **Positive**: Empty/New/Legacy v5 で Master 厳密 1 件。既存テストの
  発行 ID 不変(reserved 領域)。v5 snapshot bytes 保護。Migration は
  決定的かつ read-only。削除保護が composite で表現可能。
- **Negative/Deferred**: Mute/Solo・Temp Ref transaction・異 Layout
  channel map 承認フロー・Plugin capability 実検証・ControlOwner
  分離は後続 Issue。processor の `advertised` identity は SF-01 では
  role 名ベースであり、実 plugin port identity は PC-C4 で置換。
- **Risks**: reserved-base 方式は「全 Map を watermark fold」契約の
  文字通りの適用からわずかに外れるが、reserved 値は発行不能定数で
  あり invariant(発行済み live ID > watermark 禁止)は完全に保持。

## Alternatives Considered

- **low-range bootstrap + `ProjectController::new` clamp**: watermark
  を N+1 に進め generator を clamp する案。seed が小さい全テストの
  初回発行 ID がずれ、fixture/断言が広範に回帰するため不採用。
- **AddTrack が strip 自動生成**: Track=制作コンテナの責務分離と、
  発行 ID 数の変化による既存テスト回帰のため不採用。`AddTrackStrip`
  を明示 op にし、SF-02 の CreateTrack transaction が束ねる。
- **削除時に op validate で参照検査**: composite 正当ケースを殺す
  ため不採用(oracle 一本化へ)。
