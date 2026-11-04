---
id: "ADR-034CTL01Px0vYs6zB2cD3eFg"
type: "adr"
status: "accepted"
title: "CTL-01 ControlOwner/AutomationTimeScope 責務分離・Schema v7・Versioned Migration 境界"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  implemented-by:
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  depends-on:
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
    - "docs/adr/pc-a-parameter-control-placement-project-model-workspace-session.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# CTL-01 ControlOwner/AutomationTimeScope 責務分離・Schema v7

## Status

Approved — Issue #121 の 2026-11-04 合意に基づく初期実装。

## Context

- `ControlScope` は owner と automation 点の時刻座標を同一 enum
  (tag `scope`)で保持しており、SF-01 の Bus/Strip/Send/Processor
  Stable ID を所有する形がない。
- Migration ladder は `fn(Project) -> Result<Project, Error>` の
  typed step であり、step 適用は `serde_json` での `SnapshotEnvelope`
  全量デシリアライズ**後**に走る。v6 body が新 `Project` 型に
  deserialize できることが v6→v7 step の前提条件。
- 合意済み方針: pre-release のため旧 `ControlScope` との実行時
  互換層は作らず、形式は Versioned Migration で作り直す。

## Considered Options

- **Option A(採用)**: owner と time_scope を別 typed field として
  `ParameterTarget` に永続化し、Schema v7 migration で旧
  `ControlScope` を投影。型として責務が分離され、Bus/Strip/Send/
  Processor 所有が型付きに表現できる。
- **Option B(却下)**: `ControlScope` に `Strip`/`Send`/`Processor`
  variant を追加して継続使用。owner と座標の混在が残り、Strip 所有
  で project 時刻の target を型安全に表せないため不採用。
- **Option C(却下)**: raw JSON を書き換える別 Migration Reader を
  追加。`fn(Project) -> Project` の typed ladder と
  `SnapshotEnvelope` 一経路 deserialize の単一正本契約を壊すため
  不採用。

## Decisions

### D1: owner と time_scope を別フィールドとして永続化する

`ParameterTarget` は `owner: ControlOwner` + `role: ParameterRole` +
`time_scope: AutomationTimeScope` を持つ。`ModulationSource` は
`owner: ControlOwner` のみ(source signal は assignment 先 target の
解決済み座標で評価される既存契約のため)。`ControlScope` 型は削除し、
評価時の座標解決は `resolve_scope_pos(project, owner, time_scope, pos)`
が担う。

### D2: `ControlOwner` に Bus 変種を作らない

Bus の音声 Parameter は `StripOwner::Bus(BusId)` の指す Strip に
`ControlOwner::Strip` で binding する。Bus は論理経路であり DSP 値の
正本ではない — Bus 単位の独自 gain 永続化を防ぐ。

### D3: Schema v7 の migration は既存 ladder の typed step とする

`SCHEMA_VERSION = 7`、`STEPS` に `{ from: 6, migrate_v6_control }` を
追加する。専用 Reader / 並行 migrator は作らない(#17/#86/#92 と同一
registry)。step は owner を identity に、`time_scope` を
`Clip→ClipLocal`/`Note→NoteLocal`/その他→`ProjectMusical` に、
`role` を `Generic` に投影する決定的変換であり、新規 ID 採番・
`id_watermark` 更新は行わない。

### D4: v6 body の deserialize 互換は `ControlOwner` の wire 受理のみ

`owner` フィールドは `{"owner": "...", "id": ...}` が新形式。
`ControlOwner::Deserialize` は migration 境界として旧 tag `scope`
(`{"scope": "Track", "id": ...}`)を受理する — これは実行時互換層
ではなく、ladder step が受け取る typed `Project` を作るための
reader-side 受理であり、serialize は常に新形式のみを書く。
`time_scope`/`role` は `#[serde(default)]` で v6 body の欠落を許し、
step が厳密に導出する。v7 body での欠落は default に倒れるが、
`validate_references` の owner×time_scope 組合せ検査が非法値を拒否
する。

### D5: owner×time_scope 組合せは validation matrix で規定する

- `ClipLocal` は `ControlOwner::Clip` のみ、`NoteLocal` は
  `ControlOwner::Note` のみ有効。
- `Project`/`Track`/`Strip`/`Send`/`Processor` は `ProjectMusical` または
  `ProjectAbsolute`。
- `ProjectAbsolute` は型のみ導入する。absolute-time point 型が未導入の
  ため評価は明示 `ScopeUnavailable` を返す(暗黙の beats 解釈はしない)。

### D6: (owner, role) 一意性は singleton role のみに適用する

`Fader`/`Pan`/`Mute`/`Solo`/`SendLevel` は owner ごと 1 件。
`Generic`(既存 migration 先・汎用 param)と `ProcessorParam`
(plugin param は 1 processor に複数あり得る)は重複を許す。

### D7: `SendConfig.gain_target` の owner mapping を検証する

`Some(target)` は `Project::control` に存在するだけでなく
`owner == ControlOwner::Send(send.id)` を要求する。Send 作成時の
atomic target 生成は SF-02 #105 の RoutingTransaction に委ね、本件は
不変条件の検証のみ。

## Consequences

- 既存 v6 snapshot は ladder 経由で座標厳密保持のまま v7 化できる。
- `ControlScope` API・wire 形は残らず、downstream(gateway/workspace/
  テスト)は新フィールドに追随する。
- `ProjectAbsolute` の実座標は後続 Issue に明示 defer する。
