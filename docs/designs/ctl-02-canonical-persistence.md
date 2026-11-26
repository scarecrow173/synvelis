---
id: "DESIGN-kPeDqP4gwqUY0r89uRpuRV"
type: "design"
status: "approved"
title: "CTL-02 canonical persistence 設計"
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
  implements:
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/adr/ctl-02-canonical-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/adr/ctl-02-canonical-persistence.md"
    - "docs/specs/ctl-02-adopt-sendlevel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-canonical-persistence.md"
    - "docs/adr/ctl-02-canonical-persistence.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  defers: []
  deferred-by: []
---

# CTL-02 canonical persistence 設計

## Context

`crates/synvelis-project/src/control.rs` の `canonical_internal_spec` は
internal channel/send target の canonical `(kind, mapping, time_scope)` を
返す。capability `None` の条件は同じ contract の一部として write path
と adoption path が検査している。snapshot migration と load oracle は
この契約をまだ適用していない。

本設計は migration と oracle が同じ policy を共有し、legacy snapshot の
usable state を保ちながら current state を fail-closed にする境界を定める。
編集範囲は `synvelis-project` の control/migration/project/store と、指定
された test/docs/evidence に限る。

## Scope

### In Scope

- `canonical_internal_spec` を参照する read-only deviation helper と、
  migration 専用の normalization helper を `control.rs` に追加する。
- migration ladder に v15→v16 step を追加する。
- `Project::validate_references` の target loop に canonical oracle を
  追加する。
- `SCHEMA_VERSION` を 16 にする。
- ctl_02 fixture と schema version 固定 test を更新する。

### Out of Scope

- `ops.rs` と `routing_tx.rs` の変更。write/adoption path は既存の防御を
  使い、本 slice は persistence boundary だけを閉じる。
- orphan/unreferenced target の削除、cascade、free-form target への適用。
- GUI、plugin runtime、EXT-L3、3OS、provisioned runner。

## Components and Boundaries

### canonical policy helpers（`control.rs`）

```rust
pub(crate) fn canonical_internal_spec_deviations(
    target: &ParameterTarget,
) -> Vec<&'static str>

pub(crate) fn normalize_canonical_internal_spec(
    target: &mut ParameterTarget,
) -> bool
```

`canonical_internal_spec_deviations` は target が internal contract の
対象かどうかを `canonical_internal_spec(target.owner, target.role)` で
判定し、`kind`/`mapping`/`time_scope`/`capability` の順に外れた
フィールド名を返す。free-form target と plugin/processor target では空を
返す。

`normalize_canonical_internal_spec` は同じ deviation 判定を使い、内部
target だけの固定フィールドを canonical 値へ書き戻す。`id`/`owner`/
`role`/`name`/`base` は触らない。戻り値は変更有無だが、v15→v16 step は
全 target を走査するだけでよく、分岐には使わない。

この 2 つの helper は policy を新しく定義しない。判定対象と canonical
値は `canonical_internal_spec` に委ねる。capability `None` の条件だけを
別表に複製しないため、deviation helper が `capability.is_some()` を
canonical internal target に限って報告する。

### v15→v16 migration（`migration.rs`）

```rust
fn migrate_v15_canonical_internal_specs(mut project: Project)
    -> Result<Project, Error>
```

- `project.control.targets.values_mut()` を走査し、
  `normalize_canonical_internal_spec` を呼ぶ。
- ID を採番しない。`id_watermark` を変更しない。
- lane、assignment、send binding を書き換えない。target ID が不変なので
  これらの参照は自然に保持される。
- deterministic pure projection とする。同じ input bytes に対し map iteration
  順序は `BTreeMap` で安定し、出力も同じになる。

migration は validation 前に走る。capability が不正な値を持っていても、
internal target の capability を落とすことで legacy usable state を救済
できる。`base` が不正な場合は authored state の異常であり、正規化せず
後段の既存 oracle が拒否する。

### load/post-apply oracle（`project.rs`）

`validate_references` の target loop で、kind/base/capability の既存検査に
加えて `canonical_internal_spec_deviations` を呼ぶ。deviation があれば
`Error::Validation` を返す。message は次を含める。

- target id
- owner（Debug 表記）
- role（Debug 表記）
- 外れたフィールド名の comma-separated list

検査位置は capability detail の検査前に置く。v16 body で capability
`Some` を持つ internal target は、capability の中身が valid/invalid に
関わらず canonical deviation として報告される。v15 body は migration が
先に capability を落とすため、この oracle には到達しない。

### schema version（`store.rs`）

`SCHEMA_VERSION` を 16 にする。v16 は serialized field の追加ではなく、
persisted internal spec の canonicalization を意味する semantic advance
である。`SnapshotEnvelope` の形は変えない。

## Data and State Invariants

- Internal target invariant: `canonical_internal_spec(owner, role)` が
  `Some` を返す target は、current state で canonical kind/mapping/
  time_scope/capability None を持つ。
- Authored state invariant: `name`/`base` は migration が保持する。
- Identity invariant: `id`/`owner`/`role` は migration が保持する。
- Reference invariant: target ID が変わらないため、lane/assignment/send
  binding は migration が触れなくても保持される。
- Foreign-state invariant: v16 以降の deviation は修復せず reject する。

## Error Handling

- v15→v16 migration 自体は deterministic field projection で失敗しない。
  `Result` は ladder の uniform shape に合わせる。
- v16 以降の deviation は `Error::Validation`。
- v16 より新しい schema は従来どおり `Error::UnsupportedSchema` /
  `SnapshotError::UnsupportedSchema` 系で reject する。

## Test Strategy

1. v15 fixture: master fader、通常 strip bool role、SendLevel を非 canonical
   にし、migration 後の canonical spec、authored `name`/`base`、ID/owner/
   role、lane、send binding、watermark、free-form/plugin target untouched、
   deterministic reload を確認する。
2. v16 reject fixture: master fader の `kind`/`mapping`/`time_scope`/
   `capability` を 1 件ずつ外し、load oracle が `Validation` で拒否し、
   message に id/owner/role/field が入ることを確認する。
3. fresh/v9 fixture: fresh project と v9→v16 migration が oracle を pass
   することを確認する。既存 strip seeding、first-use Pan、send atomicity
   tests の非回帰を package test で確認する。
4. schema fixed assertions を v16 に更新し、v15 acceptance/newer rejection
   の既存 test が通ることを確認する。

## Verification Plan

正規コマンドは mise task とする。対象 crate は `synvelis-project`。

- `mise run fmt-check`
- `mise run check synvelis-project`
- `mise run lint synvelis-project`
- `mise run test synvelis-project --test ctl_02`
- `mise run test synvelis-project`
- `mise run test-rt`
- `mise run check-headless-deps`

workspace-wide `cargo fmt` が Windows path/command length 制約で起動不能
な場合は、Pass に見せかけず environmental failure として evidence に
記録し、対象 package scoped fmt を補助証拠に留める。
