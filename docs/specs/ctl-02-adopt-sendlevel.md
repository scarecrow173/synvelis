---
id: "SPEC-xA6Rt9SOy2hTBVmns3bmV4"
type: "spec"
status: "approved"
title: "CTL-02 adoption 経路の非 canonical SendLevel persisted 化抑止"
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
    - "docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/ctl-02-adopt-sendlevel.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/122"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md"
  defers: []
  deferred-by: []
---

# CTL-02 adoption 経路の非 canonical SendLevel persisted 化抑止

## What Should Be Built

Issue #122（CTL-02）の残存観測を閉じる headless slice。
`ProjectOp::AddSend` / `ProjectOp::SetSendGainTarget` の **adoption
apply 経路**で、adopt される target が CTL-02 と同じ canonical 契約
（`Send`+`SendLevel` の kind/mapping/time_scope/capability）を満たす
ことを強制する。

現状の apply は採用 target の `owner` を `ControlOwner::Send`、
`role` を `ParameterRole::SendLevel` に書き換えるが、
`kind`/`mapping`/`time_scope`/`capability` は書き換えない。
そのため canonical spec を持たない自由形式 target（例:
`Exponential` mapping・`capability: Some`・`ProjectAbsolute`
time_scope）を adopt すると、非 canonical な `SendLevel` が
persisted され得る。先行 slice（PR#350）は `AddParameterTarget`
の write-time validation のみを強化しており、adoption 経路は
未防御だった。

採用 target の spec が canonical でない場合は **reject**
（typed `Error::Validation`）で扱う — apply 時の正規化は採用しない
（根拠は ADR `docs/adr/ctl-02-adopt-sendlevel.md`）。
`SendGainSpec::Create` 経路（canonical spec を staging が発行する
既存経路）と `SendGainSpec::Adopt` の canonical 受理は不変。

## Why Now

Issue #122 の最新統合コメントが名指しした残存観測であり、
「内部 channel/send target は canonical persisted spec を持つ」
という CTL-02 契約に対する最後の write 経路の穴である。
`AddParameterTarget` を通らない persisted 形状（adoption による
owner/role 書き換え）が残っている限り、lowering・authority lookup
・将来の検証が定義しない spec を持つ `SendLevel` が保存され得る。

## Users and Value

- `synvelis-project` の control/routing 永続化契約: adoption で
  persisted される `SendLevel` が常に canonical spec を持つことが
  保証され、`send_level_target_id` / lowering / automation 評価が
  参照する形状が一意になる。
- `RoutingEditIntent`（`SendGainSpec::Adopt` /
  `RoutingEdit::SetSendGainTarget`）と低層 `ProjectOp` の双方で
  同一契約が fail-closed に効く。

## Scope

### In Scope

- `crates/synvelis-project/src/control.rs`: `Send`+`SendLevel` の
  canonical spec を返す単一 source の追加と、採用前 target が
  その契約を満たすかの判定 helper。
- `crates/synvelis-project/src/ops.rs`: `AddSend` /
  `SetSendGainTarget` の `gain_target: Some` validate に canonical
  spec 検査を追加（両経路で同一契約）。
- `crates/synvelis-project/tests/ctl_02.rs`: 非 canonical adopt の
  reject・canonical adopt の受理・ProjectAbsolute time_scope の
  reject を pin する fixture 追加。既存 11 tests の非回帰確認。
- `docs/` 文書チェーン（spec/ADR/design/plan/test-spec/task/impl
  record）の新規追加と、触れた dir の管理 index 再生成。

### Out of Scope

- `SendGainSpec::Create` 経路の canonical 受理の変更（直前 slice で
  green 確認済み・無変更維持）。
- `Generic`/`ProcessorParam` 等他 role・他 owner・
  `capability: Some` の既存利用の変更。
- `crates/synvelis-project/` 以外のクレート、`mise.toml`、
  `.github/`、`Cargo.lock`（依存変更なし）。
- snapshot load / migration が読む既存 persisted 形状の矯正
  （write-time 契約の対象は op 経路であり、既存データの
  migration は本 slice の対象外）。
- Issue #122 の残件: 実 Plugin/OS・#103/#109 Domain Oracle・
  AudioLens・実 Plugin verified binding 受入・Device Oracle —
  外部 gate であり本 slice の範囲外（PR 本文に記載）。

## Requirements

### CTL-02A-R1: adoption の canonical spec 強制

