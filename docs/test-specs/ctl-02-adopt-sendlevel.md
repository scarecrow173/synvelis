---
id: "TSPEC-l7ZEZAPF6fGjIhiojtusYR"
type: "test-spec"
status: "approved"
title: "CTL-02 adoption SendLevel canonical spec 検証仕様"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
    - "docs/designs/ctl-02-adopt-sendlevel-design.md"
  derived-by: []
  verifies:
    - "docs/specs/ctl-02-adopt-sendlevel.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# CTL-02 adoption SendLevel canonical spec 検証仕様

`crates/synvelis-project/tests/ctl_02.rs` が担保する契約。
`AddSend`/`SetSendGainTarget` の adoption（低層 op 直接経路と
`RoutingEditIntent` transaction 経路の双方）で、adopt される
target の persisted spec が `Send`+`SendLevel` canonical 契約
（`kind == Continuous`、`mapping == Linear`、
`time_scope == ProjectMusical`、`capability == None`）を満たす
ことを要求し、違反は typed `Error::Validation` で reject して
project を無変更に保つ。`name`/`base` は契約対象外で authored
のまま。全て headless の project 層 fixture — 実 Plugin/OS/RT
観測は対象外。

## Feature: 非 canonical adopt の reject

- Rule(CTL02A-R1-r1): `ProjectOp::AddSend { gain_target: Some(t) }`
  で `t` が `kind != Continuous`・`mapping != Linear`・
  `time_scope != ProjectMusical`・`capability.is_some()` の
  いずれかを持つ場合、`Err(Error::Validation)` で reject され、
  send・connection・target のいずれも persisted されない
  （project state が apply 前と一致）。
- Rule(CTL02A-R1-r2): `ProjectOp::SetSendGainTarget {
  gain_target: Some(t) }` で同条件の `t` を bind しようとした
  場合、同様に `Err(Error::Validation)` で reject され、send の
  `gain_target` と target の owner/role は不変。
- Rule(CTL02A-R1-r3): `RoutingEditIntent` 経路
  （`CreateSend { level: SendGainSpec::Adopt(t) }` と
  `RoutingEdit::SetSendGainTarget { target: Some(t) }`）でも同一
  reject が transaction 単位で atomic に効く — intent 全体が
  reject され部分適用されない。
- Rule(CTL02A-R1-r4): `time_scope == ProjectAbsolute` の target は
  旧来の「project-time」検査を通過するが canonical `SendLevel`
  は `ProjectMusical` 固定のため reject される — project-time
  許容と canonical 契約の差分を pin する。

## Feature: canonical adopt の受理

- Rule(CTL02A-R2-r1): canonical spec
  （Continuous/Linear/ProjectMusical/capability None）を持つ
  自由形式 target（例: Track-owned `Generic`）の adoption が
  `AddSend`・`SetSendGainTarget`・`SendGainSpec::Adopt` で受理
  され、apply 後に `owner == ControlOwner::Send(send_id)`・
  `role == ParameterRole::SendLevel` に seal され、
  `send_level_target_id` が解決する。`name`/`base` は authored
  のまま保持される。
- Rule(CTL02A-R2-r2): Send-owned canonical `SendLevel` target の
  `SetSendGainTarget` 再 bind（同一 send の差し替え/再確認）が
  引き続き受理される — `SendGainSpec::Create` staging 経路と
  同じ形状。
- Rule(CTL02A-R2-r3): `SendGainSpec::Create`（staging が canonical
  `AddParameterTarget` を発行）は無変更で受理される —
  既存 `send_create_commits_gain_target_in_one_undo` 等の
  fixture が非回帰。

## Feature: 既存 guard の非回帰

- Rule(CTL02A-R2-r4): well-known bootstrap target・Strip/Processor
  owner target・二重 bind target・非 project-time（ClipLocal/
  NoteLocal）target の adopt は従来どおり reject され、新検査の
  追加で挙動が変わらない（既存 `ctl_02.rs`/`ctl_01.rs`/
  `sf_02.rs`/`routing_sf01.rs` fixture が green）。

## Guarantees

- persisted される `Send`+`SendLevel` target は常に
  `canonical_internal_spec` の契約形状を持つ — adoption は
  spec を書き換えず、validate が契約適合を保証する。
- reject は validate 段階で完結し、project state・undo 履歴・
  revision は無変更（`Command` の CAS 規律どおり）。

## Non-goals

- `SendGainSpec::Create` の canonical 受理仕様の変更。
- `Generic`/`ProcessorParam`・他 owner・`capability: Some` の
  既存利用の変更。
- snapshot load / migration が読む既存 persisted 形状の矯正、
  `validate_references` への canonical oracle 追加。
- Issue #122 残件の外部 gate（実 Plugin/OS・#103/#109 Domain
  Oracle・AudioLens・実 Plugin verified binding 受入・Device
  Oracle）。

## Risk

- 検査追加により `ProjectAbsolute` target の adoption が新たに
  reject される — 意図的な契約狭め（ADR Decision 4）であり、
  既存 test がこの組合せを使っていないことを全 test pass で
  確認する。
