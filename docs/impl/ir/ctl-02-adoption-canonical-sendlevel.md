---
id: "IMPL-034WBaliNYvopY8qIaeCeh"
type: "impl"
status: "completed"
title: "CTL-02 adoption 経路の非 canonical SendLevel 抑止"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/ctl_02.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/ctl-02-adopt-sendlevel-task.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
    - "docs/plans/ctl-02-adopt-sendlevel-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md"
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
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

# CTL-02 adoption 経路の非 canonical SendLevel 抑止

## Summary

Issue #122 残存観測 slice。`AddSend`/`SetSendGainTarget` の
adoption は apply 時に既存 target の `owner`/`role` のみを
`Send`/`SendLevel` へ書き換え、`kind`/`mapping`/`time_scope`/
`capability` を書き換えないため、canonical spec を持たない
target を adopt すると非 canonical な `SendLevel` が persisted
され得た。両 op の validate に canonical spec 検査を追加し、
非 canonical adopt を `Error::Validation` で reject するように
した。routing transaction 経路(`SendGainSpec::Adopt` /
`RoutingEdit::SetSendGainTarget`)は同一 op を経由するため追加
変更なしで同じ契約を継承する。

## Context

前 slice(PR#350 / squash `01ecd9c`)で `AddParameterTarget` の
validate が `Send`+`SendLevel` の canonical spec
(`Continuous`/`Linear`/`ProjectMusical`/`capability: None`)を
強制済みだったが、adoption は既存 target を再所有する経路であり
`AddParameterTarget` を通らない。先行 impl record
(`ctl-02-strip-send-host-parameter-plugin-binding-routing.md`)の
「残存観測」が本 slice の対象を名指ししていた。

## Implementation

### Changes

- `control.rs`: canonical SendLevel spec の tuple を
  `send_level_spec()` として抽出し、`canonical_internal_spec` の
  `Send`+`SendLevel` 分岐と共有化。`has_canonical_send_level_spec`
  を新設し、target の `kind`/`mapping`/`time_scope` が canonical
  tuple と一致し `capability.is_none()` であることを判定する。
- `ops.rs`: `ProjectOp::AddSend` と `ProjectOp::SetSendGainTarget`
  の validate に `has_canonical_send_level_spec` 検査を追加。
  既存の well-known ID・project-time・重複 binding・owner guard
  (Strip/Processor 所有拒否)の後、apply による owner/role 書換え
  より前に reject する。`name`/`base` は従来どおり authored の
  まま保持される。
- `tests/ctl_02.rs`: `add_free_target` helper と
  `AdoptFixture`/`adopt_fixture`/`main_port_id` を追加し、
  `adopted_send_gain_targets_reject_non_canonical_specs`・
  `canonical_send_gain_targets_adopt_and_seal`・
  `canonical_send_gain_targets_adopt_via_routing_intent` の
  3 テストを追加。非 canonical 4 spec(誤 kind/誤 mapping/
  `ProjectAbsolute`/capability Some)× low-level 2 op × intent
  2 経路の reject と serialized project 不変、canonical spec の
  全経路受理・owner/role seal・`name`/`base`/spec 保持・
  `send_level_target_id` 解決を fixture 化。

### Decisions During Implementation

- **reject を選択(正規化しない)**: design で検討した 2 案のうち
  reject を採用。理由は (1) CTL-02 の write-time 契約が
  `AddParameterTarget` で既に reject であり adoption だけ正規化
  すると契約が二重化する、(2) 正規化は authored spec を黙って
  書き換えるため persisted 意図の改竄になり得る、(3) adoption
  対象は internal contract を満たすべきであり authored のまま
  受理すべき自由 spec ではない、の 3 点。詳細は
  `docs/adr/ctl-02-adopt-sendlevel.md`。
- **canonical tuple の単一 source 化**: 直接作成
  (`canonical_internal_spec`)と adoption 検査が同一 tuple を
  参照するよう `send_level_spec()` に集約し、将来の spec 変更が
  片方だけに反映される drift を防ぐ。
- **`ProjectAbsolute` の reject**: 既存の「project-time」検査は
  `ProjectMusical|ProjectAbsolute` を許容するが、canonical
  SendLevel は `ProjectMusical` のみのため、新検査で
  `ProjectAbsolute` も reject される。これは意図的な契約の
  狭窄であり design/test-spec に記録済み。

## Related Experiments

### Adopted

なし(既知解の機械的実装のため exp は作成しなかった)。

### Rejected

なし。

## Validation

- `mise exec -- cargo test -p synvelis-project --test ctl_02`:
  14 tests pass(新規 3 件を含む、既存 11 件は非回帰)。
- `mise exec -- cargo test -p synvelis-project`: 全 suite green
  (40 test binaries、failure なし)。
- canonical 9 lane(Windows、rustup default toolchain、
  `Cargo.lock` locked)— すべて exit 0:
  - `mise run fmt-check` — `cargo fmt --all -- --check` exit 0
  - `mise run check` — `cargo check --workspace --all-targets` exit 0
  - `mise run lint` — `cargo clippy --workspace --all-targets --
    -D warnings` exit 0(初回 `clippy::too_many_lines` を
    fixture 分割で解消後に green)
  - `mise run test` — `cargo test --workspace --locked` exit 0
    (221 test-result-ok、failure なし)
  - `mise run test-rt` — `cargo test --workspace --locked rt_`
    exit 0
  - `mise run check-app-native` — `cargo check -p synvelis-app
    --features native --all-targets` exit 0
  - `mise run lint-app-native` — `cargo clippy -p synvelis-app
    --all-targets --features native -- -D warnings` exit 0
  - `mise run test-msrv` — `mise exec rust@1.89.0 -- cargo test
    --workspace --locked` exit 0
  - `mise run build` — `cargo build --workspace --locked` exit 0
- 実 Plugin/OS・実機受入・#103/#109 Domain Oracle・AudioLens・
  実 Plugin verified binding・Device Oracle: **NotRun**(Issue
  #122 の外部 gate、本 slice 対象外)。

## Risks

- 既に persisted された非 canonical `SendLevel` を持つ snapshot
  の load 時挙動は本 slice の対象外。`validate_references` は
  owner/role の整合のみを検査し spec canonical 性は検査しない
  ため、legacy データが残る場合の扱いは別 slice の判断事項。
- adoption 対象として `Generic` 以外の role(Fader/Pan 等)を
  持つ target も canonical spec を満たせば受理される。role は
  apply で `SendLevel` に上書きされるため persisted 結果は
  canonical であり、契約上の問題はない。

## Follow-ups

- Issue #122 の残件: 外部 gate(実 Plugin/OS・#103/#109 Domain
  Oracle・AudioLens・実 Plugin verified binding 受入・
  Device Oracle)は別 slice/Gate で継続。Issue は Open 継続。
- snapshot load 時の既存 persisted 非 canonical `SendLevel`
  への対処(load-time oracle/migration)は必要なら別 slice。
