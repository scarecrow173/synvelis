# AUTX-01 Action Catalog — Case 台帳 (Issue #260 headless slice)

作成: 2026-11-14 / branch: `feat/260-autx01-action-catalog`
対象: `crates/synvelis-workspace/src/action_catalog.rs`・
`crates/synvelis-workspace/src/action_catalog/fixture.rs`・
`crates/synvelis-workspace/tests/autx01_action_catalog.rs` と
対応 docs（`docs/impl/ir|exp`・`docs/test-specs`）

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass ではない**。
実機未検証項目は Case ID 付きで NotRun/Deferred として記録する。
本スライスは `crates/synvelis-gateway`（sv#263 AUTX-04 リース）・
`crates/synvelis-app`・`crates/synvelis-cli`（#262 リース）・
`assets/` 等を変更しない（別 Worker リース / 範囲外）。

検証環境: Windows（worktree ホスト x86_64）、rustc は `mise` 管理
toolchain（`mise.ci.toml` pin）。canonical 検証は `mise run` レーン。

## 1. 受入条件 → Case → test → 結果

| Issue #260 受入行 | Case | 検証内容 | test / 証跡 | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| Action Catalog と ScriptApiModel (#271) の Capability ID/Version/Schema/Availability が一致し、二重 Registry/Metadata Disclosure 漏れがない | SV260-AC-01 | catalog の唯一 source は供給 `CapabilityDescriptor`。presentation は display metadata のみ（deny_unknown_fields + serialized-key 検査）。#271 実装は main 未存在 | `catalog_joins_presentations_with_current_descriptors`・`presentation_carries_only_display_metadata`・`catalog_is_registry_truth_not_a_second_registry` | descriptor は登録物 verbatim、presentation に authority field なし、第二 registry/登録 API なし | 全 case pass。#271 との実照合は SV260-NR-03 Deferred | **Pass（契約）/ NotRun（#271 照合）** |
| Script API 投影 #271 と Action Catalog が同じ Capability ID/Version/Schema/Availability を参照し、2 つの独立 Registry を作らない | SV260-AC-02 | 型レベルで同一保証: catalog は `Gateway`/`CapabilityRegistry` の read 投影のみ、`descriptors()` が AUTX-07 view にそのまま供給される。#271 実装は別 Issue | `catalog_feeds_autx07_view_projection_unchanged` + 本台帳 §3 | GUI/Script 双方が同一 `CapabilityRef`/descriptor を共有する構造 | 構造保証は pass。#271 ScriptApiModel 実装との照合は **NotRun** | **Pass（契約）/ NotRun（#271）** |
| 既存 `CapabilityRef` と Action は 1:1 の version 付き参照、表示名変更でも登録 Capability/Invocation の意味は変化しない | SV260-AC-03 | `capability: CapabilityRef` 完全一致 join・version 推測なし。display_key 変更で identity/status 不変 | `display_name_change_preserves_capability_identity`・`describe_distinguishes_unknown_id_and_unsupported_version` | rename で `capability`/`descriptor`/`status` 不変、未知 version は `UnsupportedVersion` | 全 case pass | **Pass（headless）** |
| `list/describe` で Unknown Version/Retired/Disabled/Planned/No Handler を別判定し、機械可読な Schema/Target/Effect/Availability の表示値は登録 Descriptor と一致 | SV260-AC-04 | `ActionStatus`（Unknown/UnsupportedVersion/Disabled/Planned/Retired/Available）+ `SchemaConsistency`（Consistent/Mismatch/Unknown）+ `ExecutorState`（Unknown — 下記 §3）を typed verdict で分離。descriptor field は登録物そのもの | `availability_states_project_verbatim_and_distinct`・`schema_consistency_is_typed_consistent_mismatch_unknown`・`orphan_presentations_surface_as_unknown_or_unsupported` | 5 状態が区別可能、schema/target/effect/availability は descriptor と一致 | 全 case pass。**No Handler は `ExecutorState::Unknown`**（has_executor accessor が lease 外のため SV260-NR-01 Deferred — Coordinator 指示の typed Unknown fallback） | **Pass（headless、No Handler は Deferred）** |
| 非公開/Protected Metadata を policy に反して開示せず、Action 列挙→Invoke だけで Grant/Approval/Manual Authority を獲得できない | SV260-AC-05 | presentation は display metadata のみ・wire は authority field を `parse_wire` が拒否・invoke は毎回 bind/policy を通る（DenyAllPolicy → `Denied`） | `enumeration_grants_no_authority_invoke_still_policy_gated`・`presentation_carries_only_display_metadata` | list/describe は read-only、DenyAll 下で invoke は Denied、self-declared authority は parse 拒否 | 全 case pass | **Pass（headless）** |
| Track/Clip Query・ProjectMutation・RuntimeControl・AsyncOperation について GUI/CLI 双方が同じ Capability ID/Version と明示 Stable Target で呼べる Fixture がある | SV260-AC-06 | consumer-neutral fixture が 4 domain の `WireRequest` を明示 target + domain-correct consistency で供給。`Gateway::bind`/`invoke` で Completed/Accepted を確認。GUI 消費は AUTX-07 projection 経由で固定 | `fixture_invocations_cover_all_domains_with_explicit_targets`・`catalog_feeds_autx07_view_projection_unchanged` | 全 domain が bind/invoke 成功、target は選択/focus から推測しない | 全 case pass。CLI surface 実装は #262 のため **NotRun** | **Pass（headless fixture）/ NotRun（CLI surface）** |
| Native UI 入力イベントを DAW の Action として誤扱いせず、Project/Undo/RT/Canvas は既存独立 Oracle のまま保つ | SV260-AC-07 | view-local UI action は registry 未登録 id として `Unknown` entry に留まり descriptor 無し・実行不能。Project/Undo/RT/Canvas は本 slice で非接触 | `unregistered_ids_stay_unknown_and_unexecutable` | ViewAction 系 id は catalog の executable capability を生まない | 全 case pass | **Pass（headless）** |