- `AddSend { gain_target: Some(t) }` と
  `SetSendGainTarget { gain_target: Some(t) }` の validate は、
  adopt 対象 `t` の persisted spec が `Send`+`SendLevel` の
  canonical 契約 — `kind == Continuous`、`mapping == Linear`、
  `time_scope == ProjectMusical`、`capability == None` — を
  満たすことを要求する。`name`/`base` は従来どおり authored で
  契約対象外。
- 違反は typed `Error::Validation` で reject し、project は
  無変更（既存の validate→apply 規律どおり）。
- `RoutingEdit` 経路（`CreateSend` の `SendGainSpec::Adopt`、
  `SetSendGainTarget`）は同じ `ProjectOp` に stage されるため、
  同一検査が transaction 単位で atomic に効く — 追加の
  transaction 固有検査は設けない。

### CTL-02A-R2: 既存契約の非回帰

- `SendGainSpec::Create`（staging が canonical
  `AddParameterTarget` を発行して bind）が引き続き受理される。
- canonical spec を持つ自由形式 target（例: Track-owned
  `Generic`・Continuous/Linear/ProjectMusical/capability None）の
  adoption が引き続き受理され、apply で `Send`+`SendLevel` に
  sealed される。
- 既存の拒否（well-known bootstrap target・非 project-time・
  二重 bind・Strip/Processor owner の adopt・非 Send owner の
  `SendLevel` orphan 作成）は不変。

### CTL-02A-R3: fixture pin

- `crates/synvelis-project/tests/ctl_02.rs` に、非 canonical spec
  の adopt が `AddSend`・`SetSendGainTarget`・`RoutingEdit`
  両経路で reject され persisted されないこと、canonical adopt
  が受理されることを pin する test を追加する。
- 既存 `ctl_02.rs` 11 tests を含む `mise run test` が全 pass。

## Acceptance Criteria

- [ ] `AddSend`/`SetSendGainTarget` の adoption apply 経路で、
  adopt される target の spec が CTL-02 と同じ canonical 契約
  （Send+SendLevel の kind/mapping/time_scope/capability）を満たす
  ことを強制する。canonical spec を持たない target の adopt は
  reject で扱い、非 canonical `SendLevel` が persisted されない。
- [ ] 非 canonical adopt が persisted されないことと、canonical
  adopt が引き続き受理されることが
  `crates/synvelis-project/tests/` の fixture で pin され、
  既存 `ctl_02.rs` 11 tests が非回帰である。
- [ ] 追加・変更した test が `mise run test` で全 pass し、
  canonical 9 lane（`fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`）が
  すべて exit 0。
- [ ] #122 の残件（外部 gate）が PR 本文の「未対応の作業」に
  記載され、本 slice の範囲外であることが明記される
  （`Refs #122` で Open 継続）。
- [ ] `git merge origin/main` で最新 main を取り込み、doc-chain
  文書と管理 index の再生成を含めてコミットする。

## Deferred Design Concerns

- 既存 persisted snapshot に非 canonical `SendLevel` が存在する
  場合の load-time 挙動は本 slice の対象外（write-time 契約は
  op 経路に対するものであり、`validate_references` の拡張は
  別途判断）。
- `ProjectAbsolute` time_scope を持つ target の adoption は
  canonical 契約（`ProjectMusical` 固定）により reject される —
  旧来の「project-time なら可」は canonical 内で狭められる意図的
  変更であり、design/ADR に根拠を記録する。
- Issue #122 残件の外部 gate（実 Plugin/OS・#103/#109 Domain
  Oracle・AudioLens・実 Plugin verified binding 受入・Device
  Oracle）は引き続き NotRun/Deferred。

## Sources and Relations

- Issue #122（本 spec の直接の source）: 最新統合コメントの
  「残存観測」が adoption 経路の非 canonical `SendLevel`
  persisted 化を名指し。
- `docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md`:
  先行 slice の実装記録 — canonical spec 契約
  （`canonical_internal_spec`/`strip_channel_spec`）と
  「残存観測(本 slice 対象外)」の正本。
- `docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md`:
  `ControlOwner`/`AutomationTimeScope`/`ParameterRole` の typed
  schema 契約。
- `docs/specs/sf-02-routing-transaction-commands-semantic-validation.md`:
  `RoutingEditIntent`/`SendGainSpec` transaction 契約。
- `docs/adr/ctl-02-adopt-sendlevel.md` — reject 採用の決定。
