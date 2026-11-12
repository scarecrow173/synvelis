---
id: "IMPL-034QUbof4T2tg909CWkwXN"
type: "impl"
status: "completed"
title: "GW-04 Semantic Operation Preparation / Effect Evaluation / Linked MIDI Scope"
created: "2026-11-03"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/56"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare_regressions.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/capability.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/context_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/outcome.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/wire.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/contract.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/56"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/56"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/56"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# GW-04 Semantic Operation Preparation / Effect Evaluation / Linked MIDI Scope 実装記録

## Summary

Issue #56。#45 GTC-005・#46 GW-04 に基づく semantic operation preparation を、
既存 `synvelis-project::Operation`/`ProjectOp`/`ProjectController` 契約に
接続して実装した。AI 専用の第二 validator は作らず、domain legality は
`Operation::validate` に一元化し、Effect は pin 済み実 Project の
before/after 観測から導出する。commit/apply 配線は GW-06 へ defer。

## Context

- source Issue: #56(GW-04)。`implements`/`verifies` は Issue #45・#46・#56
  (GitHub Issue)で、個別の設計文書はなく GW-01〜03 の IR を前提とする。
- `depends-on`: `docs/impl/ir/gw-01-gateway-contract.md`、
  `docs/impl/ir/gw-02-authorization-policy.md`、
  `docs/impl/ir/gw-03-context-projection.md` — GW-01 の bind/invoke 経路、
  GW-02 の `GrantPolicy` internals、GW-03 の exact-revision pin
  (`pin_revision`)を再利用する。
- 検証 base は GW-01〜03 merge 済み + 本変更。commit/apply 配線は
  GW-06 へ defer。

## Implementation

| 範囲 | 内容 |
| --- | --- |
| `prepare.rs`(新規) | `NamedOperation` — `{capability, target?, args?}` の厳格 parse(deny_unknown)。`SemanticCommand`/`Precondition`/`DerivedEffect`/`PreparedOperation`/`PreparedRecord`/`PreparationBasis`。`op_specs`+`lower_for` — versioned capability → `TargetContract`/`ValueSchema`/`ProjectOp` lowering の静的表(lowering は syntactic で state 非依存)。`op_facts` — `Operation::validate` が仮定する前提を `Precondition` として mirror する semantic 表 + internal reads + `impacted`(linked clip 辺) + `unsupported`。**`Observed`** — `ResourceAttributes` だけでは muted/velocity/source state/name/ppqn/tempo map を観測できないため、`attrs + detail`(entity 全体の serde 値)の2層観測を追加。`derive_effects` — attrs/detail 差分で Write、集合差で Create/Delete。`BaseFirstOracle` — base 優先・candidate 補完の `ResourceOracle`(transaction 内生成 parent の resolvable 化)。`PrepareAuthorizer` — GW-02 への trait seam(`authorize_effects`/`authorize_emission`)。`emitted_classes`/`flattened_scoped`/`result_json`/`effect_json` |
| `prepare_engine.rs`(新規) | `PrepareEngine` — `synvelis.operation.prepare` descriptor(`OperationDomain::Proposal`、targetless、Read 宣言)と全 semantic mutation op の descriptor+input schema を `install` で登録(mutation は executor 無し = apply は GW-06)。run 流れ: inner op 解決 → `pin` 正確 revision → target bind → schema 検証 → lowering → `Operation::validate`(pin 済み clone 上、live 非変更) → 決定的 `CounterIdGenerator` で private clone へ `apply` → Project invariant re-check → `resource_map` before/after → `op_facts`/`derive_effects` → shared-source write references に linked clip を enrich → `authorizer.authorize_effects`/`authorize_emission` → `PreparedRecord` 保持(FIFO cap)。全失敗は型付き envelope(PreparationRejected/SnapshotExpired/SchemaViolation/UnknownCapability 等)。`PreparedOperation` は commands/preconditions/effects/unsupported/candidate/base を保持 — GW-06 の再検証入力 |
| `prepare_policy.rs`(新規) | `PreparePolicy` — `PrepareAuthorizer` のデフォルト実装。GW-02 `GrantPolicy` の既存 internals(`authorize_agent`→preflight、`check_effects`→scope coverage+create_within、emission boundary→data_access class 検査)を再利用。inner op の capability floor は proposal commands を照合。human caller は GW-02 と同じ early-permit だが **unsupported fail-closed は caller 種別に関係なく engine が先に実行**(human の early-permit を迂回経路にしない) |
| `context_engine.rs` | `pin_revision(project, revision)` を追加 — `pin_for` の `ReadAt` と同じ exact-revision 意味論。unknown/evicted は `SnapshotExpired`、latest への silent fallback なし |
| `capability.rs` | `OperationDomain::Proposal` 追加 |
| `request.rs` | `AuthorizationStage::PrepareProposal` 追加、wire tag `prepare-proposal` |
| `gateway.rs` | Proposal → `PrepareProposal` stage への dispatch。登録 coherence: `{Query, Proposal}` が mutating `effect_bound` を宣言するのを拒否(reason 文言を `read-side capabilities may not declare mutating effects` に一般化 — contract test 更新) |
| `policy.rs` | `GrantPolicy` internals を `pub(crate)` 化 — `prepare_policy` が `authorize_agent`/`check_effects`/`check_grant_dimensions`/`stage_gate` 等を再利用(第二の認可実装を作らない) |
| `outcome.rs` | `GatewayError::PreparationRejected` 追加(domain-level reject。category Validation、external envelope には safe message のみ) |
| `wire.rs` | `allowed_target_keys`/`deny_extra_keys` — serde internally-tagged enum が未知 key を黙過するため、wire 段で nested `target` object の key を shape 別 allowlist で reject(`resource`/`resource-set`/`create`) |
| `lib.rs` | `prepare`/`prepare_engine`/`prepare_policy` module + public 型 export |
| `tests/prepare.rs`(新規) | 18 テスト — 後述 matrix 参照 |
| `tests/rig/mod.rs`(新規) | linked-MIDI demo project fixture・request builder・grant helper の共有 test rig(`prepare.rs`/`prepare_regressions.rs` 共用) |
| `tests/prepare_regressions.rs`(新規) | 7 regression テスト — review-protocol で確定した各指摘の恒久検証(下表参照) |

