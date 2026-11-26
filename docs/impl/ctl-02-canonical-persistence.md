---
id: "IMPL-MRw5ou5y3nnARQ1vIjPApO"
type: "impl"
status: "approved"
title: "CTL-02 canonical persistence implementation record"
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
    - "docs/plans/ctl-02-canonical-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/designs/ctl-02-canonical-persistence.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-02-canonical-persistence.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-canonical-persistence.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv122/canonical-persistence-verification.md"
  defers: []
  deferred-by: []
---

# CTL-02 canonical persistence implementation record

## 実装内容

- `crates/synvelis-project/src/store.rs` の `SCHEMA_VERSION` を v15 から v16 へ更新した。
- `crates/synvelis-project/src/control.rs` に、internal Strip/Send target の canonical spec 偏差を列挙する `canonical_internal_spec_deviations` と、v15 snapshot を正規化する pure projection `normalize_canonical_internal_spec` を追加した。canonical 定義の source of truth は既存の `canonical_internal_spec(owner, role)` のままである。
- `crates/synvelis-project/src/migration.rs` に deterministic な v15→v16 step を ordered ladder へ追加した。この step は対象 target の `kind`/`mapping`/`time_scope` を canonical 値へ揃え、`capability` を `None` にする。`id`/`owner`/`role`/`name`/`base`、automation lane 参照、send binding、`id_watermark` は変更しない。
- `crates/synvelis-project/src/project.rs` の `Project::validate_references` に canonical spec oracle を追加し、v16 以降の non-canonical internal target を target id・owner・role・外れた field を含む `Error::Validation` で fail-closed に拒否するようにした。
- `crates/synvelis-project/tests/ctl_02.rs` に v15 fixture の正規化、authored state/foreign target 保存、deterministic projection、v16 fixture の field 別 fail-closed、fresh/v9 migration oracle pass の検証を追加した。`cb01_variant.rs`/`tim_02_smf.rs`/`md_a.rs` の schema fixed assertion を v16 へ更新した。
- spec/ADR/design/plan/task/test-spec の docs chain を追加した。docs index は本 slice では更新しない。

## 実測サマリ

`mise run test synvelis-project --test ctl_02`（Windows, exit 0）で 21/21 pass。`mise run lint synvelis-project`（Windows, exit 0）、`mise run check synvelis-project`（Windows, exit 0）、`mise run check-headless-deps`（Windows, exit 0）。`mise exec -- cargo fmt --package synvelis-project -- --check` は補助証拠として exit 0。canonical `mise run fmt-check` は環境依存の filesystem path length error（`os error 206`）で収集不能であり、本 slice の変更差分ではない。

## 判断記録

- migration は authored `name`/`base` を上書きしない。canonical spec の対象外は既存 contract どおり free-form target と plugin/processor target に限定した。
- v16 body は修復せず reject だけを行う。黙った書き換えは v15→v16 migration の一度きりに限定した。
- SendLevel oracle は `SendConfig.gain_target` の有無に依存せず、`ControlOwner::Send` × `ParameterRole::SendLevel` の target 全般に適用した。orphan target の削除・cascade は追加していない。
- `Pan` は既存どおり seeded role ではなく first-use registration とし、この slice では seeding 方針を変更していない。

## 参照

- verification: `evidence/sv122/canonical-persistence-verification.md`

## 残件

GUI/Inspector/Mixer の表示分離、実 plugin binding、SourceMap/ControlTrace 統合、graph generation 照合、誤配送ゼロ実測、channel mismatch approval flow、Realtime/Offline/2Window、AudioLens 非回帰、EXT-L3/GUI-L2/3OS/provisioned runner、docs index 整備（Coordinator）。
