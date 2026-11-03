---
id: "IMPL-0Gw02AuthorizationPo01"
type: "impl"
status: "completed"
title: "GW-02 Principal / Execution Grant / Authorization Policy — Fixed/Dynamic Scope"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/54"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-gateway/src/clock.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/grant.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/store.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/policy.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/authorization.rs"
      - type: "file"
        path: "docs/designs/gw-02-authorization-policy-design.md"
      - type: "file"
        path: "docs/impl/ir/gw-02-authorization-policy.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/capability.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/request.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/id.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/fixture.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/contract.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/fixtures.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/gw-02-authorization-policy-design.md"
  implemented-by: []
  depends-on:
    - "docs/designs/gw-01-gateway-contract-design.md"
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/54"
  derived-by: []
  verifies:
    - "docs/designs/gw-02-authorization-policy-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# GW-02 Authorization Policy 実装記録

Issue #54。#44 AHG-006〜010・#45 GTC-003/ACG-003 に基づく認可ポリシー層を、
GW-01 の `PolicyEvaluator` seam 上の実 evaluator として `synvelis-gateway`
内に実装した。Mock Policy Store で先行(Issue 明示)、durable store・commit
fence・recovery は GW-06 へ defer。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `capability.rs` | `DataAccessClass`(MetadataRead/MediaBytesRead/DerivedDataRead/ExternalDisclosure/ProviderTransfer)と `DataAccessSet` を追加。`CapabilityDescriptor.data_access` で capability が宣言する data-access 集合を型化 — descriptor ⊆ grant を評価が要求 |
| `clock.rs`(新規) | `PolicyInstant`(u64 ordered opaque instant)、`PolicyClock` trait、`SystemClock`(wall clock)、`ManualClock`(決定的 — expiry 境界をテスト可能に)。evaluator は `SystemTime::now` を直接呼ばない |
| `grant.rs`(新規) | `PolicyEpoch`、`GrantBinding`(Task/Session/PersistentProject)、`ApprovalPolicy`(Suggest/ApproveToApply/DelegatedApply)、`GrantLimits.apply_budget`、`Validity`(expires_at+revoked、`live(now)`)、`OwnerConstraint`、`MoveArgs`(after-state 再評価用 args field 名)、`DynamicScope`(owners/kinds/range/max_members/include_future/move_args)、`PlacementConstraint`、`CreateWithin`、`ResourceScope`(Exact/Snapshot/Dynamic)、`Grant`(principal/project/binding/capabilities/permissions/data_access/scope/create_within/approval_policy/limits/validity/epoch_ceiling/issued_at_revision)、`ResourceAttributes`(kind/owner/range/created_revision/related)、`ResourceFacts`(Known/Unresolved — 不在と不可視を区別しない)、`ResourceOracle` trait、`CreateEffect`、`ScopedEffects`(reads/writes/creates/deletes/disclosures/external_transfers/unsupported — GW-04 導出 view の入力型) |
| `store.rs`(新規) | `PolicyStore` trait(grant/task/proactive_permit/policy_epoch/try_consume_apply — 全て単一原子操作、ACG-003 線形化 seam)。`try_consume_apply` は write lock 内で存在/liveness/epoch/budget を再検証してから消費(TOCTOU 排除)。authority ID は単一発行 — 再発行は no-op で revoked/consumed を巻き戻さない。`AuthorizedTask`、`Activation`(UserIssued/Proactive{permit})、`ProactivePermit`、`GrantRecord`(grant+consumed_applies)、`InMemoryPolicyStore`(単一 `RwLock` で全操作直列化。issue/revoke/update_policy の管理 API) |
| `policy.rs`(新規) | `GrantPolicy` — `PolicyEvaluator` 実装。評価連鎖: human principal は adapter-attested manual authority として全 stage permit。agent は live task(activation; proactive は live permit 再照会)+ principal/project/binding 一致の grant の両方必須。binding 証明前の失敗は一律 `no applicable authorization`、scope 失敗は `target outside grant scope`(存在情報を漏らさない)。descriptor は request の capability ref と id+version 一致を検証。target/effect 両 pass で1本の `ScopeCounters` を共有(ResourceRef 単位で重複計上なし)。`evaluate_effects`(ScopedEffects 照合 — unsupported 非空は一律 deny)、`evaluate_commit`(Commit stage、fresh store read + DelegatedApply unit の原子消費、GW-06 fence seam)を公開 |
| `request.rs` | `ResourceRef` に `PartialOrd`/`Ord` 追加(BTreeSet scope 用)、`ResourceRef::parse_tagged` pub(crate) helper |
| `id.rs` | `ProactivePermitRef` 追加。`CapabilityRef` に `PartialOrd`/`Ord` |
| `fixture.rs` | fixture descriptor 5 件に `data_access` 宣言を付与、`StaticOracle` テスト用 oracle |
| `lib.rs` | GW-02 モジュール・型の export、crate doc に GW-02 節追加 |
| `tests/authorization.rs`(新規) | 74 テスト — SEC-01〜14・DATA-01〜10 該当分 + AHG-007/008/009/010・ACG-003・GTC-003。下記 matrix 参照 |
| `tests/contract.rs` / `tests/fixtures.rs` | descriptor literal に `data_access` field を追記(GW-02 field 追加への追従) |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| 未認証/別Principal/別Project/Grantなし/期限切れ/取消済みは拒否(存在漏えいなし) | `authorize_agent` は binding 証明前の全失敗を `None` → `no applicable authorization` に統一。`sec04_no_existence_leak_between_deny_reasons` が unknown task / unknown grant / revoked task の deny reason が区別不能なことを直接検証。`ResourceFacts::Unresolved` が不在/不可視を区別しない |
| Fixed Scope と動的 Scope に新規 Clip・移動・Reparent・Source 共有を適用 | `sec09_exact_*`(Exact/Snapshot 同等性)、`sec10_dynamic_*`(kind/owner/range/domain/future/max_members)、`sec06_move_*`(after-state 再評価・範囲逸脱・malformed deny)、`sec05_shared_source_*`(related は mutating op で独立評価) |
| Metadata Read / Media Bytes Read / Derived Data / External Disclosure / Provider Transfer を区別 | `DataAccessClass` 5 variant。descriptor ⊆ grant を `check_grant_dimensions` で、`effects.disclosures`/`external_transfers` を `check_effects` で照合(`data_access_*`・`data_effect_disclosure_and_transfer_checked`) |
| 3 モード・Proactive 起動・限度超過・Policy Version 更新の拒否テスト | `ahg009_*`(3 mode 全部 + wire 宣言非依存)、`ahg007_*`(proactive permit live/revoked/別principal、delegated ≠ proactive)、`ahg010_*`(budget 消費・枯渇・原子性)、`sec12_*`(epoch ceiling で失効、ceiling なしは生存) |
| Revoke 確定と Commit の線形化 interface を GW-06 へ提示 | `PolicyStore` trait が原子操作単位、`evaluate_commit` が fence 内呼出し seam。`acg003_commit_revalidates_fresh_state`(revoke 後 commit は deny)・`acg003_commit_before_revoke_stands`(先立つ commit は revoke で無効化されない)で契約両側を検証。最終原子性判定は GW-06 結合時 |
| SEC-01〜14 / DATA-01〜10 該当分を Pass/Blocked + 証拠で記録 | 下記 matrix |