## 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| Effect API を `synvelis-project` に足さず gateway 側 Port で包む | `Operation` の既存契約(validate/apply)を壊さず互換性を保つ。validate は pin 上 clone に走らせるので live は不変 — apply だけが private clone に向く |
| `Observed{attrs, detail}` の2層観測 | `ResourceAttributes`(owner/range/related)は muted・velocity・`SourceState`・name・ppqn・tempo map を表せない — それらの変化を Write として検出できないと SEC-14 的な「正直でない effect」を出してしまう。entity 全体の serde 値を detail にして diff |
| unsupported は `PreparationRejected` で caller 非依存 fail-closed | 派生 effect が網羅できない op(clock-epoch 等)を human early-permit が迂回すると G2 違反 — engine が authorization 呼出の**前**に検査 |
| transaction 内生成 resource の parent 解決は base-first oracle | detach で新規 source 配下の note create は parent が base に未存在 — base だけでは Unresolved deny、candidate だけでは delete された resource を見失う。base 優先 ∪ candidate が正当な両立 |
| inner mutation descriptor に executor を置かない | 直接 invoke しても apply 経路が無い = GW-06 の commit seam まで capability は「準備可能」としてのみ存在。`register_capability(desc, None)` |
| FIFO cap 付き proposal retention | `max_proposals` で memory  bounded — GW-06 が `prepared(&ProposalRef)` で取り出し・`evict` で破棄 |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| Prepare は Project を変更しない | `Operation::apply` は pin 済み `Arc<Project>` の **clone** にのみ走る — `prepare_derives_effects_and_mutates_nothing` が commit 済み project の byte 同一性を検証 |
| 正確 revision pin・silent fallback なし | `expected-revision`/`read-at`/`fixed-snapshot` 全て exact — unknown → `SnapshotExpired`、evict 後 commit 済み rev2 でも rev1 指定は `SnapshotExpired`(`invalid_revision_rejects_without_mutation`、`expired_base_snapshot_never_rebases_to_latest`) |
| `{capability, target?, args?}` 厳格 parse | `NamedOperation` deny_unknown + `inner_op_schema` — malformed/unknown field/unknown capability は型付き envelope(`unknown_capability_fails_closed` 等) |
| versioned capability・TargetContract・ValueSchema | op 解決は `CapabilityRef{id,version}` exact match、target は `bind_target`(None/Resource/ResourceSet/Create の shape+kind 照合)、args は op 毎の `ValueSchema` — **lowering 前** |
| lowering は syntactic | `LowerFn` が `(&OperationTarget,&Value) -> ProjectOp` の純粋写像 — state を見ない。domain legality は `Operation::validate` が唯一の権威(`invalid_target_id_rejects_without_mutation` の absent-clip 分岐) |
| Read/Create/Write/Delete + before/after + detail | `DerivedEffect` — `before`/`after`(ResourceAttributes)+`before_detail`/`after_detail`(serde entity)。muted/velocity/move-range/detach 関連変更を実検証(`clip_move_reports_before_and_after_range` 等) |
| deterministic ordering | reads `BTreeSet` → creates/writes/deletes `ResourceRef` 順(`derive_effects`) |
| structural references | create=owner(parent)、write/delete=survivor after related・delete before related、`impacted` linked clips を shared-source Write references に enrich |
| fail-closed unverifiable | `facts.unsupported` 非空 → `PreparationRejected` — agent/human 両経路で検証(`unverifiable_effects_fail_closed_for_every_caller`)+`clock-epoch.register` は unsupported 宣言 |
| linked MIDI 明示・implicit copy なし | source-level note op は `impacted` で shared source Write references に全 linked clip を列挙。detach は新 Source+新 note+clip repoint の明示 Create/Write(`linked_midi_write_names_source_and_linked_clips`、`detach_is_explicit_new_source_plus_clip_repoint`) |
| real effects → GW-02 | `PreparePolicy` が `check_effects`/`authorize_emission` を通す — scope coverage(`derived_effects_flow_through_grant_scope`)、inner capability floor(`inner_capability_outside_grant_denies`)、create_within axis(`creation_requires_the_create_axis`)、source-scope 非継承(`linked_source_write_needs_source_scope_not_just_clip`) |
| composite atomicity | batch → 単一 `ProjectOp::Composite`、member 失敗は全体 reject(`batch_composes_member_effects_in_order`、`batch_rejects_atomically_when_a_member_fails`) |
| ProjectController 統合(Undo 迂回なし) | 保持された `record.op` を `Command{expected_revision: base.revision, op}` で verbatim apply → Undo/Redo 実経路。stale expected_revision は `StaleRevision` 拒否(`prepared_op_commits_through_controller_and_undoes`) |
| 保持 proposal の GW-06 seam | `PreparedRecord{request, op, prepared{base{project,revision,snapshot,tempo_map_revision}, commands, preconditions, effects, candidate}}` + `prepared()/evict()`(`retained_record_carries_revalidation_data`) |

