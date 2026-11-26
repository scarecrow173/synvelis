---
id: "SPEC-gtjTcKhGLn9Po50wiKWkuC"
type: "spec"
status: "approved"
title: "CTL-02 canonical persistence — v15→v16 migration と load oracle"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
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
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-02-canonical-persistence.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/adr/ctl-02-canonical-persistence.md"
    - "docs/designs/ctl-02-canonical-persistence.md"
  defers: []
  deferred-by: []
---

# CTL-02 canonical persistence — v15→v16 migration と load oracle

## What Should Be Built

Issue #122（CTL-02）の persistence slice として、Strip/Send の内部
Host Parameter target が persisted state で canonical spec から外れない
ようにする。対象は `ControlOwner::Strip` × `Fader`/`Pan`/`Mute`/`Solo`
と `ControlOwner::Send` × `SendLevel` である。

現行 main は `AddParameterTarget` の write path と
`AddSend`/`SetSendGainTarget` の adoption path を既に防御している。
しかし v10→v15 era に合法だった snapshot には非 canonical spec が残り
得る。例えば master fader は v7→v8 migration で既存 target の
`kind`/`mapping`/`time_scope`/`capability` を保持したまま well-known ID
へ re-key され得る。既存 snapshot を正規化する migration がなく、
`Project::validate_references` も canonical spec を oracle として
検査していない。

本 slice は schema version を v15 から v16 へ上げ、ordered migration
ladder に v15→v16 step を追加する。この step は内部 target の固定 spec
フィールド `kind`/`mapping`/`time_scope` を
`canonical_internal_spec(owner, role)` の値へ決定論的に正規化し、
`capability` を `None` にする。`id`/`owner`/`role` は identity、
`name`/`base` は authored state なので保持する。free-form target と
plugin/processor target は変更しない。

同じ invariant を `Project::validate_references` に追加する。v16 以降の
current snapshot と post-apply state で canonical spec から外れた内部
target を `Error::Validation` で fail-closed に拒否する。load 時に黙って
修復するのは v15→v16 migration だけであり、v16 body の異常は修復
しない。

## Why Now

`docs/specs/ctl-02-adopt-sendlevel.md` は既存 persisted snapshot の
load-time 挙動と `validate_references` 拡張を Deferred Design Concerns
として明示的に残した。この slice は、その deferred な headless 実装だけを
閉じる。write path と adoption path を防御しても、古い snapshot が非
canonical spec を持ち込む限り、Save/Reload の 1 Stable Parameter 正本は
成立しない。

## Users and Value

- `synvelis-project`: persisted control model が current schema で一意の
  internal target spec を持つ。lowering、authority lookup、 automation
  evaluation が未定義の spec shape を読まない。
- 上位の mixer/send UI と routing transaction: reload 後も同じ target ID
  と authored value を前提にできる。migration が ID を採番せず、send
  binding と automation lane を書き換えない。
- 後続の GUI/EXT-L3 gate: persisted state の headless invariant が先に
  fail-closed になるため、外部 gate の観測と persisted spec の問題を
  分離できる。

## Scope

### In Scope

- `SCHEMA_VERSION` を 16 へ上げ、migration ladder に v15→v16 step を
  追加する。
- `control.rs` の `canonical_internal_spec` を単一 source として使う
  canonical deviation oracle と migration normalization helper を追加する。
- v15→v16 step で内部 target の `kind`/`mapping`/`time_scope` を canonical
  値へ、`capability` を `None` へ正規化する。`id`/`owner`/`role`/
  `name`/`base`、automation lane、send binding、watermark は保持する。
- `Project::validate_references` に同じ invariant の oracle を追加し、
  target id、owner、role、外れたフィールドを error message に含める。
- `crates/synvelis-project/tests/ctl_02.rs` に v15 fixture、v16 reject
  fixture、fresh/v9 migration の oracle pass fixture を追加する。
- `cb01_variant.rs`、`tim_02_smf.rs`、`md_a.rs` の `SCHEMA_VERSION` 固定
  期待値を 16 へ更新する。
- docs chain と `evidence/sv122/` を追加する。

### Out of Scope