## SEC / DATA matrix(#46 Failure Matrix 対応)

### SEC-01〜14

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| SEC-01 | 未認証 MCP Tool 実行 → Deny | Pass | `sec01_agent_without_any_authority_denied`(task/grant 両無し)、`sec01_grant_ref_alone_is_not_proof`、`sec01_human_needs_no_grant` |
| SEC-02 | Agent が Human Authority を自己申告 → 拒否 | Pass | GW-01 層: wire に authority field なし(`FORBIDDEN_AUTHORITY_FIELDS` → `SelfDeclaredAuthority`、tests/contract.rs)。GW-02 層: `sec02_authority_issued_for_another_principal_denied`(principal 不一致の task/grant は不可視と同じ deny) |
| SEC-03 | Grant なし Project Context Read → Deny、存在不開示 | Pass | `sec04_unknown_grant_ref_denied`、`sec04_no_existence_leak_between_deny_reasons`、`sec09_exact_scope_denies_unlisted_and_unresolvable_alike` |
| SEC-04 | 別 Project の Stable ID → Deny | Pass | `sec03_authority_for_another_project_denied`(task/grant の project 不一致) |
| SEC-05 | Track/Clip 権限のみで共有 Source 編集 → Deny | Pass | `sec05_shared_source_not_inherited_through_clip`(related source が scope 外 → deny)、`sec05_read_only_capability_does_not_follow_related` |
| SEC-06 | Move で動的 Scope 逸脱 → Before/After 評価し Deny | Pass | `sec06_move_outside_scope_denied`、`sec06_move_range_outside_bound_denied`、`sec06_move_args_unverifiable_after_state_denied`(after-state 検証不能 → deny)、`sec06_move_inside_scope_permitted` |
| SEC-07 | Grant 発行後の UI Selection 変更 → Target 不変 | Pass | grant の対象は発行時の明示 ID/scope に結び付き UI 状態を参照しない(AHG-006)。GW-01 層が `selection`/`focus`/`active_window`/`current_selection` を `ImplicitTargetRejected` で拒否。`sec09_exact_scope_permits_listed_member`・`sec10_dynamic_*` が固定/制約所属を検証 |
| SEC-08 | Suggest Agent から Apply → Deny、Revision 不変 | Pass | `ahg009_suggest_mode_denies_apply_but_permits_prestages` — RequestApply/Commit は Deny、pre-stage は Permit(handler 未実行のため Revision 不変は自明) |
| SEC-09 | 承認後の ChangeSet 改竄 → Digest 不一致 | Blocked | Proposal/digest/approval receipt は GW-05 所有。GW-02 は `ApprovalRequired` を返すのみで digest 照合契約を持たない |
| SEC-10 | 他 Principal の Approval 使用 → Deny | Blocked | Approval receipt の principal 照合は GW-05/06。GW-02 側の部分(principal 不一致 grant/task/permit の deny)は `sec02_*`・`ahg007_permit_for_other_principal_denies` で Pass |
| SEC-11 | Approval 後 Project Revision 変更 → Conflict | Blocked | 承認と revision の結合は GW-05/06。revision conflict 半分は GW-01 `Consistency::ExpectedRevision` + tests/fixtures.rs の `RevisionConflict` で既存 Pass |
| SEC-12 | Grant Revoke 後の未消費 Approval で Apply → Deny | Pass | `sec08_revoked_grant_denied`・`acg003_commit_revalidates_fresh_state` — Commit stage は fresh store read で revoked を観測 |
| SEC-13 | 既存 Grant で新 Capability 実行 → Deny | Pass | `sec13_capability_outside_grant_set_denied`、`sec13_capability_version_is_exact`(同 id 別 version も deny)、`gt003_new_capability_not_implicitly_granted` |
| SEC-14 | Unknown/Opaque Effect の Delegated Apply → 自動 Permit しない | Pass | `sec14_unverifiable_effects_deny`(unsupported 非空は一律 deny、全 mode)、`sec14_write_outside_scope_denied`、`sec14_create_effect_checks_axis` |