## SEC / GT matrix(#46 Failure Matrix 対応・GW-04 該当分)

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| SEC-05 | clip scope ≠ source scope — linked source write は source 自体を coverage 評価 | Pass | `linked_source_write_needs_source_scope_not_just_clip` — clip だけを scope に持つ grant は shared-source Write で `target outside grant scope` |
| SEC-06 | destination coverage — 移動先/新範囲を after-state で再評価 | Pass | `PreparePolicy` が `check_effects` に `after_states` を渡し移動先を `coverage` で再評価 |
| SEC-13 | inner op は proposal capability で黙認しない | Pass | `inner_capability_outside_grant_denies` — prepare 許可は member を許可しない。commands を照合 |
| SEC-14 | unverifiable effects は全 caller で reject | Pass | `unverifiable_effects_fail_closed_for_every_caller`(agent+human 両経路) |
| G2 | unknown/unsupported は Permit に丸めない | Pass | `unknown_capability_fails_closed` + unsupported fail-closed |
| GT-005 | invalid id/revision/range で zero mutation | Pass | `invalid_target_id_rejects_without_mutation`・`invalid_revision_rejects_without_mutation`・`wrong_time_domain_and_range_reject_without_mutation` — `proposal_count()==0` と latest revision 不変を両立して検証 |
| GT-006 | 時間 domain の暗黙変換なし | Pass | `wrong_time_domain_and_range_reject_without_mutation` — `project-musical` range は `source-native` schema で `WrongTimeDomain` |
| 原子性 | composite の member 失敗で partial effect を出さない | Pass | `batch_rejects_atomically_when_a_member_fails` — `proposal_count()==0`、project 無変更 |

## Related Experiments

