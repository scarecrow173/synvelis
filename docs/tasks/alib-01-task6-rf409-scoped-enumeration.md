---
id: "TASK-SV85RF409ScopedEnum9zQ2"
type: "task"
status: "done"
title: "ALIB-01 task6 — ruviform#409 scoped re-root / bounded 列挙への正規契約移行"
created: "2026-11-07"
updated: "2026-11-07"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
    - "https://github.com/scarecrow173/ruviform/issues/409"
    - "https://github.com/scarecrow173/ruviform/pull/412"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-01-asset-provider-baseline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-01-asset-provider-baseline.md"
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/designs/alib-01-asset-provider-contract-design.md"
    - "docs/test-specs/alib-01-asset-provider-contract.md"
    - "docs/tasks/alib-01-task5-rooted-cutover.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# ALIB-01 task6 — ruviform#409 正規契約への移行

task5 が残した 2 件の BLOCKED/暫定実装を、ruviform#409（PR412、
main `d0ed6208`、後続 `79d1db0a`）が提供する正規契約で置き換える。

- ruviform 依存を main HEAD `79d1db0a` へ更新（既定 main 追従
  git dep、`cargo update -p ruviform-io-file --precise` で 26
  source を単一 rev に統一。`Cargo.lock`・`ui/vendor/ruviform/REV`
  は `tools/check-ruviform-rev.mjs` の検査で一致を確認）。
- `open_scope` の暫定実装（保持 root `metadata(WithinRoot)` の
  権威解決 + `canonical_root` pathname への `RootedDir::open_with`
  再取得 + `dir_identity`/`NodeId` same-object 照合 +
  `IdentityStrength::Strong` gate）を撤去し、保持 `RootedDir` の
  `open_scoped_root(rel, WithinRoot)` へ置き換え。権威は解決済み
  directory handle 自体が capability floor になる — pathname
  再 open の window が存在せず、`NodeId` 照合も不要（weak
  identity FS を gate しない。未固定データを固定扱いしない AC1
  との整合: capability が address するのは解決 object であり
  名前ではない）。`open_dir` は `WithinRoot` 境界が provider
  root のままで scope 内 link 判定を弱めるため引き続き不使用。
- scope の公開 `base` prefix は、scope の handle-derived
  canonical を held root の *現在の* handle-derived canonical
  （`open_scoped_root("")` で再 anchor）から strip して求める。
  両辺とも handle 由来のため root rename 後も locator は
  canonical-relative を維持し、prefix 不成立は
  `PermissionDenied` で fail-closed。
- `collect_entries` の `read_dir`（全件 `Vec` materialize）を
  `read_dir_each`（callback 列挙 + `EntryFlow::Stop` /
  `EnumerationOutcome`）へ置き換え。kept window
  （`remaining+1`、旧 `take(remaining+1)` と同じ enumeration
  order prefix）を visitor 内で収集してから sort — emit/subset
  意味論は同値で、列挙そのものの memory/work bound と
  cancel/deadline の列挙段階適用が回復（task5 の BLOCKED 残余
  を解消）。visitor 内で `cx.check_interrupts()` を観測し
  `Stop` → `Cancelled`/`BudgetExceeded` を返す。
- `tests/containment_swap.rs` を新契約へ更新:
  `local_scope_acquisition_swap_cannot_escape_root`（tag を
  `"open"` へ — scoped acquisition が唯一の touch point）、
  `local_scope_open_swap_cannot_escape_root` を
  `local_scope_post_acquisition_swap_keeps_held_entries` へ
  置換（取得後の scope dir swap は held handle が透過 —
  旧 metadata/open_with 間 window は契約上存在しない）、
  `local_scope_root_replacement_cannot_rebind_capability` の
  期待を `SourceChanged` から「held tree を
  `sub/inner.wav` で serve・replacement の `out-secret.wav`
  を出さない」へ更新（handle authority の正規意味論）。
  seam sanity の期待 op 列は `["open","read_dir","stat"]`。
- `tests/local.rs` に列挙途中 cancel の回帰
  `cancel_during_directory_enumeration_fails_the_page` を追加
  （`"read_dir"` barrier で token cancel → visitor が
  `Cancelled` を返す）し、dir entry の `byte_len` が
  `Unknown` であることを明示 assert。
- spec/design/test-spec/plan/impl record を実態へ更新。

## 検証

- `cargo test -p synvelis-library` — containment_swap 11/11、
  local 34、project 21、contract 2、types 15 全 pass。
- `mise run fmt`/`fmt-check`/`check`/`lint`/`test` 全 workspace、
  `check-ruviform-rev`（resolved `79d1db0a`・26 sources・REV
  match）・`check-headless-deps`。
- `mise run test-msrv` は task5 時点と同じ既存 main 由来の
  不整合（`cargo-platform@0.3.3` が rustc 1.91 要求）で fail —
  本差分の `Cargo.lock` delta に当該 pkg なし。別途 follow-up。

## 互換差（task5 からの差分）

- locator scope 付き browse/search の weak-identity FS
  `UnsupportedCapability` gate は撤去 — `open_scoped_root` は
  `NodeId`/strong identity を要求しないため、weak backend でも
  scoped browse は動作する（root 置換 rebind は handle authority
  で構造的に不可能）。
- root rename + 同一 pathname 別 tree 設置時、locator scope
  browse は `SourceChanged` ではなく **held tree を serve する**
  — RF409 の正規意味論（held handle が同一 directory object を
  address し続け、replacement は別 object で capability が
  到達しない）。locator は handle-derived anchor 基準の
  canonical-relative を維持。in-root の scope dir
  rename/recreate も同様に acquisition 時点の object を serve
  する（旧実装の `SourceChanged` 厳格化は pathname 再 open の
  補償であり、正規契約では不要）。
- scope 取得の fault-barrier tag は `"stat"`+`"open"` の 2 地点
  から `"open"` 1 地点へ（touch point が `open_scoped_root`
  のみに集約）。
- `read_dir_each` は `read_dir` と同一 entry 規則（no-follow
  kind、`.`/`..` 非報告、順序は backend 依存）で、列挙打切りは
  `EnumerationOutcome::Stopped` — SV 側は kept window bound と
  interrupt のみを観測し outcome 区別は使わない。
