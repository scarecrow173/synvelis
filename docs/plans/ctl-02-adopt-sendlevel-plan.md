---
id: "PLAN-3o92uvIdnyfhflikf7TlYQ"
type: "plan"
status: "approved"
title: "CTL-02 adoption SendLevel canonical spec 強制 実装計画"
created: "2026-11-13"
updated: "2026-11-13"
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
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
    - "docs/adr/ctl-02-adopt-sendlevel.md"
  derived-by: []
  verifies:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  verified-by:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  references: []
  defers: []
  deferred-by: []
---

# CTL-02 adoption SendLevel canonical spec 強制 実装計画 Implementation Plan

## Goal

`AddSend`/`SetSendGainTarget` の adoption で非 canonical `SendLevel`
が persisted される経路を、op validate の fail-closed 検査で塞ぐ。
canonical spec の定義は `control.rs` に単一 source を置き、
`AddParameterTarget` 検査と共有する。他 crate・manifest・
`SendGainSpec::Create` 受理は変更しない。

## Scope

- In: `crates/synvelis-project/src/control.rs`・`src/ops.rs`・
  `tests/ctl_02.rs`、docs チェーン、触れた dir の管理 index。
- Out: spec の Out of Scope と同一。

## Upstream Documents

- spec: `docs/specs/ctl-02-adopt-sendlevel.md`
- adr: `docs/adr/ctl-02-adopt-sendlevel.md`
- design: `docs/designs/ctl-02-adopt-sendlevel-design.md`
- test-spec: `docs/test-specs/ctl-02-adopt-sendlevel.md`

## Design Inputs

- `send_level_spec()` + `has_canonical_send_level_spec()` を
  `control.rs` に追加し、`canonical_internal_spec` の
  `Send`+`SendLevel` 分岐を同一定数に揃える — design doc が正本。
- `ops.rs` の `AddSend`/`SetSendGainTarget` validate の
  `gain_target: Some` ブロックに canonical spec 検査を追加
  （既存 guard 列の後・`Error::Validation`）。
- 先例: `strip_channel_targets_require_the_canonical_spec` /
  `send_level_targets_require_the_canonical_spec`（`ctl_02.rs`）の
  `NON_CANONICAL_SPECS` deviation 網羅と `expect_target_reject`
  流儀。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-project/src/control.rs` | `send_level_spec()`・`has_canonical_send_level_spec()` 追加、`canonical_internal_spec` を共有化 |
| `crates/synvelis-project/src/ops.rs` | `AddSend`/`SetSendGainTarget` validate に canonical spec 検査追加 |
| `crates/synvelis-project/tests/ctl_02.rs` | adoption canonical 契約 fixture 追加 |
| `docs/impl/ir/ctl-02-adoption-canonical-sendlevel.md` | 実装記録（完了時） |
| `docs/{specs,adr,designs,plans,tasks,test-specs}/README.md` | 管理 index 再生成（doc-maintenance） |

## Implementation Sequence

- [x] `control.rs`: `send_level_spec()`/`has_canonical_send_level_spec()`
      追加 + `canonical_internal_spec` の Send 分岐を共有化。
- [x] `ops.rs`: 両 validate 腕に canonical spec 検査を追加。
- [x] `tests/ctl_02.rs`: 非 canonical adopt reject（4 deviation +
      ProjectAbsolute、低層 op・`RoutingEdit` 両経路）と canonical
      adopt 受理の fixture 追加。
- [x] 補助検証: `cargo test -p synvelis-project` で対象 test と
      既存 ctl_02/ctl_01/sf_02/routing_sf01 の非回帰確認。
- [x] canonical 9 lane 実行 + impl record + index 再生成。

## Dependencies and Risks

- `SendGainSpec::Create`・既存 canonical adopt・他 role/owner
  利用は無変更 — 影響は adoption validate のみ。
- `ProjectAbsolute` target の adoption が新たに reject される
  意図的変更 — test-spec Risk に記録済み、既存 fixture が同組合せ
  を使わないことを test で確認。
- apply の owner/role seal は不変 — persisted `SendLevel` は
  validate 通過後のみ canonical spec を持つ。

## Task Breakdown

- T1: `control.rs` 単一 source + `ops.rs` 両腕の検査追加
- T2: `tests/ctl_02.rs` fixture 追加 + 補助検証
- T3: canonical 9 lane + impl record + index 再生成

## Verification

- [x] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` /
      `mise run check-app-native` / `mise run lint-app-native` /
      `mise run test-msrv` / `mise run build` — 全 exit status を
      impl record に記録。
- [x] test-spec `docs/test-specs/ctl-02-adopt-sendlevel.md` の各
      Feature が fixture で担保されること。

## Open Questions / Gaps

- なし。

## Self-Review

- scope は spec の In Scope と一致。`SendGainSpec::Create`・
  他 crate・manifest・load-time oracle への変更なし。reject 採用は
  ADR の決定どおり。

## Implementation Handoff

- Recommended mode: inline execution（dispatched worker）。
- Independent work streams: none。
- User approval required before dispatch: no（worker spec で実装
  済み）。

## Review Handoff

- `ProjectAbsolute` reject が意図的契約狭めであること、
  `capability: Some` adopt の reject が plugin binding 転用を
  防ぐことを impl record/PR に明記すること。
- headless の証拠境界（実 Plugin/OS/#103/#109 Oracle・AudioLens・
  Device Oracle は外部 gate・NotRun）を PR/impl record で明示する
  こと。