本実装では実験ログを採用・却下していない。front matter の
`metadata.experiments.adopted`/`rejected` はいずれも空で、`docs/impl/exp/`
に本件(GW-04)に対応する実験ファイルは存在しない。設計文書どおりに
実装し、検証は prepare test・regression test と review-protocol で行った。

## Validation

- 環境: Windows、rust 1.99.0(mise pin)、GW-01〜03 merge 済み base + 本変更
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、workspace 45 test binary 全緑・0 failure。
  `tests/prepare.rs` 18 テスト + `tests/prepare_regressions.rs` 7 テスト pass、
  既存 `tests/contract.rs` 全 pass(1件は coherence reason 文言の更新に伴う
  test expectation 修正)

## Review-protocol 実施結果

独立 reviewer 2 系(A: 受入条件トレース / B: adversarial)を投入し、全 candidate を
コード実測で個別検証。確定した指摘は全て修正し、`tests/prepare_regressions.rs` に恒久検証を追加。

| 指摘 | 判定 | 対応 |
| --- | --- | --- |
| MIDI source の note payload が metadata-only の emission class で返る — grant が `StructuredContentRead` を持たなくても内容が漏洩する | 確定(B) | `emitted_classes` で note detail を持つ effect を structured-content class に分類。`midi_source_read_discloses_structured_content_not_metadata` で deny→grant 差分を検証 |
| domain `NotFound` が scope 認可より先に走り、absent と out-of-scope を error で識別可能(SEC-03 existence blinding 違反) | 確定(B) | provisional scope gate(`provisional_gate`)を `op.validate` より前へ — 両者が同一 `Denied` verdict で fail。`absent_and_uncovered_targets_deny_identically` |
| batch member の `args` が op 別 `ValueSchema` 検証を迂回(ordinary path にのみ実施) | 確定(A) | schema 検証を batch/ordinary 分岐より前の共通段へ移動。`batch_args_are_schema_validated` |
| batch capability 自体が authorization floor に含まれず member だけで prepare 可能(SEC-13) | 確定(A) | capability floor に executed operation の ref を追加。`batch_capability_itself_must_be_granted` |
| nested `target` object の未知 key を serde internally-tagged enum が黙過 | 確定(B) | `wire.rs` に shape 別 allowlist(`allowed_target_keys`/`deny_extra_keys`)を追加。`inner_target_rejects_unknown_keys` |
| project-level create の `parent` が request project と照合されず別 project を名指せた | 確定(B) | `provisional_gate` 冒頭で `parent == req.project()` を検査。`project_parent_must_be_the_request_project` |
| `CapabilityRef` version 不一致が `UnknownCapability` に丸まり taxonomy 違反 | 確定(A) | id 既知・version 不一致は `UnsupportedCapabilityVersion` を返す。`known_id_wrong_version_reports_version_not_unknown` |
| precondition が generic で domain の実不変条件を表していない(split/note) | 確定(A) | `SplitClip` に `clip-musically-anchored`/`split-inside-extent`/`split-offset-not-before-source`/`no-note-straddles-split`、note mutation に `note-exists`/`note-fields-valid` を追加 — `Operation::validate` の実前提を mirror |

## Risks

- transaction 内生成 resource の `create_within.parents` は `OwnerConstraint::Any` が必要
  — 生成される新 resource の id は grant 発行時点で名指し不可能。`OneOf` で拘束する場合は
  直接 parent が base-resolvable な op(track.create 等)に限る
- `midi.note.*` の note create は `StructuredContentRead` class を emission boundary で要求
  — note detail(range)を持つ effect を emit するため metadata-only grant では emission deny
- candidate の `Arc<Project>` を保持する proposal retention は `max_proposals` で bounded
  だが、project が巨大な場合 1 proposal あたりの memory footprint は O(project size) × clone

## Follow-ups

- GW-06: `PreparedRecord` を `ProjectController::apply` へ流す commit capability。
  `base.revision` を `expected_revision`、preconditions を再検証、`prepared.candidate`
  との drift 検出を commit 時に行う
- `RegisterClockEpoch` は `unsupported` 宣言済み — gateway resource model が clock epoch を
  観測できる resource kind を得たら lower + effect 化
- 残り `ProjectOp` 変数(parameter/automation/comp 系)の lowering — issue #56 範囲外の
  semantic capability として別 issue で扱う