### DATA-01〜10

| Case | 内容 | 判定 | 証拠 |
| --- | --- | --- | --- |
| DATA-01 | Metadata 権限のみで PCM 取得 → Deny | Pass | `data_access_class_subset_enforced`(descriptor が MediaBytesRead を要求・grant は MetadataRead のみ → deny) |
| DATA-02 | Derived Feature への開示権限なし → Deny | Pass | `data_access_class_subset_enforced`・`data_effect_disclosure_and_transfer_checked`(disclosures ⊆ data_access) |
| DATA-03 | Snapshot paging 中の Revision 更新 → 混在させない | Blocked | Context paging/snapshot 一貫性は GW-03(Context Query)所有。GW-02 は paging を行わない |
| DATA-04 | Grant 失効後の追加 Chunk 要求 → 新規開示 Deny | Pass | 各 stage 評価は毎回 fresh store read — `sec08_revoked_grant_denied`・`acg003_commit_revalidates_fresh_state`。paged 取出し自体は GW-03(ReadOperationResult stage で同連鎖が走る) |
| DATA-05 | 別 Client が既知 Operation ID で結果取得 → 再照会 Deny | Pass | `sec02_authority_issued_for_another_principal_denied`(principal 不一致は存在不可視と同じ deny) |
| DATA-06 | Asset Metadata への Prompt Injection → Policy/Grant に影響しない | Pass | args は権限状態へ到達不能(wire に authority field なし)。`sec06_move_args_unverifiable_after_state_denied` が malformed args → deny を検証 |
| DATA-07 | 不正 Path/Resource URI で任意取得 → Deny | Pass | `sec09_exact_scope_denies_unlisted_and_unresolvable_alike`(oracle Unresolved = outside scope)、`sec11_create_under_unresolvable_parent_denied` |
| DATA-08 | 大量 Payload/過大 Range → Bounded Budget 内で拒否 | Pass | `ahg010_budget_binds_at_commit`(apply budget は commit 認可で原子消費)、`ahg010_commit_without_prior_apply_enforces_budget`、`sec10_dynamic_range_outside_bound_denied`(range 包含外)、GW-01 の ResourceSet 1024 上限 |
| DATA-09 | Tool 一覧取得後に Grant 失効 → 実行時再認可で Deny | Pass | `acg003_commit_revalidates_fresh_state`・`sec08_revoked_grant_denied` — discovery≠permission、評価は実行時の fresh read |
| DATA-10 | External Transfer 未許可で Provider 送信 → 送信しない | Pass | `data_effect_disclosure_and_transfer_checked`(`external_transfers>0` かつ grant が ProviderTransfer を含まない → deny) |