- GUI / Inspector / Mixer の表示分離、Pending/Blocked/Observed surfacing。
- 実 plugin binding、SourceMap/ControlTrace 統合、Graph generation 照合、
  誤配送ゼロの実測。
- Channel mismatch approval flow、Realtime/Offline/2Window、AudioLens
  非回帰。
- routing transaction / atomic send creation の変更。既存 atomicity test の
  回帰確認のみ。
- orphan/unreferenced target の削除や cascade 仕様の追加。
- EXT-L3 / GUI-L2 / 3OS / provisioned runner。
- `docs/**/README.md` index 更新。Coordinator が統合時に整備する。

## Requirements

### CTL-02P-R1: deterministic v15→v16 migration

- migration は pure projection であり、ID を採番せず `id_watermark` を
  変更しない。
- `canonical_internal_spec(owner, role)` が定義する内部 target について、
  `kind`/`mapping`/`time_scope` を canonical 値へ設定し、`capability` を
  `None` にする。
- `name`/`base`/`id`/`owner`/`role` を保持する。automation lane、
  modulation assignment、send binding、watermark を変更しない。
- free-form target と plugin/processor target を変更しない。
- 同じ v15 bytes は常に同じ v16 project へ投影される。

### CTL-02P-R2: current-state oracle

- `Project::validate_references` は内部 target の canonical spec deviation
  を `Error::Validation` で拒否する。
- error message は target id、owner、role、外れたフィールド名を人間が
  追える形で含める。
- v16 snapshot は修復せず拒否する。黙った正規化は v15→v16 migration
  に限定する。
- SendLevel oracle は `SendConfig.gain_target` の有無に依存せず、
  `ControlOwner::Send` × `ParameterRole::SendLevel` の target 全般に
  適用する。

### CTL-02P-R3: single policy source

- migration と oracle は `control.rs` の canonical spec 定義を共有する。
  政策を migration と oracle で二重実装しない。
- `canonical_internal_spec` の対象外フィールドは `name`/`base` のみと
  する。`base` は mixer/send level の値正本なので migration が上書き
  しない。

### CTL-02P-R4: non-regression

- fresh project と v9→v16 migration は canonical oracle を pass する。
- 既存 strip seeding、first-use `Pan`、send create atomicity test は非回帰
  である。
- migration ladder は v15 を受け入れ、v16 より新しい version を
  `UnsupportedSchema` で拒否する既存挙動を維持する。

## Acceptance Criteria

- [ ] v15 snapshot に置かれた非 canonical internal strip target と非
  canonical `SendLevel` が load/migration 後に canonical spec へ正規化
  され、`name`/`base`/ID/owner/role が保持される。free-form/plugin
  capability target は untouched である。
- [ ] v16 snapshot で内部 target の `kind`/`mapping`/`time_scope`/
  `capability` を 1 件ずつ外した fixture が load oracle で fail-closed に
  拒否される。
- [ ] fresh project と v9→v16 migration が canonical oracle を pass し、
  既存 CTL-02 tests が非回帰である。
- [ ] `SCHEMA_VERSION` fixed assertions が v16 に更新され、v15 acceptance
  と newer-version rejection の既存挙動が維持される。
- [ ] 指定された canonical verification lane が実行され、結果が evidence
  に記録される。環境起因で実行不能な lane は Pass に見せかけず、理由と
  補助証拠を区別する。

## Deferred Design Concerns

- GUI / Plugin / Domain Oracle / AudioLens / Channel mismatch approval は
  後続 gate で扱う。
- schema version bump に伴う branch content payload の運用は Creative
  Branch 側の既存契約に委ねる。本 slice は project snapshot の control
  model のみを対象にする。
- orphan/unreferenced target の存否は既存 invariant に委ね、新しい削除
  仕様は追加しない。

## Sources and Relations

- Issue #122: CTL-02 の受入条件と残件。
- `docs/specs/ctl-02-adopt-sendlevel.md`: load-time 挙動と
  `validate_references` 拡張を deferred した先行 spec。
- `docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md`:
  `ControlOwner`/`AutomationTimeScope`/`ParameterRole` の typed schema。
- `docs/adr/ctl-02-canonical-persistence.md`: legacy snapshot は migration
  で一度だけ正規化し、current snapshot は reject する決定。