## 2. Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| SV260-T-01 | `synvelis-workspace::autx01_action_catalog` 全 15 test | Pass | join/status/sch
ema consistency/no-authority/identity/fixture 4 domain/view 互換 |
| SV260-T-02 | `synvelis-workspace` 既存 suite（autx07 29 test 他） | Pass | 既存契約の非破壊を確認 |
| SV260-T-03 | `cargo clippy --locked --all-targets`（synvelis-workspace、warnings denied） | Pass | doc_markdown/manual_assert_eq/match_wildcard 修正済み |
| SV260-T-04 | `cargo fmt --all --check` | Pass | fmt 適用済み |

## 3. NotRun / Deferred / Unsupported 対応表

| Case | 内容 | 状態 | Required/Deferred | 後続 gate / 必要ファイル |
| --- | --- | --- | --- | --- |
| SV260-NR-01 | **No Handler 別判定**: 登録済みだが executor 無し capability の事前判定。`CapabilityRegistry::handler`/`resolve` は `pub(crate)`、`has_executor` predicate が存在しない | **Unsupported → Deferred** | Coordinator が `crates/synvelis-gateway/` lease 拡張を否（sv#263 AUTX-04 衝突）。typed `ExecutorState::Unknown` で代替 — fail-closed のため誤実行なし、invoke 時 `NoExecutor` が最終 authority | 必要 API: `CapabilityRegistry::has_executor(&self, id) -> bool`（または `has_handler`）。Coordinator が sv#263 完了後に後続 slice を起票予定。対象: `crates/synvelis-gateway/src/registry.rs` + `crates/synvelis-workspace/src/action_catalog.rs`（`ExecutorState::{Installed,Absent}` 追加） |
| SV260-NR-02 | **Schema 整合の registry 経路**: `Gateway::schemas()` read accessor が無く `project_gateway` は `schemas_mut()` の sequential borrow で代替済みだが、`&Gateway` から直接 project する経路は `Option<&SchemaRegistry>` 供給時のみ `Unknown` 判定になる | **Deferred** | `Gateway::schemas_mut()` は公開済みのため実害なし。`&Gateway` caller が schema 判定を得るには read accessor が必要 | 必要 API: `Gateway::schemas(&self) -> &SchemaRegistry`。対象: `crates/synvelis-gateway/src/gateway.rs` + `action_catalog.rs`（`project(&Gateway)` 追加検討） |
| SV260-NR-03 | 実 ScriptApiModel（#271）との Capability ID/Version/Schema/Availability 同一性照合 | **NotRun** | Deferred（#271 Owner — ScriptApiModel 実装が main 未存在） | synvelis#271。実装後に `catalog.descriptors()` を共通 source として ID/version/schema/availability を照合 |
| SV260-NR-04 | CLI surface 本体（list/describe コマンド・認証・`parse_wire` 経路の実結合） | **NotRun** | Deferred（synvelis#262 のリース — `crates/synvelis-cli`） | synvelis#262。`action_catalog::fixture::invocations()` の wire shape を CLI が消費する結合検証 |
| SV260-NR-05 | Script Action 登録・script→catalog 投影 | **NotRun** | Deferred（synvelis#272） | synvelis#272 |
| SV260-NR-06 | 実 GPUI render・実 OS・live Gateway adapter 経由の実行 | **NotRun** | Deferred（#114/#267 別 Gate — GUI-L2） | synvelis#114/#267。`crates/synvelis-app` native lane |
| SV260-NR-07 | `doc-driven-dev-graph` route による task graph 生成 | **NotRun** | リポジトリ全域の pre-existing artifact-graph/broken-relation findings で router が `blocked` を返却（本 slice の変更とは無関係）。route 要求の impl/ir・impl/exp・test-specs 文書は lease 指定パスに作成済み | doc artifact graph の修復は別タスク。本 slice では canonical 契約 docs（`docs/automation/`）を直接参照して進行 |

## 4. 補足

- catalog は build 時 snapshot であり、registry への後続登録は
  rebuild まで反映されない。登録は append-only/immutable のため
  descriptor 改変は起きず、stale snapshot が権威を偽ることはない。
- `ActionCatalog::descriptors()` が返すのは登録 descriptor の
  clone であり、AUTX-07 `project_action_catalog` が必要とする
  `&[CapabilityDescriptor]` をそのまま供給する（view 契約不変）。
- `parameter_hints` は product display metadata であり、schema
  field との照合は行わない（hint 名と schema の不一致は表示品質の
  問題であって authority 契約違反ではない）。