## 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、base revision `a932f3f` + 本変更
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、workspace 全 suite 緑(gateway 129 テスト: authorization 74 + contract 40 + fixtures 8 + schema_compat 7)

## Review-protocol 実施結果

独立 reviewer 2 系(A: 受入条件トレース / B: adversarial security)を投入し、
指摘をコード実測で検証して blocking を修正、再検証まで実施。

| 指摘 | 判定 | 対応 |
| --- | --- | --- |
| A1+B2 apply budget が評価呼出回数で消費(1 preflight で N commit が underwriting されうる、commit 直行は budget bypass) | Confirmed P1 | unit は `evaluate_commit` の `try_consume_apply` が write lock 内で消費 — 論理 apply 1 回につき 1 unit。preflight は枯渇の早期報告のみ |
| B3 `max_members` が target pass/effect pass で別々に計上(実効 cap が 2 倍) | Confirmed P1 | `ScopeCounters` を per-request 1 本化、`ResourceRef` 単位で dedupe。`sec10_dynamic_max_members_spans_target_and_effects` |
| B9 `decide`/`evaluate_*` が手渡し descriptor を request の ref と照合しない | Confirmed P1 | `evaluate_with` 冒頭で id+version の一致を検証(`sec13_capability_descriptor_mismatch_denied`) |
| B5 issue 再発行で revoked/consumed が巻き戻る | Confirmed P2 | task/permit/grant の issue を `entry().or_insert()` 化 — authority ID は単一発行(`ahg010_reissue_preserves_revocation_and_spend`) |
| B6 `try_consume_apply` が消費時に liveness/epoch を再検証しない(TOCTOU) | Confirmed P2 | write lock 内で存在/live/epoch/budget を再検証してから消費 |
| B7 `_ => PermitForStage` ワイルドカードが将来 stage を fail-open | Confirmed P2 | `stage_gate` は既知 stage を全列挙、将来 variant はコンパイル時に強制 |
| B1 Session binding が adapter multiplex で混同しうる | Doc | `GrantBinding::Session` は adapter 発行 session identity を `AdapterId` で表現 — multiplex adapter は session ごとに別 ID を mint(GW-08 契約)と明記 |
| B8/A6 `move_args: None` が after-state 未検査 = snapshot 相当であること | Doc | `MoveArgs`/`DynamicScope` に opt-in 契約を明記 |
| B4 human short-circuit が effects 検査を skip | Rejected | 設計意図 — manual authority は別権威であり agent grant の effect 制約は適用外。module doc に明記 |
| B10 `invoke` が preflight のみで effects stage を実行しない | Rejected | 設計意図 — effects 照合は GW-04 導出 view が来てからの opt-in(GW-06 が配線) |
| A5 SEC-03 matrix 証拠のケース番号引用の精緻化 | Fixed | matrix 記述を実ケースへ合わせ済み |

blocking 指摘は全て修正済み、修正込みで `mise run fmt-check / check / lint /
test` 再実行・全緑を確認。

## 未実施の検証

- `mise run test-msrv`(rust 1.89 toolchain 未導入のため未実行)
- GW-05/06 との結合(approval receipt・durable fence) — Issue スコープ外、
  最終原子性は GW-06 PoC で判定

## 残課題

- `InMemoryPolicyStore` は mock — 永続 grant store・commit fence・durable
  idempotency/recovery は GW-06(ACG-003/004)。
- `evaluate_commit` は線形化 seam の提示のみ — fence による policy read と
  project commit の直列化は GW-06 が構成する。
- `ScopedEffects` の実導出は GW-04 — GW-02 は入力 view の照合のみ。
- Approval receipt の binding(digest・principal・revision)は GW-05/06 —
  GW-02 は `ApprovalRequired` を返すまで。
- ProactivePermit.condition は opaque tag — 条件の型化は GW-09/ポリシー
  スキーマ設計へ。
- Dynamic scope の `max_members` は 1 request あたりの cap — target/effect
  両 pass で共有され同一 ResourceRef は重複計上しない(review 修正済み)。
